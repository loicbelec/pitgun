#!/usr/bin/env python3
"""Scientific summary of the pinned exploratory screen, no parameter fitting."""
import json,pathlib
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
HERE=pathlib.Path(__file__).resolve().parent
report=json.loads((HERE/'results/screen-v1.json').read_text())
rows={(r['case'],r['candidate']):r['data'] for r in report['results']}
fig,axs=plt.subplots(2,2,figsize=(12,8.5),constrained_layout=True)
colors=['#b56345','#287f91','#505d65']
ax=axs[0,0]
for i,c in enumerate([0,1,2]):
 r=rows[(f'budget-early8-monza-c{c}',True)]
 ax.plot([l['lap'] for l in r['laps']],[l['engine_min_max_c'][1] for l in r['laps']],marker='o',label=f'Cooling {c}; engine {6-c}',color=colors[i])
ax.axhline(110,color='#555555',ls=':',label='Derating threshold')
ax.set(title='Era I / Monza: equal effective budget 8',xlabel='Lap',ylabel='Peak engine temperature (°C)');ax.legend(fontsize=8)
ax=axs[0,1]
labels=[];values=[]
for name,label,c in [('early8-monza','V8 1960\nMonza',2),('classic15-monza','V8 1970\nMonza',2),('classic15-montreal','V8 1970\nMontreal',2),('modern20-monza','V6T\nMonza',2),('modern20-silverstone','V6T\nSilverstone',2),('modern20-barcelona','V6T\nBarcelona',2)]:
 labels.append(label);values.append(rows[(f'budget-{name}-c0',True)]['outcome']['time_s']-rows[(f'budget-{name}-c{c}',True)]['outcome']['time_s'])
ax.bar(labels,values,color='#287f91');ax.set(title='Transfer two engine points to cooling',ylabel='Time gained over 5 laps (s)');ax.tick_params(axis='x',labelsize=8)
for i,v in enumerate(values):ax.text(i,v+.15,f'{v:.2f}',ha='center',fontsize=8)
ax.set_ylim(0,max(values)*1.18)
ax=axs[1,0]
r=rows[('tires-medium',True)]
x=[0]+[l['lap'] for l in r['laps']]
ax.plot(x,[r['resolved_parameters']['initial_state']['tire_temp']]+[l['surface_end_c'] for l in r['laps']],color='#287f91',label='V2 surface')
ax.plot(x,[r['resolved_parameters']['thermal_physics_v2']['initial_carcass_temperature_c']]+[l['carcass_end_c'] for l in r['laps']],color='#b56345',ls='--',label='V2 carcass')
old=rows[('tires-medium',False)]
ax.plot(x,[old['resolved_parameters']['initial_state']['tire_temp']]+[l['surface_end_c'] for l in old['laps']],color='#505d65',ls=':',label='Energy V1 aggregate')
ax.set(title='Barcelona / medium: distinct thermal states',xlabel='Lap',ylabel='End-of-lap temperature (°C)');ax.legend(fontsize=8)
ax=axs[1,1]
def cumulative(v):
 out=[0.0]
 for l in v:out.append(out[-1]+l['time_s'])
 return out
hard=cumulative(rows[('tires-hard',True)]['laps'])
for i,n in enumerate(['soft','medium','hard']):
 t=cumulative(rows[('tires-'+n,True)]['laps']);ax.plot(range(21),[a-b for a,b in zip(t,hard)],color=colors[i],label=n)
ax.axhline(0,color='#777777',lw=.5);ax.set(title='Same 20-lap stint: short / long tradeoff',xlabel='Lap',ylabel='Cumulative time versus hard (s)');ax.legend(fontsize=8)
for ax in axs.flat:ax.grid(axis='y',alpha=.15);ax.spines[['top','right']].set_visible(False)
fig.suptitle('Pitgun thermal V2 — native exploratory screen\nCalibration anchors; no experimental or competitive acceptance',fontsize=13)
out=HERE/'figures';out.mkdir(exist_ok=True)
fig.savefig(out/'thermal-v2-screen.png',dpi=180)
print(out/'thermal-v2-screen.png')
