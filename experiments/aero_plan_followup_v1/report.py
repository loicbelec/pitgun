#!/usr/bin/env python3
"""Read-only comparison of the separately frozen SM and MH2 diagnostics."""
import argparse
import gzip
import json
from pathlib import Path
from run import ROOT, analyze, metric, sha, write


def main():
    parser=argparse.ArgumentParser();parser.add_argument('input',type=Path);parser.add_argument('--output-dir',type=Path,required=True);args=parser.parse_args()
    data=json.load(gzip.open(args.input));result=analyze(data)
    original_path='experiments/aero_tradeoff_audit_v1/results/main-v1.json.gz'
    assert sha(ROOT/original_path)==data['plan']['retained_files'][original_path]
    original=json.load(gzip.open(ROOT/original_path));sm={r['case']['id']:r for r in original['rows']}
    lines=['# Conditional aero response — matched-MH two-stop follow-up','',
        f"Numerical PASS: **{result['numerical_pass']}**. Separate {result['main_gp']} main GP +{result['fresh_repeats']} fresh repeats, "
        f"{result['accepted_laps']} laps; {result['exact_repeats']} exact repeats and three exact prior qualification bridges.",'',
        'This exploratory follow-up was registered after the original48-GP audit was closed. It is not a held-out confirmation. '
        'Same car, means/bonuses, driver, seed, gear.5, fuel100kg, solver and thermal coefficients. '
        'The changed strategy includes different compounds **and** stint/service timing; no pure material or additive thermal-seconds attribution.','',
        '## Additional downforce at fixed aerodynamic drag area','',
        '| Circuit | SM1 first-five delta s | SM1 full-GP delta s | MH2 first-five delta s | MH2 full-GP delta s | MH2 surface delta °C |',
        '|---|---:|---:|---:|---:|---:|']
    for r in result['summaries']:
        c=r['circuit'];a=r['baseline'];b=r['extra_downforce'];s0=metric(sm[f'{c}/coupled/0.5']);s1=metric(sm[f'{c}/downforce_only/0.8'])
        lines.append(f"| {c} | {s1['first_five_s']-s0['first_five_s']:+.3f} | {s1['total_s']-s0['total_s']:+.3f} | {b['first_five_s']-a['first_five_s']:+.3f} | {b['total_s']-a['total_s']:+.3f} | {b['surface_mean_c']-a['surface_mean_c']:+.3f} |")
    lines+=['','## Ordinary coupled slider response on MH2','',
        '| Circuit | Slider0 total s | Slider.5 total s | Slider1 total s | Minimum minus midpoint s |', '|---|---:|---:|---:|---:|']
    for r in result['summaries']:
        t=r['coupled'];lines.append(f"| {r['circuit']} | {t['0.0']['total_s']:.3f} | {t['0.5']['total_s']:.3f} | {t['1.0']['total_s']:.3f} | {t['0.0']['total_s']-t['0.5']['total_s']:+.3f} |")
    lines+=['','## Decision','',
        'Extra downforce is useful over the full adapted MH2 race at held drag area on all three contexts. '
        'With the ordinary coupled slider, midpoint beats minimum at Silverstone/São Paulo, while Monza favors minimum among the three tested values. '
        'There is no need to manufacture a downforce benefit: it is present and conditional on tire/strategy state. '
        'Three samples do not locate an exact optimum; no coefficient change or complete aero/industrial identification follows.','',
        'Next: jointly screen aero and tire/stint choices in bounded budget-aware opponent preparation, keeping physics fixed. '
        'The broader #469 marginal-point/gearing questions remain separate. Native/WASM/recovery, admission and owner playtest still precede deployment.']
    args.output_dir.mkdir(parents=True,exist_ok=True);write(args.output_dir/'DECISION_V1.json',result)
    (args.output_dir/'RESULTS_V1.md').write_text('\n'.join(lines)+'\n')
    print(json.dumps({'numerical_pass':result['numerical_pass'],'exact_repeats':result['exact_repeats'],'laps':result['accepted_laps']}))


if __name__=='__main__':main()
