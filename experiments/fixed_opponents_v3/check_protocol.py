#!/usr/bin/env python3
"""Preflight the selection logic and actual career limits, without new physics."""
import copy
import json

import common as c
from run_local import assess


def main():
    assert len(c.ROSTER["drivers"]) == 9
    assert set(c.ROSTER["front_runners"]) == {"franz_hermann", "charles_leclair"}
    contexts, players, results = {}, [], []
    actors = list(c.ROSTER["front_runners"]) + [a for a in c.ROSTER["drivers"] if a not in c.ROSTER["front_runners"]]
    for context, ctx in c.CONTEXTS.items():
        contexts[context] = []
        for i, actor in enumerate(actors):
            x = c.case(ctx, actor, [0, ctx["base_budget"], 0, 0], .5, .5, c.strategy(ctx["laps"], ["medium", "hard"], [ctx["laps"] // 2]))
            contexts[context].append({"case": x, "data": {"outcome": {"ok": True, "time_s": 100 + i * 2}, "lap_times_s": [100]}})
    for fid, fixture in c.FIXTURES.items():
        for prep in fixture["preparedRequests"]:
            context = (fixture["era"], prep["circuitId"])
            ctx = c.CONTEXTS[context]
            n = fixture["discretionaryPoints"]
            for i, (style, time) in enumerate([("informed", 98), ("sampled_mc", 104), ("fixed_mc", 108), ("routine", 110)]):
                x = c.case(ctx, "player", [0, n-i, i, 0], .5, .5, c.strategy(ctx["laps"], ["medium", "hard"], [ctx["laps"] // 2]), n, "career_player", fid)
                assert c.permitted(x)
                for neighbor in c.local(x, context[0]):
                    assert c.permitted(neighbor)
                players.append({"fixture": fid, "context": list(context), "style": style, "case": x})
                results.append({"case": x, "data": {"outcome": {"ok": True, "time_s": time}, "lap_times_s": [time]}})
    assert len(players) == 72
    assert assess(contexts, players, results)["eligible"]
    # Identical informed/MC performance everywhere cannot meet meaningful-choice target.
    ties = copy.deepcopy(results)
    mc_ids = {p["case"]["id"] for p in players if p["style"] == "sampled_mc"}
    for r in ties:
        if r["case"]["id"] in mc_ids:
            r["data"]["outcome"]["time_s"] = 98
    assert not assess(contexts, players, ties)["eligible"]
    # Already installed II cooling can make concentrated setups equally good in II.
    late_ties = copy.deepcopy(results)
    late_ids = {p["case"]["id"] for p in players if p["style"] == "sampled_mc" and p["fixture"].endswith("era2-exit")}
    for r in late_ties:
        if r["case"]["id"] in late_ids:
            r["data"]["outcome"]["time_s"] = 98
    assert assess(contexts, players, late_ties)["eligible"]
    failures = copy.deepcopy(contexts)
    first = next(iter(failures.values()))[0]
    first["data"]["outcome"] = {"ok": False, "accepted_laps": 0}
    assert not assess(failures, players, results)["eligible"]
    wrong_favorites = copy.deepcopy(contexts)
    for rows in wrong_favorites.values():
        rows[0]["data"]["outcome"]["time_s"] = 130
    assert not assess(wrong_favorites, players, results)["eligible"]
    print(json.dumps({"synthetic_selection_scenarios": 5, "real_preparations_checked": 18, "fixed_driver_count": 9, "all_checks_pass": True}))


if __name__ == "__main__":
    main()
