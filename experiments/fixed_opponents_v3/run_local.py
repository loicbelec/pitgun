#!/usr/bin/env python3
"""Fixed personalities, trained preparation, sealed evaluation. No admitted runtime."""
import collections
import copy
import itertools
import json
import threading
import pathlib
import platform
import subprocess
import sys
import tempfile

import common as c
from common import EXP, ROOT, PLAN, PROFILE, ROSTER, FIXTURES, CONTEXTS
from common import change, enc, key, load, sha, write


def distinct(rows, identity, count):
    found = {}
    for r in sorted((r for r in rows if c.ok(r)), key=c.order):
        found.setdefault(identity(r), r)
        if len(found) == count:
            break
    return list(found.values())


def setup_id(r):
    v = r["case"]
    return tuple(v["allocation"]), v["gear"], v["downforce"]


def select_players(rows, labels):
    lookup = {key(r["case"]): r for r in rows}
    groups = collections.defaultdict(list)
    for fixture, context, style, identity in labels:
        groups[(fixture, context, style)].append(lookup[identity])
    return [{"fixture": f, "context": list(ctx), "style": style, "case": c.best(rs)["case"]} for (f, ctx, style), rs in sorted(groups.items())]


def assess(fields, players, player_rows):
    lookup = {key(r["case"]): r for r in player_rows}
    points = collections.defaultdict(float)
    all_finished = True
    for context, rows in fields.items():
        for i, r in enumerate(c.rank(rows)):
            points[r["case"]["actor"]] += [25, 18, 15, 12, 10, 8, 6, 4, 2][i] if c.ok(r) else 0
            all_finished &= c.ok(r)
    top = sorted(points, key=lambda a: (-points[a], a))[:2]
    difficulty = {}
    pooled = []
    player_index = {(p["fixture"], tuple(p["context"]), p["style"]): p for p in players}
    for era in [1, 2]:
        positions, mc_positions, fixed_positions, improvements = [], [], [], []
        for p in players:
            if p["style"] != "informed" or not p["fixture"].endswith(f"era{era}-exit"):
                continue
            context = tuple(p["context"])
            outcomes = {}
            for style in ["informed", "sampled_mc", "fixed_mc", "routine"]:
                q = player_index[(p["fixture"], context, style)]
                r = lookup[key(q["case"])]
                ranked = c.rank(fields[context] + [r])
                outcomes[style] = (next(i + 1 for i, x in enumerate(ranked) if x["case"]["actor"] == "player"), r)
                all_finished &= c.ok(r)
            positions.append(outcomes["informed"][0])
            mc_positions.append(outcomes["sampled_mc"][0])
            fixed_positions.append(outcomes["fixed_mc"][0])
            improvements.append(outcomes["routine"][1]["data"]["outcome"]["time_s"] - outcomes["informed"][1]["data"]["outcome"]["time_s"] if c.ok(outcomes["routine"][1]) and c.ok(outcomes["informed"][1]) else 0)
        gains = [m - p for m, p in zip(mc_positions, positions)]
        pooled += gains
        difficulty[str(era)] = {"positions": positions, "sampled_mc_positions": mc_positions, "fixed_mc_positions": fixed_positions, "wins": positions.count(1), "median": sorted(positions)[len(positions) // 2], "mean_routine_gain_s": sum(improvements) / len(improvements), "mean_rank_gain_over_sampled_mc": sum(gains) / len(gains)}
    pooled_gain = sum(pooled) / len(pooled)
    eligible = all_finished and set(top) == set(ROSTER["front_runners"]) and pooled_gain >= .5 and all(d["wins"] >= 1 and d["median"] <= 3 and d["mean_routine_gain_s"] > 0 and d["mean_rank_gain_over_sampled_mc"] >= 0 and d["fixed_mc_positions"].count(1) < len(d["positions"]) for d in difficulty.values())
    return {"eligible": eligible, "all_finished": all_finished, "top_two": top, "points": dict(points), "difficulty": difficulty, "pooled_exit_rank_gain_over_sampled_mc": pooled_gain}


def main():
    if sys.argv[1:] == ["--preview"]:
        print({"contexts": len(CONTEXTS), "fixed_drivers": len(ROSTER["drivers"]), "named_initial_pairs": 10 * 9 * 3 * 3, "exit_player_full_strategy_requests": 9 * 2 * 114, "final_driver_strategy_requests_per_attempt": 10 * 9 * 198, "max_distinct_budget_attempts": 3, "workers": PLAN["workers"]})
        return
    if len(sys.argv) != 3:
        raise SystemExit("usage: run_local.py FROZEN_NATIVE_BINARY PRIVATE_KERNEL_CACHE")
    out = EXP / "results"
    out.mkdir(exist_ok=True)
    if (out / "campaign-v3.json").exists():
        raise RuntimeError("refuse overwrite completed campaign")
    transfer = ROOT / "experiments/tire_transfer_v1/results"
    transfer_receipt = load(transfer / "campaign-v1.json")
    if transfer_receipt["status"] != "PASS_NATIVE_TRANSFER":
        raise RuntimeError("completed tire transfer required")
    for name, digest in transfer_receipt["files"].items():
        if sha((transfer / name).read_bytes()) != digest:
            raise RuntimeError("transfer receipt changed: " + name)
    kernel = AuditedKernel(sys.argv[1], sys.argv[2], PLAN["workers"], out / "native-executions-v3.jsonl")
    if kernel.pin != transfer_receipt["provenance"]["kernel_pin"]:
        raise RuntimeError("physical kernel differs from qualified transfer")
    paths = [EXP / "run_local.py", EXP / "common.py", EXP / "plan-v3.json", EXP / "check_protocol.py", ROOT / "experiments/tire_performance_v1/run_screen.py", ROOT / "experiments/tire_performance_v1/results/profile-selection-lock-v1.json", transfer / "campaign-v1.json"]
    sources = {str(p.relative_to(ROOT)): sha(p.read_bytes()) for p in paths}
    provenance = {"source_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(), "sources": sources, "kernel_pin": kernel.pin, "performance_profile": PROFILE, "python": platform.python_version(), "initial_cached_case_ids": sorted(p.stem.removesuffix(".json") for p in kernel.cache.glob("*.json.gz")), "reserved_results_opened": False}
    pin = kernel.cache / "fixed-field-protocol-v3.json"
    if pin.exists():
        prior = load(pin)
        if prior["sources"] != sources or prior["kernel_pin"] != kernel.pin:
            raise RuntimeError("fixed field protocol changed")
        provenance = prior
    else:
        write(pin, provenance)
    write(out / "provenance-v3.json", provenance)
    stats = load(out / "phase-stats-v3.json") if (out / "phase-stats-v3.json").exists() else {}
    phase = "initialization"

    def batch(name, cases, force=False):
        nonlocal phase
        phase = name
        kernel.phase = name
        if name in stats:
            p = out / (name + "-v3.json.gz")
            if sha(p.read_bytes()) != stats[name]["receipt_digest"]:
                raise RuntimeError("retained phase changed: " + name)
            return load(p)
        rows, counts = kernel.batch(name, cases, out, force)
        p = out / (name + "-v3.json.gz")
        (out / (name + "-v1.json.gz")).replace(p)
        counts["receipt_digest"] = sha(p.read_bytes())
        stats[name] = counts
        write(out / "phase-stats-v3.json", stats)
        return rows

    def verify():
        kernel.verify()
        if {p: sha((ROOT / p).read_bytes()) for p in sources} != sources:
            raise RuntimeError("campaign protocol/source changed")

    def finish(status, **extra):
        verify()
        write(out / "campaign-v3.json", {"schema": "pitgun.fixed-field-campaign-receipt/v3", "status": status, "phase_stats": stats, "provenance": provenance, **extra, "files": {p.name: sha(p.read_bytes()) for p in sorted(out.glob("*-v3.*")) if p.name != "campaign-v3.json"}})
        print("campaign complete: " + status, flush=True)
        return 0 if status == "COMPLETE_NATIVE_EVALUATION" else 2

    try:
        setups = [r for name in ["setup-search", "setup-refinement"] for r in load(transfer / (name + "-v1.json.gz"))]
        setups = list({key(r["case"]): r for r in setups}.values())
        strategies = [r for r in load(transfer / "strategy-comparison-v1.json.gz") if not r["case"]["diagnostic_mode"]]
        write(out / "reference-reuse-v3.json", {"transfer_receipt_digest": sha((transfer / "campaign-v1.json").read_bytes()), "setup_cases": len(setups), "normal_strategy_cases": len(strategies), "no_old_physics_driver_imports": True})
        initial = []
        for context in CONTEXTS:
            top_a = distinct([r for r in setups if c.cohort(r) == context], setup_id, 3)
            top_s = distinct([r for r in strategies if c.cohort(r) == context], lambda r: sha(enc(r["case"]["strategy"])), 3)
            for actor, a, s in itertools.product(ROSTER["drivers"], top_a, top_s):
                initial.append(change(a["case"], actor=actor, strategy=s["case"]["strategy"]))
        adapted = batch("named-driver-search", initial)
        local_cases = []
        for context in CONTEXTS:
            for actor in ROSTER["drivers"]:
                b = c.best([r for r in adapted if c.cohort(r) == context and r["case"]["actor"] == actor])
                local_cases += c.local(b["case"], context[0], False)
        adapted += batch("named-driver-local", local_cases)
        selected = {(context, actor): c.near_style([r for r in adapted if c.cohort(r) == context and r["case"]["actor"] == actor], actor) for context in CONTEXTS for actor in ROSTER["drivers"]}
        preflight_repeats = []
        for era in [1, 2]:
            context = next(ctx for ctx in CONTEXTS if ctx[0] == era)
            preflight_repeats += [selected[(context, actor)]["case"] for actor in ROSTER["drivers"]]
        batch("named-driver-exact-repeat", preflight_repeats, True)
        tiers = []
        for (context, actor), r in selected.items():
            for budget, level in itertools.product(PLAN["budget_tiers"][str(context[0])], [0, 1, 2] if actor in ROSTER["front_runners"] else [0]):
                tiers.append(change(c.scale(r["case"], budget, context[0]), elite_level=level))
        tier_rows = batch("fixed-budget-calibration", tiers)
        tier_map = {key(r["case"]): r for r in tier_rows}
        players, labels = [], []
        for fixture, f in FIXTURES.items():
            for prep in f["preparedRequests"]:
                context = (f["era"], prep["circuitId"])
                ctx = CONTEXTS[context]
                budget = f["discretionaryPoints"]
                normal = [budget // 4] * 4
                normal[2] += budget - sum(normal)
                default = c.strategy(ctx["laps"], ["medium", "hard"], [ctx["laps"] // 2])
                for style, allocation in [("routine", normal), ("fixed_mc", [0, budget - budget // 2, budget // 2, 0])]:
                    x = c.case(ctx, "player", allocation, .5, .5, default, budget, "career_player", fixture)
                    players.append(x)
                    labels.append((fixture, context, style, key(x)))
                top_s = distinct([r for r in strategies if c.cohort(r) == context], lambda r: sha(enc(r["case"]["strategy"])), 6)
                for style in ["informed", "sampled_mc"]:
                    options = [r for r in setups if c.cohort(r) == context and (style != "sampled_mc" or r["case"]["allocation"][0] == r["case"]["allocation"][3] == 0)]
                    for a, s in itertools.product(distinct(options, setup_id, 3), top_s):
                        scaled = c.scale(a["case"], budget, context[0])
                        df = scaled["downforce"] if f["sliders"]["downforce"][0] != f["sliders"]["downforce"][1] else .5
                        x = c.case(ctx, "player", scaled["allocation"], scaled["gear"], df, s["case"]["strategy"], budget, "career_player", fixture)
                        if not c.permitted(x):
                            raise RuntimeError("generated player setup exceeds actual preparation limits")
                        players.append(x)
                        labels.append((fixture, context, style, key(x)))
        player_rows = batch("player-training", players)
        for round_index in range(2):
            neighbors, new_labels = [], []
            for p in select_players(player_rows, labels):
                if p["style"] not in ["informed", "sampled_mc"]:
                    continue
                x = p["case"]
                ns = c.local(x, p["context"][0], True)
                if round_index == 0:
                    for donor, receiver in itertools.permutations(range(4), 2):
                        if x["allocation"][donor] < 2:
                            continue
                        a = x["allocation"].copy()
                        a[donor] -= 2
                        a[receiver] += 2
                        n = change(x, allocation=a)
                        if c.permitted(n):
                            ns.append(n)
                for n in ns:
                    if p["style"] == "sampled_mc" and (n["allocation"][0] or n["allocation"][3]):
                        continue
                    neighbors.append(n)
                    new_labels.append((p["fixture"], tuple(p["context"]), p["style"], key(n)))
            player_rows += batch(f"player-local-{round_index+1}", neighbors)
            labels += new_labels
        sweep, sweep_labels = [], []
        for p in select_players(player_rows, labels):
            if p["style"] not in ["informed", "sampled_mc"] or not p["fixture"].endswith("exit"):
                continue
            for s in c.recipes(CONTEXTS[tuple(p["context"])]["laps"]):
                n = change(p["case"], strategy=s)
                sweep.append(n)
                sweep_labels.append((p["fixture"], tuple(p["context"]), p["style"], key(n)))
        player_rows += batch("exit-player-strategies", sweep)
        labels += sweep_labels
        player_selected = select_players(player_rows, labels)
        selected_player_ids = {key(p["case"]) for p in player_selected}
        player_rows = list({key(r["case"]): r for r in player_rows if key(r["case"]) in selected_player_ids}.values())
        del setups, strategies, adapted

        def initial_fields(n1, n2, level):
            result = {}
            for context in CONTEXTS:
                result[context] = []
                for actor in ROSTER["drivers"]:
                    x = change(c.scale(selected[(context, actor)]["case"], n1 if context[0] == 1 else n2, context[0]), elite_level=level if actor in ROSTER["front_runners"] else 0)
                    result[context].append(tier_map[key(x)])
            return result

        approximations = []
        for n1, n2, level in itertools.product(PLAN["budget_tiers"]["1"], PLAN["budget_tiers"]["2"], [0, 1, 2]):
            approximations.append({"budgets": {"1": n1, "2": n2}, "elite_level": level, **assess(initial_fields(n1, n2, level), player_selected, player_rows)})
        write(out / "calibration-candidates-v3.json", approximations)
        by_budget = {}
        for x in approximations:
            if x["eligible"]:
                k = (x["budgets"]["1"], x["budgets"]["2"])
                if k not in by_budget or x["elite_level"] < by_budget[k]["elite_level"]:
                    by_budget[k] = x
        choices = sorted(by_budget.values(), key=lambda x: (min(x["budgets"]["1"] / 8, x["budgets"]["2"] / 16), x["budgets"]["1"] / 8 + x["budgets"]["2"] / 16, -x["elite_level"], x["budgets"]["2"], x["budgets"]["1"]), reverse=True)[:3]
        write(out / "training-attempt-lock-v3.json", {"choices": choices, "before_full_named_strategy_qualification": True, "reserved_results_opened": False})
        qualified = None
        attempts = []
        for index, choice in enumerate(choices):
            field = initial_fields(choice["budgets"]["1"], choice["budgets"]["2"], choice["elite_level"])
            final_field, pit_checks = {}, []
            for context, rs in field.items():
                cases = []
                normal, extra_plans = c.physical_recipes(CONTEXTS[context]["laps"])
                for r in rs:
                    cases.append(r["case"])
                    cases += [change(r["case"], strategy=s, diagnostic_mode=False) for s in normal]
                    cases += [change(r["case"], strategy=s, diagnostic_mode=True) for s in extra_plans]
                rows = batch(f"final-driver-strategies-{index+1}-era{context[0]}-{context[1]}", cases)
                final_field[context] = []
                for actor in ROSTER["drivers"]:
                    actor_rows = [r for r in rows if c.cohort(r) == context and r["case"]["actor"] == actor]
                    normal_rows = [r for r in actor_rows if not r["case"]["diagnostic_mode"]]
                    preferred = c.near_style(normal_rows, actor)
                    absolute = c.best(normal_rows)
                    extra = c.best([r for r in actor_rows if r["case"]["diagnostic_mode"]])
                    suppressed = c.order(preferred)[0] > c.order(extra)[0] and c.order(absolute)[0] <= c.order(extra)[0]
                    chosen = absolute if suppressed else preferred
                    margin = c.order(extra)[0] - c.order(chosen)[0]
                    final_field[context].append(chosen)
                    pit_checks.append({"context": list(context), "actor": actor, "normal_case": chosen["case"], "extra_case": extra["case"], "extra_stop_margin_s": margin, "style_preference_suppressed": suppressed, "passed": margin >= 0 and all(c.ok(r) for r in actor_rows)})
                del rows
            result = {**choice, "final_training_assessment": assess(final_field, player_selected, player_rows), "pit_checks": pit_checks}
            result["qualified"] = result["final_training_assessment"]["eligible"] and all(p["passed"] for p in pit_checks)
            attempts.append(result)
            write(out / "training-qualification-v3.json", attempts)
            if result["qualified"]:
                qualified = (result, final_field)
                break
        if qualified is None:
            return finish("REFINE_TRAINING_NO_HOLDOUT", no_policy_selected=True, reserved_results_opened=False)

        chosen, field = qualified
        for actor in ROSTER["drivers"]:
            records = [r["data"]["receipt"]["driver"] for rs in field.values() for r in rs if r["case"]["actor"] == actor]
            if len({r["driver_resource_digest"] for r in records}) != 1 or any(r["controls"]["driver_id"] != actor or r["controls"]["requested_commitment"] != .8 for r in records):
                raise RuntimeError("driver identity/traits/mode changed between training contexts")
        policy = {"schema": "pitgun.fixed-opponent-policy-candidate/v1", "not_admitted_to_runtime": True, "performance_profile": PROFILE, "maximum_candidate_voluntary_stops": 2, "roster": copy.deepcopy(ROSTER), "elite_level": chosen["elite_level"], "budgets_by_era": chosen["budgets"], "field_contexts": [{"era": ctx[0], "circuit": ctx[1], "cases": [r["case"] for r in rs]} for ctx, rs in field.items()], "player_cases": player_selected, "training_choice": chosen["final_training_assessment"]}
        for actor in ROSTER["front_runners"]:
            for name, value in policy["roster"]["drivers"][actor]["resource"]["traits"].items():
                policy["roster"]["drivers"][actor]["resource"]["traits"][name] = min(.99, value + .02 * chosen["elite_level"])
        policy["roster"]["status"] = "native V3 candidate; fixed expanded traits chosen in training, not admitted to runtime"
        policy["profile_derivation"] = "Native cases use unchanged base V2 roster plus the one globally selected elite_level. Expanded roster resources must not receive that calibration a second time during future admission."
        seal = {"schema": "pitgun.fixed-field-selection-lock/v1", "policy_digest": sha(enc(policy)), "policy": policy, "reserved_results_opened_at_seal": False}
        p = out / "selection-lock-v3.json"
        if p.exists() and load(p) != seal:
            raise RuntimeError("selected policy changed")
        write(p, seal)
        validation = [change(r["case"], seed=seed, environment=env) for rs in field.values() for r in rs for seed, env in itertools.product(PLAN["validation_seeds"], PLAN["validation_environments"])]
        validation += [change(p["case"], seed=seed, environment=env) for p in player_selected for seed, env in itertools.product(PLAN["validation_seeds"], PLAN["validation_environments"])]
        held = batch("reserved-field-and-player", validation)
        repeats = [r["case"] for r in held if r["case"]["seed"] == PLAN["validation_seeds"][0] and r["case"]["environment"] is None]
        batch("reserved-exact-repeat", repeats, True)
        audits, invariance = [], []
        with tempfile.TemporaryDirectory(prefix="pitgun-final-field-") as td:
            p = pathlib.Path(td) / "field.json"
            for context, rs in field.items():
                if context[1] not in ["MONZA", "ZANDVOORT", "MONTREAL"]:
                    continue
                player = next(x for x in player_selected if x["context"] == list(context) and x["style"] == "informed" and x["fixture"].endswith("exit"))
                cases = [change(player["case"], seed=PLAN["validation_seeds"][0])] + [change(r["case"], seed=PLAN["validation_seeds"][0]) for r in rs]
                write(p, cases)
                native = load_native_field(kernel, p)
                for r in native["standings"]:
                    case = next(x for x in cases if x["actor"] == r["actor"])
                    if r["data"] != kernel.execute(case):
                        raise RuntimeError("native ten-car/independent result mismatch")
                audits.append({"context": list(context), "cases": cases, "result": native})
            for record in [next(a for a in audits if a["context"][0] == era) for era in [1, 2]]:
                original = {r["actor"]: r["data"] for r in record["result"]["standings"]}
                write(p, list(reversed(record["cases"])))
                reordered = load_native_field(kernel, p)
                if {r["actor"]: r["data"] for r in reordered["standings"]} != original:
                    raise RuntimeError("roster order changes physical result")
                player = next(x for x in record["cases"] if x["actor"] == "player")
                alternative = c.point_neighbors(player, record["context"][0])[0]
                altered = [alternative if x["actor"] == "player" else x for x in record["cases"]]
                write(p, altered)
                changed = load_native_field(kernel, p)
                if any(r["data"] != original[r["actor"]] for r in changed["standings"] if r["actor"] != "player"):
                    raise RuntimeError("player setup changes an opponent")
                invariance.append({"context": record["context"], "reorder_exact": True, "player_change_preserves_nine": True, "altered_cases": altered, "altered_result": changed})
        write(out / "native-field-audit-v3.json.gz", {"fields": audits, "invariance": invariance})
        if load(out / "selection-lock-v3.json") != seal:
            raise RuntimeError("policy changed after held-out evaluation")
        return finish("COMPLETE_NATIVE_EVALUATION", policy_digest=seal["policy_digest"], native_field_extra_executions=len(audits)*10+len(invariance)*20, reserved_results_opened=True)
    except Exception as error:
        write(out / "interrupted-v3.json", {"phase": phase, "error": str(error), "provenance": provenance})
        raise
    finally:
        kernel.close()


class AuditedKernel(c.Kernel):
    def __init__(self, binary, cache, workers, journal):
        super().__init__(binary, cache, workers)
        self.journal = journal
        self.journal_lock = threading.Lock()
        self.phase = "initialization"

    def record(self, data, forced=False, field=False):
        with self.journal_lock:
            with self.journal.open("a") as stream:
                stream.write(json.dumps({"id": data["id"], "phase": self.phase, "forced": forced, "native_field": field, "ok": data["outcome"]["ok"], "result_digest": data["outcome"].get("result_digest")}, separators=(",", ":")) + "\n")

    def execute(self, case, force=False):
        fresh = force or not (self.cache / (key(case) + ".json.gz")).exists()
        data = super().execute(case, force)
        if fresh:
            self.record(data, force)
        return data


def load_native_field(kernel, path):
    kernel.phase = "native-field-audit"
    result = json.loads(subprocess.check_output([str(kernel.binary), str(ROOT), str(path), "--field"]))
    for row in result["standings"]:
        kernel.record(row["data"], field=True)
    return result


if __name__ == "__main__":
    sys.exit(main())
