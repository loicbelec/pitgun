#!/usr/bin/env python3
"""Read-only analysis; no coefficient selection, further runs or AI fitting."""
import argparse
import gzip
import json
from pathlib import Path


def load(path):
    raw = path.read_bytes()
    return json.loads(gzip.decompress(raw) if path.suffix == ".gz" else raw)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("input", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--gear-controls", type=Path)
    args = parser.parse_args()
    report = load(args.input)
    if not report["complete"]:
        raise SystemExit("Incomplete checkpoint: no final qualification report generated.")
    plan, rows = report["plan"], report["rows"]
    lookup = {r["case"]["id"]: r for r in rows}
    circuits = [c["circuit"] for c in plan["contexts"]]
    failures = [r for r in rows if r["checks"]]
    failed_validations = [v for v in report["validations"] if not v["exact"]]

    def row(circuit, setup, recipe, suffix=""):
        return lookup[f"{circuit}/{setup}/{recipe}{suffix}"]

    def time_s(r):
        return r["data"]["outcome"].get("time_s", float("inf")) if not r["checks"] else float("inf")

    def best(items):
        valid = [r for r in items if not r["checks"]]
        return min(valid, key=time_s) if valid else None

    def core(circuit, setup):
        return [row(circuit, setup, p) for p in plan["plans"]]

    def driving_metrics(r):
        laps = r["data"]["laps"]
        return {"min_power": min(l["minimum_power_fraction"] for l in laps),
                "derated_s": sum(l["derated_time_s"] for l in laps),
                "errors": sum(l["driver_lapses"] for l in laps)}

    numerical_pass = not failures and not failed_validations
    lines = ["# Era-I strategy qualification — frozen operating candidate", "",
             "Tracking: #482 / #479 / draft#481. No physical parameters changed during this campaign.", "",
             f"Numerical gate: **{'PASS' if numerical_pass else 'REFINE'}**. "
             f"{len(rows)} main cases and {len(report['validations'])} validation executions; "
             f"{len(failures)} main cases with failures/check violations, {len(failed_validations)} failed equality checks.", "",
             "## Scope", "",
             "Single-car full-distance native runs, eight effective points, same baseline engine/car/110kg/reference traits. "
             "The five preparation samples include three allocation profiles and two paired slider contrasts. "
             "Modes and reserved seed are separate comparisons. No rivals or AI setups are fitted. "
             "Zero stops is a physical control, not a change to sporting rules. All rankings below include catalog service loss.", "",
             "## Best sampled physical preparation", "",
             "Balanced/primary-seed cases only, including diagnostic0/3 stops and paired slider samples. "
             "This is not a career-legal recommendation: era-I baseline downforce is locked at.5, and the game requires at least two stints. This is a bounded sample, not a global optimum.", "",
             "| Circuit | Preparation / recipe | Stops | Total s | Driving s | Service s |",
             "|---|---|---:|---:|---:|---:|"]
    for circuit in circuits:
        winner = best([r for r in rows if r["case"]["circuit"] == circuit
                       and r["case"]["mode"] == "balanced" and r["case"]["seed"] == plan["seed"]])
        if winner is None:
            lines.append(f"| {circuit} | no valid finish | — | — | — | — |")
            continue
        o = winner["data"]["outcome"]
        lines.append(f"| {circuit} | {' / '.join(winner['case']['id'].split('/')[1:])} | "
                     f"{len(winner['case']['strategy']['pit_laps'])} | {o['time_s']:.3f} | {o['drive_time_s']:.3f} | {o['service_time_s']:.1f} |")

    lines += ["", "## Stop-count comparison at the same balanced preparation", "",
              "Only the nine preregistered core recipes on2222/.5/.5. Values are total GP seconds; each cell is the best sampled recipe for that stop count.", "",
              "| Circuit | 0 stops | 1 stop | 2 stops | 3 stops | Sampled best |",
              "|---|---:|---:|---:|---:|---|"]
    three_stop_counters = []
    for circuit in circuits:
        grouped = [best([r for r in core(circuit, "balanced") if len(r["case"]["strategy"]["pit_laps"]) == count]) for count in range(4)]
        winner = best([r for r in grouped if r is not None])
        cells = [f"{time_s(r):.3f}" if r else "failure" for r in grouped]
        lines.append(f"| {circuit} | {' | '.join(cells)} | {winner['case']['id'].split('/')[2] if winner else 'none'} |")
        for setup in plan["setups"]:
            counter = row(circuit, setup, "SMSM-quarters")
            normal = best([r for r in core(circuit, setup) if len(r["case"]["strategy"]["pit_laps"]) in (1, 2)])
            if normal and time_s(counter) < time_s(normal) - 0.05:
                three_stop_counters.append(counter["case"]["id"])

    lines += ["", "## Allocations under the same nine strategy samples", "",
              "A/C/E/K: balanced2222, concentrated0440, cooling0332. Default .5/.5 sliders and equal effective budget. "
              "This comparison does not optimize the concentrated setup's sliders or all legal allocations.", "",
              "| Circuit | 2222 best total s | 0440 best total s | 0332 best total s | 0440 minus best other s |",
              "|---|---:|---:|---:|---:|"]
    concentrated_dominates = True
    for circuit in circuits:
        winners = [best(core(circuit, s)) for s in plan["setups"]]
        if any(r is None for r in winners):
            lines.append(f"| {circuit} | incomplete valid comparison | — | — | — |")
            concentrated_dominates = False
            continue
        values = list(map(time_s, winners))
        delta = values[1] - min(values[0], values[2])
        concentrated_dominates &= delta <= 0.
        lines.append(f"| {circuit} | {values[0]:.3f} | {values[1]:.3f} | {values[2]:.3f} | {delta:+.3f} |")

    lines += ["", "## Driver modes at fixed preparation and SM timing", "",
              "Same reference traits and noise stream. Recoverable errors are native driver lapses, not crashes or VSC.", "",
              "| Circuit | Mode | Total s | Delta Balanced s | Minimum power factor | Derated driving s | Recoverable errors |",
              "|---|---|---:|---:|---:|---:|---:|"]
    for circuit in circuits:
        baseline = row(circuit, "balanced", "SM-50")
        for mode in ["manage", "balanced", "attack"]:
            r = row(circuit, "balanced", "SM-50", "" if mode == "balanced" else "/" + mode)
            if r["checks"]:
                lines.append(f"| {circuit} | {mode} | failure | — | — | — | — |")
                continue
            m = driving_metrics(r)
            lines.append(f"| {circuit} | {mode} | {time_s(r):.3f} | {time_s(r)-time_s(baseline):+.3f} | "
                         f"{m['min_power']:.3f} | {m['derated_s']:.1f} | {m['errors']} |")

    lines += ["", "## Timing and slider response", "",
              "Each contrast keeps eight points. Slider variants change downforce and gearing together; differences do not isolate a single coefficient.", "",
              "| Circuit | SM early minus midpoint s | SM late minus midpoint s | Low drag/long gear minus default SM s | High downforce/short gear minus default SM s |",
              "|---|---:|---:|---:|---:|"]
    for circuit in circuits:
        base_time = time_s(row(circuit, "balanced", "SM-50"))
        comparisons = [row(circuit, "balanced", "SM-early"), row(circuit, "balanced", "SM-late"),
                       row(circuit, "low_drag_long", "SM-50"), row(circuit, "high_downforce_short", "SM-50")]
        values = [f"{time_s(r)-base_time:+.3f}" if not r["checks"] else "failure" for r in comparisons]
        lines.append(f"| {circuit} | {' | '.join(values)} |")

    lines += ["", "## Fresh-set response in the fixed balanced SM plan", "",
              "First lap after service and subsequent mean surface temperatures. A low near-peak occupancy is a reported physical response, not an automatic failure. "
              "It combines load, tire preparation and model time constants; no display remapping or imposed target is applied.", "",
              "| Circuit | First new-set lap | First-lap surface °C | First-lap peak-window time % | Next two surface means °C |",
              "|---|---:|---:|---:|---|"]
    for circuit in circuits:
        r = row(circuit, "balanced", "SM-50")
        if r["checks"]:
            continue
        stop = r["case"]["strategy"]["pit_laps"][0]
        ls = r["data"]["laps"][stop:stop + 3]
        next_temperatures = ", ".join(format(l["surface_mean_c"], ".1f") for l in ls[1:])
        lines.append(f"| {circuit} | {ls[0]['lap']} | {ls[0]['surface_mean_c']:.1f} | {100*ls[0]['peak_window_fraction']:.1f} | {next_temperatures} |")

    lines += ["", "## Reserved seed", "",
              "No tuning on this seed. A one-seed repeatability test is distinct from stochastic robustness.", "",
              "| Circuit | Reserved minus primary total s | Reserved recoverable errors |",
              "|---|---:|---:|"]
    for circuit in circuits:
        primary, reserve = row(circuit, "balanced", "SM-50"), row(circuit, "balanced", "SM-50", "/reserved-seed")
        if not reserve["checks"]:
            lines.append(f"| {circuit} | {time_s(reserve)-time_s(primary):+.3f} | {driving_metrics(reserve)['errors']} |")

    lines += ["", "## Decision and limits", "",
              f"- Selected repeat and recomputed-invariant checks: {sum(v['exact'] for v in report['validations'])}/{len(report['validations'])} exact.",
              f"- Three-stop counterexamples beating the sampled one/two-stop recipes: {len(three_stop_counters)}. " + (", ".join(three_stop_counters) if three_stop_counters else "None in this matrix; not a proof against other three-stop schedules."),
              f"- The sampled0440 setup dominates the two other core profiles on every circuit: {concentrated_dominates}. "
              "This is a narrow setup test, not an exclusion of all concentrated configurations.",
              "- Different circuit temperatures and compound responses are accepted. Do not retune simply to put every setup inside a common temperature band.",
              "- No player win probability or nine-AI competitiveness claim: only one car is solved. No new game runtime, physics/catalog admission, provider call, verification submission or deployment.",
              f"- Native source commit: `{report['provenance']['source_commit']}`; binary SHA256 `{report['provenance']['identity']['binary_sha256']}`.",
              f"- Frozen operating-policy file SHA256: `{plan['policy_sha256']}`. Full pin, cases, receipts, per-lap scalars and failure records are retained in the compressed dataset."]
    if failures:
        lines += ["", "### Retained failures", ""] + [f"- {r['case']['id']}: {r['checks']} / {r['data']['outcome']}" for r in failures]
    if failed_validations:
        lines += ["", "### Equality checks requiring investigation", ""] + [f"- {v['kind']} / {v['baseline_id']}" for v in failed_validations]
    if args.gear_controls:
        controls = load(args.gear_controls)
        if not controls["complete"]:
            raise SystemExit("Incomplete gear controls")
        lines += ["", "## Legal gear-only control", "",
                  "Post-plan extension, frozen before its runs: original paired sliders confound gearing with downforce, "
                  "and baseline era-I downforce is locked in the game.18additional SM runs keep downforce.5 and cover "
                  "all three point profiles/circuits at gear.35/.85. No coefficients or primary cases changed. "
                  "This checks the confound without choosing favorable preparations from observed times.", "",
                  "| Circuit | Allocation profile | Gear .35 minus .5 s | Gear .85 minus .5 s |",
                  "|---|---|---:|---:|"]
        for circuit in circuits:
            for setup in plan["setups"]:
                baseline = row(circuit, setup, "SM-50")
                short = next(r for r in controls["rows"] if r["case"]["id"] == baseline["case"]["id"] + "/gear-short")
                long = next(r for r in controls["rows"] if r["case"]["id"] == baseline["case"]["id"] + "/gear-long")
                values = [f"{time_s(r)-time_s(baseline):+.3f}" if not r["checks"] else "failure" for r in [short, long]]
                lines.append(f"| {circuit} | {setup} | {' | '.join(values)} |")
        lines += ["", "### Best sampled preparation using available career controls", "",
                  "Balanced/primary seed, downforce.5, one/two stops. Includes original timings and gear-only controls; "
                  "does not optimize all integer allocations, ratios or pit windows.", "",
                  "| Circuit | Preparation / recipe | Gear | Total s |",
                  "|---|---|---:|---:|"]
        for circuit in circuits:
            eligible = [r for r in rows + controls["rows"] if r["case"]["circuit"] == circuit
                        and r["case"]["mode"] == "balanced" and r["case"]["seed"] == plan["seed"]
                        and r["case"]["downforce"] == .5 and len(r["case"]["strategy"]["pit_laps"]) in (1, 2)]
            winner = best(eligible)
            if winner:
                lines.append(f"| {circuit} | {' / '.join(winner['case']['id'].split('/')[1:])} | {winner['case']['gear']} | {time_s(winner):.3f} |")
    args.output.write_text("\n".join(lines) + "\n")
    print(args.output)


if __name__ == "__main__":
    main()
