#!/usr/bin/env python3
"""Audit retained evidence; render compact field/player outcomes without new fits."""
import argparse
from collections import defaultdict
import statistics
from pathlib import Path
from run import checks,load,sha,write


def audit(training,selection,validation=None,decision=None):
    assert training['complete'] and training['phase']=='training'
    assert len(training['rows'])==519 and len(training['repeats'])==18 and len(training['bridges'])==20
    rows=training['rows']+[r['fresh']for r in training['repeats']]
    assert all(r['checks']==checks(r['data']) for r in rows)
    assert all(b['exact_input'] and b.get('exact_result',True)for b in training['bridges'])
    before={r['case']['id']:r for r in training['rows']}
    for repeat in training['repeats']:
        a=before[repeat['baseline_id']];b=repeat['fresh']
        exact=not a['checks'] and not b['checks'] and a['data']['outcome']['result_digest']==b['data']['outcome'].get('result_digest') and a['data']['receipt']['physical_input_digest']==b['data'].get('receipt',{}).get('physical_input_digest')
        assert exact==repeat['exact']
    if validation:
        assert selection['training_pass'] and validation['complete'] and len(validation['rows'])==66
        assert validation['policy']==selection['policy']
        assert all(r['checks']==checks(r['data'])for r in validation['rows'])
        rows+=validation['rows']
    return {'full_gp':len(rows),'accepted_laps':sum(len(r['data'].get('laps',[]))for r in rows),
        'numerical_failures':sum(bool(r['checks'])for r in rows),'exact_repeats':sum(r['exact']for r in training['repeats']),
        'input_bridges':len(training['bridges']),'full_prior_bridges':sum('exact_result'in b for b in training['bridges']),
        'native_wall_s':sum(r['wall_s']for r in rows),'training_pass':selection['training_pass'],
        'validation_pass':decision['validation_pass']if decision else None,'release_admitted':False}


def main():
    p=argparse.ArgumentParser();p.add_argument('--training',type=Path,required=True);p.add_argument('--selection',type=Path,required=True);p.add_argument('--validation',type=Path);p.add_argument('--decision',type=Path);p.add_argument('--output',type=Path,required=True);args=p.parse_args()
    t=load(args.training);s=load(args.selection);v=load(args.validation)if args.validation else None;d=load(args.decision)if args.decision else None
    assert sha(args.training)==s['training_archive_sha256']
    if d:assert sha(args.selection)==d['selection_sha256']
    metrics=audit(t,s,v,d);write(args.output.with_suffix('.json'),metrics)
    lines=['# Fixed nine on rolling-heat-065 — native results','',str(metrics),'',
        '## Actual career exit comparisons','', '| Era | Circuit | Effective cap | Informed player | Concentrated player | Routine player |','|---|---|---:|---:|---:|---:|']
    totals=defaultdict(list)
    for ctx in t['summaries']:
        lines.append(f'| {ctx["era"]} | {ctx["circuit"]} | {ctx["exit_budget"]} | P{ctx["player"]["informed"]["rank"]} | P{ctx["player"]["concentrated"]["rank"]} | P{ctx["routine"]["rank"]} |')
        for actor,time in ctx['field_times_s'].items():totals[actor].append(1+sum(x<time for x in ctx['field_times_s'].values()))
    lines+=['','## Roster outcomes','', '| Driver | Mean rank among nine, ten exit contexts |','|---|---:|']
    for actor,ranks in sorted(totals.items(),key=lambda x:(statistics.mean(x[1]),x[0])):lines.append(f'| {actor} | {statistics.mean(ranks):.2f} |')
    if v:
        lines+=['','## Reserved budgets/seeds','', '| Era | Circuit | Effective cap | Seed | Informed player | Concentrated player |','|---|---|---:|---:|---:|---:|']
        for ctx in v['summaries']:lines.append(f'| {ctx["era"]} | {ctx["circuit"]} | {ctx["budget"]} | {ctx["seed"]} | P{ctx["player"]["informed"]["rank"]} | P{ctx["player"]["concentrated"]["rank"]} |')
    lines+=['','These are independent native elapsed-car comparisons and bounded sampled preparations. No traffic, all-era coverage, global setup optimality, WASM equality or runtime admission is claimed. The current browser preview remains unchanged. Actual entry player equipment is kept; reserved scaled players are explicitly equal-field-equipment diagnostics. See ACCEPTANCE.md and retained receipts for fuel, driver, component, budget, strategy and source bindings.','']
    args.output.write_text('\n'.join(lines))

if __name__=='__main__':main()
