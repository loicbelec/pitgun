#!/usr/bin/env python3
"""Render archived native evidence only; does not solve or fit a model."""
import hashlib,json,pathlib
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
HERE=pathlib.Path(__file__).resolve().parent
reports={}
for name in ['local-gameplay-v1','budget-grid-v1','grid-refinement-v1']:
 p=HERE/'results'/f'{name}.json';raw=p.read_bytes();assert p.with_suffix('.sha256').read_text().strip()=='sha256:'+hashlib.sha256(raw).hexdigest();reports[name]=json.loads(raw)
r=reports['local-gameplay-v1'];grid=reports['budget-grid-v1'];fine=reports['grid-refinement-v1'];by={(x['case_id'],x['model']):x['data'] for x in r['results']}
plt.rcParams.update({'font.size':10,'axes.spines.right':False,'axes.spines.top':False})
fig,axes=plt.subplots(2,2,figsize=(12,8),layout='constrained');ax=axes[0,0]
groups=[g['id'] for g in grid['plan']['groups']]
for i,(model,color) in enumerate([('historical','#9da7a8'),('energy','#245b67')]):
 values=[min(next(c['positive_minus_zero_s'] for c in data['comparisons'] if c['group']==g and c['model']==model) for data in [grid,fine]) for g in groups]
 ax.bar([n+(i-.5)*.35 for n in range(len(groups))],values,width=.35,color=color,label=model)
ax.axhline(0,color='#49545b',lw=1);ax.set_xticks(range(len(groups)),['V6T Monza\n20 points','V6T Barcelona\n20 points','V8 Montreal\n15 points','2026 Monza\n40 points']);ax.set_ylabel('Best positive minus best zero cooling (s)');ax.set_title('Coarse grids + local cooling refinement');ax.legend(frameon=False)
ax=axes[0,1]
for prefix,cvals,label,color in [('budget-modern20-monza',[0,2,4,6],'Modern V6T','#245b67'),('budget-classic15-montreal',[0,1,2,4],'Historical V8','#b45d40')]:
 vals=[by[(f'{prefix}-c{c}','energy')]['outcome']['mechanical']['maximum_engine_temperature_c'] for c in cvals]
 ax.plot(cvals,vals,'o-',color=color,label=label)
ax.set_xlabel('Allocated cooling points');ax.set_ylabel('Peak aggregate engine temperature (°C)');ax.set_title('Equal-budget line · points taken from engine');ax.legend(frameon=False)
for tire,color in [('soft','#b44f3b'),('medium','#ad8728'),('hard','#617681')]:
 laps=by[('tires-'+tire,'energy')]['laps'];x=[v['lap'] for v in laps]
 axes[1,0].plot(x,[v['tire_mean_c'] for v in laps],color=color,label=tire)
 axes[1,1].plot(x,[100*v['wear_final'] for v in laps],color=color,label=tire)
legacy=by[('tires-medium','historical')]['laps'];axes[1,0].plot([v['lap'] for v in legacy],[v['tire_mean_c'] for v in legacy],'--',color='#8f9193',label='medium historical')
axes[1,0].axhline(100,ls=':',color='#aaa',label='catalog optimum');axes[1,0].set_ylabel('Native time-weighted tire temperature (°C)');axes[1,0].set_title('Barcelona · 20-lap stint · fixed setup');axes[1,1].set_ylabel('Native wear at lap end (%)');axes[1,1].set_title('Compound wear · same stint conditions')
for ax in axes[1,:]:ax.set_xlabel('Lap');ax.legend(frameon=False)
for ax in axes.flat:ax.grid(axis='y',alpha=.18)
fig.suptitle('Pitgun thermal / tire audit · existing laws, no parameter fitting')
fig.supxlabel('Synthetic native configurations · lower bar means positive cooling is faster · no global optimum or physical validation claim',fontsize=9)
(HERE/'figures').mkdir(exist_ok=True)
fig.savefig(HERE/'figures/gameplay-audit-v1.png',dpi=160);plt.close(fig)
