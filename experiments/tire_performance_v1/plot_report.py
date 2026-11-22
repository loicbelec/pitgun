#!/usr/bin/env python3
"""Scientific figures derived from frozen profile resources and screen receipts."""
import json
import pathlib
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
import numpy as np
from kernel import EXP, ROOT, key, load

def main():
 out=EXP/'figures';out.mkdir(exist_ok=True);profiles=load(EXP/'profile-candidates-v1.json')['profiles'];summary=load(EXP/'results/summary-v1.json');chosen=summary['profile']
 plt.rcParams.update({'font.size':10,'axes.spines.top':False,'axes.spines.right':False,'svg.hashsalt':'pitgun-tire-performance-v1'})
 w=np.linspace(0,1,401);fig,axes=plt.subplots(1,2,figsize=(12,4.8),layout='constrained')
 raw=load(ROOT/'catalogs/racing/v1.8.1/simulation/tires/soft.json');axes[0].plot(w*100,np.maximum(raw['wear_min'],1-raw['wear_grip_k']*w),label='Loi héritée · tendre',color='#a18c7c',linestyle='--')
 for name,color in [('progressive2','#9ba2a0'),('progressive4','#416c78'),('progressive8','#b66e50')]:
  c=profiles[name]['compounds']['soft']['wear_curve'];axes[0].plot(w*100,1-c['linear_loss']*w-c['late_life_loss']*w**c['late_life_power'],label=name+(' · retenu' if name==chosen else ''),color=color,linewidth=2 if name==chosen else 1.3)
 axes[0].set(xlabel='Usure brute du modèle (%)',ylabel='Facteur direct d’adhérence',xlim=(0,100),ylim=(.38,1.02),title='Perte faible au début, puis progressive');axes[0].legend(frameon=False,fontsize=9)
 dt=np.linspace(-40,40,401);base=load(ROOT/'experiments/career_physics_v1/profile-v1.json')['compounds']['soft'];width=profiles[chosen]['compounds']['soft']['temperature_width_c'];axes[1].plot(dt,np.maximum(base['minimum_temperature_grip_factor'],np.exp(-(dt/base['temperature_width_c'])**2)),color='#a18c7c',linestyle='--',label='Fenêtre héritée · tendre');axes[1].plot(dt,np.maximum(base['minimum_temperature_grip_factor'],np.exp(-(dt/width)**2)),color='#416c78',label='Candidat · tendre');axes[1].fill_between([-20,20],.97,1,color='#d5e2d8',label='Repère de conception FIA 2025');axes[1].set(xlabel='Écart à l’optimum (°C)',xlim=(-40,40),ylim=(.6,1.02),title='Fenêtre thermique explicite et commune');axes[1].legend(frameon=False,fontsize=9,loc='lower center');fig.suptitle('Pneus : hypothèses du modèle réduit à qualifier · aucun chrono corrigé après calcul')
 fig.savefig(out/'performance-curves-v1.png',dpi=170);fig.savefig(out/'performance-curves-v1.svg',metadata={'Date':None});plt.close(fig)
 selected=next(d for d in summary['profiles'] if d['profile']==chosen);contexts=selected['contexts'];fig,ax=plt.subplots(figsize=(10,4.7),layout='constrained');x=np.arange(len(contexts));margins=[c['extra_stops_margin_s'] for c in contexts];bars=ax.bar(x,margins,color='#416c78',width=.6)
 ax.axhline(0,color='#5e6566',linewidth=1);ax.set_xticks(x,[f'{c["context"][1]}\nÈre {c["context"][0]}' for c in contexts]);ax.set_ylabel('Temps 3+ arrêts − temps 1/2 arrêts (s)');ax.set_title('Le coût des arrêts supplémentaires dépasse le gain de pneus neufs\nComparaison des meilleurs plans échantillonnés · coût de stand inchangé à 22 s')
 for b,c in zip(bars,contexts):ax.text(b.get_x()+b.get_width()/2,b.get_height()+max(margins)*.02,f'+{c["extra_stops_margin_s"]:.1f} s\nPlan légal : {len(c["best_legal_case"]["strategy"]["pit_laps"])} arrêt(s)',ha='center',va='bottom',fontsize=9)
 ax.set_ylim(0,max(margins)*1.3);fig.savefig(out/'stop-incentives-v1.png',dpi=170);fig.savefig(out/'stop-incentives-v1.svg',metadata={'Date':None});plt.close(fig)
 rows=load(EXP/'results/profile-screen-v1.json.gz');bykey={key(r['case']):r for r in rows};fig,axes=plt.subplots(1,3,figsize=(13.5,4.5),layout='constrained')
 for c,color in zip(contexts,['#416c78','#b66e50','#7b8982','#c3a665']):
  r=bykey[key(c['best_legal_case'])];laps=r['data']['lap_metrics'];label=f'Ère {c["context"][0]} · {c["context"][1]}'
  # Field names are checked against emitted lap metrics rather than invented telemetry.
  xs=[l['lap'] for l in laps]
  axes[0].plot(xs,[l['surface_end_c'] for l in laps],label=label,color=color);axes[1].plot(xs,[l['wear_end']*100 for l in laps],color=color);axes[2].plot(xs,[l['wear_grip_end'] for l in laps],color=color)
 axes[0].set(ylabel='Température surface en fin de tour (°C)',xlabel='Tour',title='Équilibre dépendant du circuit');axes[0].legend(frameon=False,fontsize=8);axes[1].set(ylabel='Usure brute (%)',xlabel='Tour',title='Usure conservée, remise à zéro au stand');axes[2].set(ylabel='Facteur direct d’adhérence',xlabel='Tour',title='Effet de l’usure sur la performance');fig.suptitle('Meilleurs plans légaux testés · quatre contextes, mêmes paramètres pneus')
 fig.savefig(out/'stint-evolution-v1.png',dpi=170);fig.savefig(out/'stint-evolution-v1.svg',metadata={'Date':None});plt.close(fig)
 for p in out.glob('*.svg'):p.write_text('\n'.join(x.rstrip() for x in p.read_text().splitlines())+'\n')
 print(out)

if __name__=='__main__':main()
