#!/usr/bin/env python3
"""Read-only analysis of the frozen era-II matrix; never fits coefficients."""
import argparse
import gzip
import json
from pathlib import Path


def main():
    parser=argparse.ArgumentParser();parser.add_argument('input',type=Path);parser.add_argument('--output',type=Path,required=True);args=parser.parse_args()
    d=json.loads(gzip.decompress(args.input.read_bytes()));assert d['complete']
    p=d['plan'];rows=d['rows'];lookup={r['case']['id']:r for r in rows};validations=d['validations']
    failed=[r for r in rows if r['checks']];passed=not failed and all(v['exact']for v in validations) and len(validations)==6
    def time(r):return r['data']['outcome'].get('time_s',float('inf')) if not r['checks'] else float('inf')
    def best(rs):return min((r for r in rs if not r['checks']),key=time,default=None)
    def group(c,stage,setup):return [lookup[f'{c}/{stage}/{setup}/{recipe}']for recipe in p['normal_plans']]
    def core(c,stage):return [r for setup in p['stages'][stage]['setups']for r in group(c,stage,setup)]
    lines=['# Era-II qualification — actual career technology and bonus floors','','Tracking #483 / #479 / draft#481. Same frozen three-compound policy as era I. No tire, engine, driver or cooling coefficients fitted here.','',f'Numerical gate: **{"PASS" if passed else "REFINE"}**. {len(rows)} full-distance cases and {len(validations)} selected repeats; {len(failed)} physical/check failures, {sum(not v["exact"] for v in validations)} non-exact repeats.','','## Actual means','','The two season programmes each have six rounds; their union contains seven regular circuits. The matrix covers that union, not invitations/Monaco. Allocation arrays below are discretionary; bonuses are then added once in Rust.','','| Stage | Discretionary | A/C/E/K bonuses | Effective budget | Engine | Aero / downforce |','|---|---:|---|---:|---|---|']
    for name,s in p['stages'].items():lines.append(f'| {name} | {s["discretionary_points"]} | {s["offsets"]} | {s["discretionary_points"]+sum(s["offsets"])} | {s["engine_id"]} | {s["aero_id"]} / {"locked .5" if s["downforce_locked"] else "adjustable"} |')
    lines+=['','Entry/exit timing differences combine earned budget and installed technology. They do not identify an isolated engine-architecture effect. Zero discretionary cooling leaves Cooling1 at exit. Same driver reference traits, initial110kg and paired primary seed; reserved seed and modes are explicit.','','## Best sampled normal preparation at default sliders','','Three point profiles xthree normal recipes per stage/circuit. These are bounded candidates, not global optima or opponent-win probabilities. All totals include catalog service loss.','','| Circuit | Entry preparation / plan | Entry total s | Exit preparation / plan | Exit total s |','|---|---|---:|---|---:|']
    for ctx in p['contexts']:
        c=ctx['circuit'];a=best(core(c,'entry'));b=best(core(c,'exit'))
        if a and b:lines.append(f'| {c} | {" / ".join(a["case"]["id"].split("/")[2:])} | {time(a):.3f} | {" / ".join(b["case"]["id"].split("/")[2:])} | {time(b):.3f} |')
    lines+=['','## Discretionary cooling tradeoff at the same means','','Best of the same three normal plans for each profile, same .5/.5 sliders. Negative deltas are legitimate when inherited cooling/engine design makes additional discretionary cooling unhelpful in this sample.','','| Circuit | Stage | Balanced total s | No discretionary Cooling total s | Cooling-oriented total s | No discretionary Cooling minus best other s |','|---|---|---:|---:|---:|---:|']
    for ctx in p['contexts']:
        c=ctx['circuit']
        for stage in p['stages']:
            rs=[best(group(c,stage,s))for s in p['stages'][stage]['setups']]
            if all(rs):
                ts=list(map(time,rs));lines.append(f'| {c} | {stage} | {ts[0]:.3f} | {ts[1]:.3f} | {ts[2]:.3f} | {ts[1]-min(ts[0],ts[2]):+.3f} |')
    lines+=['','## Stop-count controls on the same exit/balanced preparation','','Zero/three-stop cases are physical diagnostics and do not change game/leaderboard stint rules. One/two-stop cells use only the sampled normal recipes.','','| Circuit | Best 0-stop total s | Best 1-stop total s | Sampled 2-stop total s | Sampled 3-stop total s | Three beats sampled one/two |','|---|---:|---:|---:|---:|---|']
    counters=[]
    for ctx in p['contexts']:
        c=ctx['circuit'];zero=best([lookup[f'{c}/exit/balanced/{k}']for k in ['S0','M0','H0']]);one=best([lookup[f'{c}/exit/balanced/{k}']for k in ['SM-50','MH-50']]);two=lookup[f'{c}/exit/balanced/SMS-thirds'];three=lookup[f'{c}/exit/balanced/SMSM-quarters'];counter=time(three)<min(time(one),time(two)) if one else False
        if counter:counters.append(c)
        if zero and one:lines.append(f'| {c} | {time(zero):.3f} | {time(one):.3f} | {time(two):.3f} | {time(three):.3f} | {counter} |')
    lines+=['','## One-factor controls on the developed car','','Fixed exit/balanced SM midpoint plan. Each slider is varied separately; entry downforce stays locked.','','| Circuit | Gear .35 minus .5 s | Gear .85 minus .5 s | Downforce .2 minus .5 s | Downforce .8 minus .5 s |','|---|---:|---:|---:|---:|']
    for ctx in p['contexts']:
        c=ctx['circuit'];base=lookup[f'{c}/exit/balanced/SM-50'];rs=[lookup[f'{c}/exit/balanced/SM-50/{suffix}']for suffix in ['gear-0.35','gear-0.85','downforce-0.2','downforce-0.8']];vals=[f'{time(r)-time(base):+.3f}'if not r['checks']else'failure'for r in rs];lines.append(f'| {c} | {" | ".join(vals)} |')
    lines+=['','## Thermal response on the balanced SM plan','','Native last-lap surface/carcass means and minimum retained engine capacity. Different circuit temperatures are expected; no uniform operating-band requirement. These aggregate thermal states are not identified experimental tire measurements.','','| Circuit | Stage | Last surface °C | Last carcass °C | Minimum thermal power factor | Final tire wear % |','|---|---|---:|---:|---:|---:|']
    for ctx in p['contexts']:
        c=ctx['circuit']
        for stage in p['stages']:
            r=lookup[f'{c}/{stage}/balanced/SM-50']
            if not r['checks']:
                ls=r['data']['laps'];l=ls[-1];lines.append(f'| {c} | {stage} | {l["surface_mean_c"]:.1f} | {l["carcass_mean_c"]:.1f} | {min(x["minimum_power_fraction"]for x in ls):.3f} | {100*l["wear_end"]:.1f} |')
    lines+=['','## Driver modes and reserved seed','','Three contexts only for mode comparison, fixed exit/balanced SM. Recoverable lapses are not crashes/VSC; these samples do not calibrate the long-run risk/reward tradeoff.','','| Circuit | Mode | Delta Balanced total s | Recoverable lapses |','|---|---|---:|---:|']
    for c in p['driver_mode_contexts']:
        base=lookup[f'{c}/exit/balanced/SM-50']
        for mode in ['manage','attack']:
            r=lookup[f'{c}/exit/balanced/SM-50/{mode}']
            if not r['checks']:lines.append(f'| {c} | {mode} | {time(r)-time(base):+.3f} | {sum(l["driver_lapses"]for l in r["data"]["laps"])} |')
    lines+=['','Reserved seed104729 was fixed before execution; no fitting on it. A single extra seed does not establish broad stochastic robustness.','','## Numerical evidence and decision','','- All actual bonus floors, effective budgets and installed component selections are checked against retained career fixtures.','- Initial preparation, both tire temperatures, wear, speed and engine temperature carry are checked at every native boundary.','- Catalog service time is included in total elapsed and independently checked from per-lap driving times.']
    success=[r for r in rows+[v['fresh']for v in validations]if not r['checks']]
    if success:
        lines += [f'- {len(success)} retained successful GP / {sum(len(r["data"]["laps"])for r in success)} driven laps; {len({r["data"]["receipt"]["physical_input_digest"]for r in rows if not r["checks"]})} distinct main physical inputs.',f'- Maximum carry jump {max(max(r["data"]["maximum_carry_jumps_speed_engine_surface_carcass_wear"])for r in success):.3g}; tire segment energy residual {max(r["data"]["thermal_ledger"]["maximum_tire_balance_residual_j"]for r in success):.3g}J; elapsed residual {max(r["data"]["outcome"]["elapsed_accounting_residual_s"]for r in success):.3g}s.']
    lines += [f'- Selected fresh repeats: {sum(v["exact"]for v in validations)}/{len(validations)} exact physical-input and full-result digests.',f'- Sampled three-stop counterexamples: {counters or "none"}. This does not prove a global optimum or exclude other three-stop schedules.',f'- Native source `{d["provenance"]["source_commit"]}`; executable SHA256 `{d["provenance"]["identity"]["binary_sha256"]}`.',f'- Operating-policy file SHA256 `{p["policy_sha256"]}`. Inputs, per-lap scalars, receipts, failures and complete source/fixture/binary pins are retained.','- Numerical PASS supports proceeding to a separately versioned local browser candidate. It does not admit a leaderboard model or qualify nine-AI fairness, all budgets, native/WASM equality, real tire identification or industrial accuracy.','- Strategic gate remains open: all seven developed-car three-stop controls beat the sampled one/two-stop schedules. Diagnose this before freezing opponents; a local preview is exploratory only.', '- Next: native-to-browser coupling and clear derating/braking display, then owner playtest before final budget-aware imperfect nine-AI preparation. No physical retune or deployment is implied.']
    if failed:lines+=['','### Retained failures','']+[f'- {r["case"]["id"]}: {r["checks"]} / {r["data"]["outcome"]}'for r in failed]
    args.output.write_text('\n'.join(lines)+'\n');print(args.output)


if __name__=='__main__':main()
