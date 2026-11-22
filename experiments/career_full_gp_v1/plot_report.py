#!/usr/bin/env python3
"""Static scientific figures from verified, recorded native results only."""
import json
import pathlib
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np
from matplotlib.colors import TwoSlopeNorm

EXP=pathlib.Path(__file__).resolve().parent


def main():
    summary=json.loads((EXP/"results/summary-v1.json").read_text())
    out=EXP/"figures";out.mkdir(exist_ok=True)
    plt.rcParams.update({"font.size":10,"axes.spines.top":False,"axes.spines.right":False,"figure.facecolor":"white","svg.hashsalt":"pitgun-full-gp-v1"})
    axes=["aero","chassis","engine","cooling"]
    labels=["Aéro","Châssis","Moteur","Cooling"]
    cohorts=sorted({(d["fixture"],d["circuit"]) for d in summary["axis_diagnostics"]})
    lookup={(d["fixture"],d["circuit"],d["axis"]):d for d in summary["axis_diagnostics"]}
    fig,plots=plt.subplots(1,2,figsize=(12,5),layout="constrained")
    for plot,metric,title in zip(plots,["time_s","engine_peak_c"],["Variation du temps total (s)","Variation du pic moteur (°C)"]):
        values=np.array([[lookup[(f,c,a)]["deltas"].get(metric,float("nan")) for a in axes] for f,c in cohorts])
        limit=max(float(np.nanmax(np.abs(values))),1e-9)
        image=plot.imshow(values,cmap="coolwarm",norm=TwoSlopeNorm(vcenter=0,vmin=-limit,vmax=limit),aspect="auto")
        for i in range(len(cohorts)):
            for j in range(len(axes)):
                v=values[i,j];plot.text(j,i,f"{v:+.2f}" if np.isfinite(v) else "échec",ha="center",va="center",color="white" if np.isfinite(v) and abs(v)>limit*.65 else "#172d33")
        plot.set_xticks(range(4),labels)
        plot.set_yticks(range(len(cohorts)),[f"{f.split(':')[1]} · {c.title()}" for f,c in cohorts])
        plot.set_title(title);fig.colorbar(image,ax=plot,shrink=.7)
    fig.suptitle("+1 point sur un seul axe — diagnostic hors budget carrière\nGP complet, autres réglages fixes ; différences brutes, tolérances inchangées",fontsize=13)
    fig.savefig(out/"axis-diagnostics-v1.png",dpi=170);fig.savefig(out/"axis-diagnostics-v1.svg",metadata={"Date":None});plt.close(fig)
    pairs=summary["selected_pairs"]
    fig,plots=plt.subplots(1,2,figsize=(13,6),layout="constrained",gridspec_kw={"width_ratios":[1,1.4]})
    names=[f"{p['fixture'].split(':')[1]}\n{p['circuit'].title()}" for p in pairs]
    y=np.arange(len(pairs));deltas=[p["concentrated_minus_diversified_s"] for p in pairs]
    plots[0].barh(y,deltas,color="#386a75")
    plots[0].set_yticks(y,names);plots[0].invert_yaxis();plots[0].axvline(0,color="#555",lw=.8)
    plots[0].set_xlabel("Concentré − diversifié (s)\npositif : diversifié plus rapide")
    plots[0].set_title("Écart GP après sélection des stratégies")
    colors=["#638e8a","#8e9662","#bf704f","#5280a0"]
    for i,(axis,color,label) in enumerate(zip(axes,colors,labels)):
        ys=[];width=[];left=[]
        for j,pair in enumerate(pairs):
            for k,family in enumerate(["concentrated","diversified"]):
                setup=pair[family];ys.append(j*3+k);width.append(setup["effective_allocation"][axis]);left.append(sum(setup["effective_allocation"][a] for a in axes[:i]))
        plots[1].barh(ys,width,left=left,color=color,label=label)
    plots[1].set_yticks([j*3+k for j in range(len(pairs)) for k in [0,1]], [f"{names[j].replace(chr(10),' · ')} · {family}" for j in range(len(pairs)) for family in ["MC","Alt."]])
    plots[1].invert_yaxis();plots[1].set_xlabel("Points effectifs = dépense + bonus HQ")
    plots[1].set_title("Allocations retenues ; HQ inclus");plots[1].legend(ncol=4,loc="upper center",bbox_to_anchor=(.5,-.12),frameon=False)
    fig.suptitle("Meilleurs échantillons de la recherche déclarée — aucun champ adverse exécuté",fontsize=13)
    fig.savefig(out/"selected-setups-v1.png",dpi=170);fig.savefig(out/"selected-setups-v1.svg",metadata={"Date":None});plt.close(fig)
    print(out)


if __name__=="__main__":main()
