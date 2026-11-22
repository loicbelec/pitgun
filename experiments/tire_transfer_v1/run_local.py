#!/usr/bin/env python3
"""Frozen native tire-profile transfer before fixed-nine fitting."""
import collections
import itertools
import pathlib
import platform
import subprocess
import sys

EXP = pathlib.Path(__file__).resolve().parent
ROOT = EXP.parents[1]
sys.path.insert(0, str(ROOT / "experiments/tire_performance_v1"))
from kernel import Kernel, change, enc, key, load, sha, strategy, write
from run_screen import recipes


def legal(a, budget, era):
    offsets = [0, 0, 0, 0] if era == 1 else [2, 1, 4, 1]
    return sum(a) == budget and all(0 <= v and v + offsets[i] <= 20 for i, v in enumerate(a))


def allocations(ctx):
    n = ctx["base_budget"]
    result = {(0, n - e, e, 0) for e in range(n + 1)}
    for cooling, engine, aero in itertools.product(
        [0, 2, 4], [0, int(n * .25 + .5), int(n * .5 + .5)], [0, int(n * .125 + .5)]
    ):
        a = (aero, n - aero - engine - cooling, engine, cooling)
        if legal(a, n, ctx["era"]):
            result.add(a)
    return sorted(result)


def case(ctx, allocation, gear, downforce, profile, seed):
    return change({
        "id": "", "fixture_id": ctx["fixture_id"], "circuit": ctx["circuit"],
        "actor": "reference", "kind": "field", "budget": ctx["base_budget"],
        "allocation": list(allocation), "gear": gear, "downforce": downforce,
        "strategy": strategy(ctx["laps"], ["medium", "hard"], [ctx["laps"] // 2]),
        "seed": seed, "environment": None, "elite_level": 0,
        "performance_profile": profile,
    })


def setup_key(c):
    return tuple(c["allocation"]), c["gear"], c["downforce"]


def neighbors(c, era):
    result = []
    for donor, receiver in itertools.permutations(range(4), 2):
        if not c["allocation"][donor]:
            continue
        a = c["allocation"].copy()
        a[donor] -= 1
        a[receiver] += 1
        if legal(a, c["budget"], era):
            result.append(change(c, allocation=a))
    for field in (["gear"] if era == 1 else ["gear", "downforce"]):
        for delta in [-.25, .25]:
            v = c[field] + delta
            if 0 <= v <= 1:
                result.append(change(c, **{field: v}))
    return result


def best(rows):
    good = [r for r in rows if r["data"]["outcome"]["ok"]]
    if not good:
        raise RuntimeError("no successful comparison; never select a failed simulation")
    return min(good, key=lambda r: (r["data"]["outcome"]["time_s"], key(r["case"])))


def select_anchors(rows, contexts):
    anchors = []
    for ctx in contexts:
        rs = [r for r in rows if r["case"]["fixture_id"] == ctx["fixture_id"] and r["case"]["circuit"] == ctx["circuit"]]
        overall = best(rs)
        mc = best([r for r in rs if r["case"]["allocation"][0] == r["case"]["allocation"][3] == 0])
        label = "engine_chassis_only"
        if setup_key(overall["case"]) == setup_key(mc["case"]):
            mc = best([r for r in rs if r["case"]["allocation"][3] > 0])
            label = "positive_cooling_alternative"
        for role, r in [("best_sampled", overall), (label, mc)]:
            anchors.append({"era": ctx["era"], "circuit": ctx["circuit"], "role": role, "case": r["case"], "source_result_digest": r["data"]["outcome"]["result_digest"]})
    return anchors


def main():
    plan = load(EXP / "plan-v1.json")
    contexts = plan["contexts"]
    if sys.argv[1:] == ["--preview"]:
        print({"contexts": len(contexts), "setup_cases": sum(len(allocations(c)) * 3 * (1 if c["era"] == 1 else 2) for c in contexts), "strategy_cases_before_deduplication": len(contexts) * 2 * 198, "fresh_exact_repeats": len(contexts) * 4, "workers": plan["workers"]})
        return
    if len(sys.argv) != 3:
        raise SystemExit("usage: run_local.py NATIVE_BINARY PRIVATE_KERNEL_CACHE, or --preview")
    out = EXP / "results"
    out.mkdir(exist_ok=True)
    if (out / "campaign-v1.json").exists():
        raise RuntimeError("refuse overwrite completed transfer campaign")
    screen = ROOT / "experiments/tire_performance_v1/results"
    receipt = load(screen / "screen-campaign-v1.json")
    for name, digest in receipt["files"].items():
        if sha((screen / name).read_bytes()) != digest:
            raise RuntimeError("retained physical screen changed: " + name)
    selection = load(screen / "profile-selection-lock-v1.json")
    profile = selection["profile"]
    if profile != receipt["profile"]:
        raise RuntimeError("selected profile/receipt mismatch")
    kernel = Kernel(sys.argv[1], sys.argv[2], plan["workers"])
    if kernel.pin != receipt["provenance"]["kernel_pin"]:
        raise RuntimeError("transfer must use the already screened physical kernel")
    paths = [EXP / "run_local.py", EXP / "plan-v1.json", ROOT / "experiments/tire_performance_v1/run_screen.py", screen / "screen-campaign-v1.json", screen / "profile-selection-lock-v1.json"]
    sources = {str(p.relative_to(ROOT)): sha(p.read_bytes()) for p in paths}
    provenance = {"source_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(), "profile": profile, "sources": sources, "kernel_pin": kernel.pin, "python": platform.python_version(), "platform": platform.platform(), "rustc": subprocess.check_output(["rustc", "-Vv"], text=True).strip(), "initial_cached_case_ids": sorted(p.stem.removesuffix(".json") for p in kernel.cache.glob("*.json.gz")), "reserved_field_results_opened": False}
    pin = kernel.cache / "tire-transfer-protocol-v1.json"
    if pin.exists():
        prior = load(pin)
        if prior["sources"] != sources or prior["kernel_pin"] != kernel.pin:
            raise RuntimeError("transfer protocol cache mismatch")
        provenance = prior
    else:
        write(pin, provenance)
    write(out / "provenance-v1.json", provenance)
    stats = load(out / "phase-stats-v1.json") if (out / "phase-stats-v1.json").exists() else {}
    phase = "initialization"

    def batch(name, cases, force=False):
        nonlocal phase
        phase = name
        if name in stats:
            p = out / (name + "-v1.json.gz")
            if sha(p.read_bytes()) != stats[name]["receipt_digest"]:
                raise RuntimeError("completed phase changed: " + name)
            return load(p)
        rows, counts = kernel.batch(name, cases, out, force)
        counts["receipt_digest"] = sha((out / (name + "-v1.json.gz")).read_bytes())
        stats[name] = counts
        write(out / "phase-stats-v1.json", stats)
        return rows

    try:
        setup = []
        for ctx in contexts:
            for a, gear, df in itertools.product(allocations(ctx), [0, .5, 1], [.5] if ctx["era"] == 1 else [0, .5]):
                setup.append(case(ctx, a, gear, df, profile, plan["training_seed"]))
        initial = batch("setup-search", setup)
        refinement = []
        for ctx in contexts:
            rs = [r for r in initial if r["case"]["circuit"] == ctx["circuit"] and r["case"]["fixture_id"] == ctx["fixture_id"] and r["data"]["outcome"]["ok"]]
            top = sorted(rs, key=lambda r: (r["data"]["outcome"]["time_s"], key(r["case"])))[:2]
            for r in top:
                refinement += neighbors(r["case"], ctx["era"])
        refined = batch("setup-refinement", refinement)
        setups = list({key(r["case"]): r for r in initial + refined}.values())
        anchors = select_anchors(setups, contexts)
        lock = {"schema": "pitgun.tire-transfer-anchor-lock/v1", "profile": profile, "anchors": anchors, "before_new_strategy_results": True, "reserved_field_results_opened": False}
        lock_path = out / "anchor-lock-v1.json"
        if lock_path.exists() and load(lock_path) != lock:
            raise RuntimeError("transfer anchors changed")
        write(lock_path, lock)
        comparison = []
        for anchor in anchors:
            c = anchor["case"]
            one_two, extra = recipes(sum(s["laps"] for s in c["strategy"]["stints"]))
            comparison += [change(c, strategy=s, diagnostic_mode=False) for s in one_two]
            comparison += [change(c, strategy=s, diagnostic_mode=True) for s in extra]
        strategies = batch("strategy-comparison", comparison)
        checks, repeats = [], []
        for anchor in anchors:
            c = anchor["case"]
            rs = [r for r in strategies if r["case"]["fixture_id"] == c["fixture_id"] and r["case"]["circuit"] == c["circuit"] and setup_key(r["case"]) == setup_key(c)]
            one_two = best([r for r in rs if not r["case"]["diagnostic_mode"]])
            extra = best([r for r in rs if r["case"]["diagnostic_mode"]])
            margin = extra["data"]["outcome"]["time_s"] - one_two["data"]["outcome"]["time_s"]
            failures = sum(not r["data"]["outcome"]["ok"] for r in rs)
            checks.append({"era": anchor["era"], "circuit": anchor["circuit"], "role": anchor["role"], "anchor_case": c, "best_one_two_case": one_two["case"], "best_extra_case": extra["case"], "best_one_two_time_s": one_two["data"]["outcome"]["time_s"], "best_extra_time_s": extra["data"]["outcome"]["time_s"], "extra_stop_margin_s": margin, "failed_simulations": failures, "passed": margin >= 0 and failures == 0})
            repeats += [one_two["case"], extra["case"]]
        write(out / "context-checks-v1.json", checks)
        repeated = batch("exact-repeat", repeats, True)
        unique = {key(r["case"]): r for r in setups + strategies}
        for r in repeated:
            if r["data"] != unique[key(r["case"])]["data"]:
                raise RuntimeError("published exact repeat differs")
        failures = [{"case": r["case"], "outcome": r["data"]["outcome"]} for r in unique.values() if not r["data"]["outcome"]["ok"]]
        carry = [max(r["data"]["maximum_carry_jumps"][i] for r in unique.values()) for i in range(5)]
        residual = max(abs(r["data"]["outcome"]["time_s"] - sum(r["data"]["lap_times_s"]) - len(r["case"]["strategy"]["pit_laps"]) * r["data"]["receipt"]["resolved_pit_loss_s"]) for r in unique.values() if r["data"]["outcome"]["ok"])
        initial_ids = set(provenance["initial_cached_case_ids"])
        passed = not failures and max(carry) == 0 and residual <= 1e-8 and all(c["passed"] for c in checks)
        summary = {"schema": "pitgun.tire-transfer-summary/v1", "status": "PASS_NATIVE_TRANSFER" if passed else "REFINE", "profile": profile, "contexts": len(contexts), "anchors": len(anchors), "unique_cases": len(unique), "new_unique_case_results": sum(k not in initial_ids for k in unique), "reused_existing_case_results": sum(k in initial_ids for k in unique), "fresh_exact_repeat_executions": stats["exact-repeat"]["fresh"], "failed_simulations": failures, "maximum_carry_jumps_speed_engine_surface_carcass_wear": carry, "maximum_pit_accounting_residual_s": residual, "checks": checks, "reserved_field_results_opened": False, "no_runtime_admission_or_final_field": True}
        write(out / "summary-v1.json", summary)
        kernel.verify()
        if {p: sha((ROOT / p).read_bytes()) for p in sources} != sources or load(lock_path) != lock:
            raise RuntimeError("protocol or anchor lock changed")
        write(out / "campaign-v1.json", {"schema": "pitgun.tire-transfer-receipt/v1", "status": summary["status"], "phase_stats": stats, "provenance": provenance, "files": {p.name: sha(p.read_bytes()) for p in sorted(out.glob("*-v1.*")) if p.name != "campaign-v1.json"}})
        print("transfer complete: " + summary["status"], flush=True)
        return 0 if passed else 2
    except Exception as error:
        write(out / "interrupted-v1.json", {"phase": phase, "error": str(error), "reserved_field_results_opened": False, "provenance": provenance})
        raise
    finally:
        kernel.close()


if __name__ == "__main__":
    sys.exit(main())
