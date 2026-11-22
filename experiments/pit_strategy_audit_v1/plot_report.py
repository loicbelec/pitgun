#!/usr/bin/env python3
"""Static review figures, derived from completed evidence and actual parameters."""
import json
import pathlib
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
import numpy as np

EXP=pathlib.Path(__file__).resolve().parent;ROOT=EXP.parents[1]
def main():
 s=json.loads((EXP/'results/summary-v1.json').read_text());out=EXP/'figures';out.mkdir(exist_ok=True)
 plt.rcParams.update({'font.size':10,'axes.spines.top':False,'axes.spines.right':False,'svg.hashsalt':'pitgun-pit-strategy-audit-v1'})
 contexts=s['contexts'];x=np.arange(len(contexts));fig,ax=plt.subplots(figsize=(13,5.5),layout='constrained')
 for i,(key,label,color) in enumerate([('control','Modèle actuel','#b66e50'),('no_wear_grip','Sans perte directe usure → grip','#416c78'),('pit_penalty','Stands doublés à 44 s','#9ba2a0')]):
  vals=[c['interventions'][key]['best_tested_stops'] for c in contexts];ax.bar(x+(i-1)*.25,vals,.25,label=label,color=color)
 labels=[c['circuit']+'\n'+('I' if 'era1' in c['fixture'] else 'II') for c in contexts]
 ax.set_xticks(x,labels,rotation=30,ha='right');ax.set_yticks(range(1,10));ax.set_ylim(0,9.5);ax.set_ylabel('Arrêts du meilleur plan testé');ax.axhline(2,color='#456e62',linestyle='--',linewidth=1);ax.text(9.45,2.1,'Limite demandée : 2',ha='right',color='#456e62');ax.legend(ncol=3,loc='upper left',frameon=False,fontsize=9);ax.set_title('Usure et fréquence des arrêts · diagnostic apparié\nMême voiture, réglage, seed et plans ; stratégies hors limite uniquement pour le diagnostic')
 fig.savefig(out/'pit-incentives-v1.png',dpi=170);fig.savefig(out/'pit-incentives-v1.svg',metadata={'Date':None});plt.close(fig)
 profile=json.loads((ROOT/'experiments/career_physics_v1/profile-v1.json').read_text());fig,axes=plt.subplots(1,2,figsize=(12,5),layout='constrained');w=np.linspace(0,1,401);dt=np.linspace(-40,40,401)
 for tire,label,color in [('soft','Tendre','#b66e50'),('medium','Medium','#416c78'),('hard','Dur','#8b918e')]:
  raw=json.loads((ROOT/f'catalogs/racing/v1.8.1/simulation/tires/{tire}.json').read_text());c=profile['compounds'][tire]
  axes[0].plot(w*100,np.maximum(raw['wear_min'],1-raw['wear_grip_k']*w),label=label,color=color)
  axes[1].plot(dt,np.maximum(c['minimum_temperature_grip_factor'],np.exp(-(dt/c['temperature_width_c'])**2)),label=label,color=color)
 axes[0].set_xlabel('Usure brute du modèle (%)');axes[0].set_ylabel('Facteur direct d’adhérence');axes[0].set_title('Loi d’usure héritée');axes[0].set_xlim(0,100);axes[0].set_ylim(.4,1.02);axes[0].legend(frameon=False)
 axes[1].fill_between([-20,20],.97,1,color='#d5e2d8',label='Repère FIA 2025 : ≥97 % à ±20°C');axes[1].set_xlabel('Écart à la température optimale (°C)');axes[1].set_title('Loi thermique actuelle');axes[1].set_xlim(-40,40);axes[1].set_ylim(.6,1.02);axes[1].legend(frameon=False,loc='lower center',fontsize=9)
 fig.suptitle('Les paramètres de grip doivent être cohérents avec les nouveaux états physiques\nLe repère FIA est un objectif de conception daté, pas une mesure universelle pour les ères de Pitgun')
 fig.savefig(out/'grip-curves-v1.png',dpi=170);fig.savefig(out/'grip-curves-v1.svg',metadata={'Date':None});plt.close(fig)
 print(out)

if __name__=='__main__':main()
