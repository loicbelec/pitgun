#!/usr/bin/env python3
"""Figures from recorded fixed-field validation; no simulation or selection."""
import collections
import gzip
import json
import pathlib
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
import numpy as np

EXP=pathlib.Path(__file__).resolve().parent

def main():
 s=json.loads((EXP/'results/summary-v2.json').read_text())
 if s['status']!='COMPLETE':raise RuntimeError('wait for completed campaign')
 figdir=EXP/'figures';figdir.mkdir(exist_ok=True)
 plt.rcParams.update({'font.size':10,'axes.spines.top':False,'axes.spines.right':False,'svg.hashsalt':'pitgun-fixed-opponents-v2'})
 order=s['hierarchy'];scores=s['field_only_scores'];drivers=s['fixed_roster'];means=[scores[a]['rank_sum']/scores[a]['races'] for a in order]
 fig,ax=plt.subplots(figsize=(10,5.5),layout='constrained')
 ax.barh(np.arange(9),means,color=['#b66e50' if a in ['franz_hermann','charles_leclair'] else '#416c78' for a in order]);ax.set_yticks(np.arange(9),[drivers[a]['name'] for a in order]);ax.invert_yaxis();ax.set_xlim(0,9);ax.set_xticks(range(1,10));ax.set_xlabel('Position moyenne parmi les neuf IA (plus petit = mieux)')
 for i,a in enumerate(order):
  inside=means[i]>=6
  ax.text(means[i]-.08 if inside else means[i]+.08,i,f"{means[i]:.2f} · {scores[a]['wins']} premières places",va='center',ha='right' if inside else 'left',color='#fff4de' if inside else '#152c33')
 ax.set_title('Personnalités fixes · conditions réservées\nOrdre des barres par points cumulés du benchmark ; aucune arrivée imposée')
 fig.savefig(figdir/'fixed-field-hierarchy-v2.png',dpi=170);fig.savefig(figdir/'fixed-field-hierarchy-v2.svg',metadata={'Date':None});plt.close(fig)
 stages=['era1-entry','era1-exit','era2-entry','era2-exit'];styles=['routine','fixed_mc','sampled_mc','informed'];labels=['Réglage initial','Recette fixe M/C','M/C échantillonné','Préparation informée'];colors=['#9ba2a0','#ba8b74','#718b99','#386c73']
 fig,ax=plt.subplots(figsize=(12,5.5),layout='constrained');x=np.arange(4)
 for i,(style,label,color) in enumerate(zip(styles,labels,colors)):
  values=[s['player_progression'][stage][style]['mean_position'] for stage in stages]
  ax.bar(x+(i-1.5)*.19,values,width=.18,label=label,color=color)
 ax.set_xticks(x,['Entrée I','Sortie I','Entrée II\nancien moteur','Sortie II']);ax.set_ylim(0,10.7);ax.set_yticks(range(1,11));ax.set_ylabel('Position moyenne du joueur (1 = premier)');ax.set_title('Même plateau fixe, préparations différentes\nCarrières et conditions réservées ; contrôleurs expérimentaux, pas des joueurs observés');ax.legend(ncol=2,loc='upper center',bbox_to_anchor=(.5,-.12),frameon=False)
 fig.savefig(figdir/'player-progression-v2.png',dpi=170);fig.savefig(figdir/'player-progression-v2.svg',metadata={'Date':None});plt.close(fig)
 cases=json.loads(gzip.decompress((EXP/'results/strategy-search-v2.json.gz').read_bytes()));groups=collections.defaultdict(list)
 for r in cases:groups[(1 if 'era1' in r['case']['fixture_id'] else 2,r['case']['circuit'])].append(r)
 fig,axes=plt.subplots(1,2,figsize=(13,5.5),layout='constrained')
 for ax,era in zip(axes,[1,2]):
  for (e,circuit),rows in sorted(groups.items()):
   if e!=era:continue
   good=[r for r in rows if r['data']['outcome']['ok']];best=min(r['data']['outcome']['time_s'] for r in good);by=collections.defaultdict(list)
   for r in good:by[len(r['case']['strategy']['pit_laps'])].append(r['data']['outcome']['time_s'])
   counts=sorted(n for n in by if 3<=n<=12)
   ax.plot(counts,[min(by[n])-best for n in counts],marker='o',label=circuit)
  ax.set_title(f'Ère {era}');ax.set_xlabel('Nombre d’arrêts testé');ax.set_xticks([3,4,5,6,7,8,12]);ax.set_ylim(bottom=-5);ax.legend(fontsize=8,frameon=False)
 axes[0].set_ylabel('Écart au meilleur plan échantillonné (s/GP)')
 fig.suptitle('La borne de trois arrêts masquait des plans plus rapides\nComparaison sur entraînement, même réglage de référence ; physique et coût de stand inchangés')
 fig.savefig(figdir/'strategy-stop-count-v2.png',dpi=170);fig.savefig(figdir/'strategy-stop-count-v2.svg',metadata={'Date':None});plt.close(fig)
 print(figdir)

if __name__=='__main__':main()
