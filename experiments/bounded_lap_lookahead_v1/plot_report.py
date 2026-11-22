#!/usr/bin/env python3
"""Render only frozen native seam metrics and selected native-point excerpts."""
import html
import json
from pathlib import Path
import sys
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt

HERE=Path(sys.argv[1]).resolve() if len(sys.argv)>1 else Path(__file__).resolve().parent
report=json.loads((HERE/'results/screen-v1.json').read_text())
plan=json.loads((HERE/'plan-v1.json').read_text())
out=HERE/'figures';out.mkdir(exist_ok=True)
plt.rcParams.update({'font.size':9,'axes.spines.top':False,'axes.spines.right':False,'figure.facecolor':'#f4f0e6','axes.facecolor':'#faf8f2','savefig.facecolor':'#f4f0e6'})
colors=['#747d80','#24515c']
labels=['Reference: global worst grip','Candidate: reachable wear']
rows=report['results']
def row(id,mode):return rows[id+':'+mode]
def decorate(ax):ax.grid(alpha=.13)
for scenario in plan['scenarios']:
    id=scenario['id'];fig,axs=plt.subplots(2,2,figsize=(12,7.2),constrained_layout=True)
    for i,mode in enumerate(plan['modes']):
        r=row(id,mode);ls=r['laps'];x=[l['lap'] for l in ls]
        axs[0,0].plot(x,[l['end_driving_speed_mps']*3.6 for l in ls],marker='.',lw=1,color=colors[i],label=labels[i])
        axs[0,1].plot(x,[l['tail_300m']['friction_brake_work_j']/1000 for l in ls],marker='.',lw=1,color=colors[i],label=labels[i])
        if r['tail_traces']:
            p=r['tail_traces'][0]['points'];axs[1,0].plot([v[0] for v in p],[v[1] for v in p],lw=1.4,color=colors[i],label=labels[i])
    axs[0,0].set(title='Driving speed at the line (before service)',xlabel='Lap',ylabel='Speed (km/h)')
    axs[0,1].set(title='Actual friction-brake work: final 300 m',xlabel='Lap',ylabel='Work (kJ)')
    axs[1,0].set(title='First lap: final 500 m of native trajectory',xlabel='Distance before the line (m)',ylabel='Speed (km/h)')
    candidate=row(id,'reachable_wear_v1');work=[l for l in candidate['work']['laps'] if not l['reachable']['service_reset'] and l['reachable']['certificate_checks']]
    x=[l['lap'] for l in work];states=[l['reachable'] for l in work]
    axs[1,1].plot(x,[w['certificate_cap_mps']*3.6 for w in states],ls='--',color='#ae573f',label='Fresh next-start certificate')
    axs[1,1].plot(x,[w['committed_end_speed_mps']*3.6 for w in states],marker='.',color=colors[1],label='Committed end speed')
    axs[1,1].plot(x,[w['anchor_cap_mps']*3.6 for w in states],ls=':',color=colors[0],label='Conservative far-horizon anchor')
    axs[1,1].set(title='Certificate checked before publishing the lap',xlabel='Non-service lap',ylabel='Speed (km/h)')
    for ax in axs.flat:decorate(ax);ax.legend(fontsize=7)
    status='Completed' if candidate['outcome']['ok'] else 'FAILED — accepted prefix only'
    fig.suptitle(f'{id} · {status}\nFixed physics coefficients; native opt-in candidate. Final lap has no next-lap constraint.',fontsize=11)
    fig.savefig(out/f'{id}.png',dpi=130);plt.close(fig)

chosen=['reference-budget-early8-monza-c0','reference-budget-classic15-monza-c2','reference-tires-medium']
fig,axs=plt.subplots(2,3,figsize=(14,7.6),constrained_layout=True)
for j,id in enumerate(chosen):
    for i,mode in enumerate(plan['modes']):
        r=row(id,mode);ls=r['laps'];p=r['tail_traces'][0]['points']
        axs[0,j].plot([l['lap'] for l in ls],[l['end_driving_speed_mps']*3.6 for l in ls],marker='.',color=colors[i],label=labels[i],lw=1)
        axs[1,j].plot([v[0] for v in p],[v[1] for v in p],color=colors[i],label=labels[i],lw=1.2)
    axs[0,j].set(title=['Early V8 · Monza 5 laps','Classic V8 · Monza 5 laps','Modern · Barcelona 20 laps'][j],xlabel='Lap',ylabel='Line speed (km/h)')
    axs[1,j].set(title='First lap / native final 500 m',xlabel='Distance before line (m)',ylabel='Speed (km/h)')
for ax in axs.flat:decorate(ax);ax.legend(fontsize=7)
fig.suptitle('Pitgun — bounded lap-boundary forecast\nNative feasibility / continuity qualification; a conservative temperature floor remains',fontsize=12)
fig.savefig(out/'overview.png',dpi=150);plt.close(fig)
options=''.join(f'<option value="{html.escape(s["id"])}">{html.escape(s["id"])}</option>' for s in plan['scenarios'])
table=[]
for s in plan['scenarios']:
    a,b=[row(s['id'],m) for m in plan['modes']]
    times=[f"{r['outcome']['time_s']:.3f}" if r['outcome']['ok'] else 'Failed' for r in [a,b]]
    worst=max(b[k] for k in ['maximum_speed_jump_mps','maximum_engine_temperature_jump_c','maximum_tire_surface_jump_c','maximum_tire_carcass_jump_c','maximum_wear_jump_fraction'])
    fallback=sum(l['reachable']['conservative_recompute'] for l in b['work']['laps'])
    table.append(f'<tr><td>{html.escape(s["id"])}</td><td>{times[0]}</td><td>{times[1]}</td><td>{"0 (exact)" if worst==0 else "FAILED"}</td><td>{fallback}</td></tr>')
page='''<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>Pitgun · Lap boundaries</title>
<style>body{margin:0;background:#f4f0e6;color:#18343d;font:15px system-ui,sans-serif}main{max-width:1150px;margin:auto;padding:24px}h1{font-size:28px}h2{font-size:20px}p{line-height:1.5;max-width:950px}strong{color:#a34e35}img{display:block;width:100%;height:auto;border-radius:8px}select{max-width:100%;padding:9px;background:#fff;border:1px solid #24515c;border-radius:5px;color:#18343d;font:inherit;margin:8px 0}table{border-collapse:collapse;width:100%;font-size:12px}td,th{text-align:right;padding:7px;border-bottom:1px solid #cbd1ca;white-space:nowrap}td:first-child,th:first-child{text-align:left}thead{background:#dce5df}.scroll{overflow:auto}a{color:#24515c}.note{font-size:13px}</style><main>
<h1>Pitgun · Lap-boundary qualification</h1><p><strong>Native experimental candidate. No game activation.</strong> Future wear is bounded from the actual tire state. End speed is checked against a fresh braking certificate before the current lap is published. Native speed and thermal state carry exactly; accepted history is never rewritten.</p>
<img src="figures/overview.png" alt="Reference and candidate line speeds and end-of-lap trajectories">
<h2>Inspect a recorded scenario</h2><select id="scenario" aria-label="Scenario">OPTIONS</select><img id="detail" src="figures/reference-budget-early8-monza-c0.png" alt="Line speed, braking work, native trace and terminal certificate">
<p class="note">The final lap has no next-lap horizon; a different finish-line speed is not by itself evidence of a bug. The candidate still uses a conservative thermal-grip floor. Pit lane motion remains an abstract, ledger-accounted reset. Surface, carcass and wear enclosures are checked on native points, not on these selected graphical excerpts.</p>
<h2>Matched inputs, different forecast algorithms</h2><p class="note">Totals include the declared pit service. Time differences are model-trajectory changes, not proof of real-world accuracy or a pure compute optimization. “Exact carry” compares speed/engine/surface/carcass/wear to the actual post-service state. Fallbacks recompute only uncommitted work; they do not clip speed.</p>
<div class="scroll"><table><thead><tr><th>Scenario</th><th>Reference total (s)</th><th>Candidate total (s)</th><th>Carry jumps</th><th>Conservative recomputes</th></tr></thead><tbody>TABLE</tbody></table></div>
<p><a href="README.md">Method and assumptions</a> · <a href="RESULTS_V1.md">Results and limitations</a> · <a href="results/screen-v1.json">Frozen receipt</a></p><p class="note">Source SOURCE. This page reads recorded native data; it does not execute the candidate in your browser.</p></main><script>document.getElementById('scenario').addEventListener('change',function(){document.getElementById('detail').src='figures/'+this.value+'.png';});</script></html>'''
(HERE/'index.html').write_text(page.replace('OPTIONS',options).replace('TABLE',''.join(table)).replace('SOURCE',report['identity']['source_commit']))
print(out/'overview.png')
