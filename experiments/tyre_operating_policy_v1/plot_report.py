#!/usr/bin/env python3
"""Render native observations; no new physics or event inference in the charts."""
import html
import json
from pathlib import Path
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

HERE = Path(__file__).resolve().parent
report = json.loads((HERE/"results/screen-v1.json").read_text())
rows = {r["case"]["id"]:r for r in report["results"]}
out = HERE/"figures"
out.mkdir(exist_ok=True)
plt.rcParams.update({"font.size":9,"axes.spines.top":False,"axes.spines.right":False,"figure.facecolor":"#f4f0e6","axes.facecolor":"#faf8f2","savefig.facecolor":"#f4f0e6"})
petrol, rust, green = "#24515c", "#ae573f", "#52865e"

def values(row,col): return [p[col] for p in row["chart"]["rows"]]
def summary(row):
    laps = row["laps"]
    driving = sum(sum(l["band_occupancy_s"].values()) for l in laps)
    totals = {k:sum(l["band_occupancy_s"][k] for l in laps) for k in ["cold","near_peak","hot"]}
    return {"driving_s":driving,"bands":{k:100*v/driving if driving else 0 for k,v in totals.items()},"heating":100*sum(l["heating_s"] for l in laps)/driving if driving else 0,"cooling":100*sum(l["cooling_s"] for l in laps)/driving if driving else 0,"floor":100*sum(l["grip_floor_s"] for l in laps)/driving if driving else 0}

def decorate(ax):
    ax.grid(alpha=.13)
    ax.set_xlabel("Simulation time (min)")

for id,row in rows.items():
    fig,axs = plt.subplots(2,2,figsize=(12,7.2),constrained_layout=True)
    time = [v/60 for v in values(row,0)]
    ax = axs[0,0]
    ax.plot(time,values(row,4),color=petrol,label="Surface (grip state)",lw=1.3)
    ax.plot(time,values(row,5),color=rust,label="Carcass",ls="--",lw=1.2)
    previous = 0.0
    for i,l in enumerate(row["laps"]):
        drive_end = previous+l["time_s"]
        ax.fill_between([previous/60,drive_end/60],l["window_c"][0],l["window_c"][1],color=green,alpha=.13,label="Near-peak thermal grip (>=95%)" if i==0 else None)
        if l["pit"]:
            ax.axvline(l["elapsed_s"]/60,color="#777",ls=":",lw=.8)
            ax.annotate("New prepared set",(l["elapsed_s"]/60,l["after_service"]["tire_temp"]),xytext=(4,8),textcoords="offset points",fontsize=7)
        previous = l["elapsed_s"]
    ax.set(title="Temperature and compound-specific operating window",ylabel="Temperature (°C)")
    ax.legend(fontsize=7,loc="best")
    ax=axs[0,1]
    ax.plot(time,[100*v for v in values(row,7)],color=petrol,label="Thermal factor only")
    ax.plot(time,[100*v for v in values(row,8)],color=rust,ls="--",label="Compound × thermal × wear")
    ax.axhline(95,color=green,ls=":",lw=1,label="Near-peak thermal threshold")
    ax.set(title="Reversible temperature penalty vs. effective grip",ylabel="Factor (%)")
    ax.legend(fontsize=7,loc="best")
    ax=axs[1,0]
    for stint in sorted(set(values(row,3))):
        p=[p for p in row["chart"]["rows"] if p[3]==stint]
        ax.plot([v[0]/60 for v in p],[100*v[6] for v in p],color=rust,lw=1.3,label="Raw wear (new set resets)" if stint==0 else None)
    ax.set(title="Irreversible wear of each tire set",ylabel="Raw wear (%)",ylim=(0,105))
    ax.legend(fontsize=7)
    ax=axs[1,1]
    early=[p for p in row["chart"]["rows"] if p[0]<=180]
    ax.plot([p[0]/60 for p in early],[p[4] for p in early],color=petrol,label="Surface, first 3 min")
    if early:
        low,high=row["laps"][0]["window_c"]
        ax.axhspan(low,high,color=green,alpha=.13)
    s=summary(row)
    ax.set(title=f"Native heating {s['heating']:.1f}% / cooling {s['cooling']:.1f}% of driving",ylabel="Surface temperature (°C)")
    for ax in axs.flat: decorate(ax)
    conditions=row["resolved_initial_conditions"]
    outcome="Completed" if row["outcome"]["ok"] else "FAILED — accepted prefix only"
    fig.suptitle(f"{id} · {outcome}\nStart {conditions['surface_c']:g}°C / pit set {conditions['pit_surface_c']:g}°C · Native exposed screen; coefficients provisional",fontsize=11)
    fig.savefig(out/f"{id}.png",dpi=130)
    plt.close(fig)

fig,axs=plt.subplots(2,2,figsize=(12,7.6),constrained_layout=True)
for t,id in [(35,"medium-initial-35"),(70,"medium-70"),(90,"medium-initial-90"),(110,"medium-initial-110")]:
    row=rows[id]
    axs[0,0].plot([v/60 for v in values(row,0)],values(row,4),label=f"Initial {t}°C",lw=1)
axs[0,0].axhspan(*rows['medium-70']['laps'][0]['window_c'],color=green,alpha=.13,label="Near-peak band")
axs[0,0].set(title="Same medium setup, four initial states",ylabel="Surface (°C)")
for name,color in [("soft",rust),("medium",petrol),("hard","#6f735d")]:
    row=rows[name+"-70"]
    axs[0,1].plot([v/60 for v in values(row,0)],values(row,4),label=name,color=color,lw=1)
axs[0,1].set(title="Prepared sets: compound response",ylabel="Surface (°C)")
for id,label in [("monza-70","Monza"),("silverstone-70","Silverstone"),("medium-70","Barcelona")]:
    row=rows[id]; points=[p for p in row['chart']['rows'] if p[0]<=300]
    axs[1,0].plot([p[0]/60 for p in points],[p[4] for p in points],label=label,lw=1)
axs[1,0].set(title="First 5 min: different circuits / fixed exposed setups",ylabel="Surface (°C)")
row=rows['cold-start-prepared-replacement']
axs[1,1].plot([v/60 for v in values(row,0)],values(row,4),label="Surface",color=petrol,lw=1)
axs[1,1].plot([v/60 for v in values(row,0)],values(row,5),label="Carcass",color=rust,ls="--",lw=1)
for l in row['laps']:
    if l['pit']: axs[1,1].axvline(l['elapsed_s']/60,color="#666",ls=":",label="Replacement at 70°C")
axs[1,1].set(title="35°C diagnostic start; independent prepared pit set",ylabel="Temperature (°C)")
for ax in axs.flat: decorate(ax); ax.legend(fontsize=7)
fig.suptitle("Pitgun tire operating policy — native qualification\nPreparation and near-peak reporting; no identified tire or competitive acceptance",fontsize=12)
fig.savefig(out/"overview.png",dpi=160)
plt.close(fig)

options=''.join(f'<option value="{html.escape(id)}">{html.escape(id)}</option>' for id in rows)
table=[]
for id,row in rows.items():
    s=summary(row); state=row['outcome'].get('final_state',{})
    table.append(f'<tr><td>{html.escape(id)}</td><td>{"OK" if row["outcome"]["ok"] else "Failed"}</td><td>{s["bands"]["cold"]:.1f}%</td><td>{s["bands"]["near_peak"]:.1f}%</td><td>{s["bands"]["hot"]:.1f}%</td><td>{s["floor"]:.1f}%</td><td>{100*state.get("tire_wear",0):.1f}%</td></tr>')
page='''<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>Pitgun · Tire thermal qualification</title>
<style>body{margin:0;background:#f4f0e6;color:#18343d;font:15px system-ui,sans-serif}main{max-width:1150px;margin:auto;padding:24px}h1{font-size:28px}h2{font-size:20px}p{line-height:1.5;max-width:900px}strong{color:#a34e35}img{display:block;width:100%;height:auto;border-radius:8px}select{max-width:100%;padding:9px;background:#fff;border:1px solid #24515c;border-radius:5px;color:#18343d;font:inherit;margin:8px 0}table{border-collapse:collapse;width:100%;font-size:12px}td,th{text-align:right;padding:7px;border-bottom:1px solid #cbd1ca;white-space:nowrap}td:first-child,th:first-child{text-align:left}thead{background:#dce5df}.scroll{overflow:auto}a{color:#24515c}.note{font-size:13px}.bar{border-top:2px solid #24515c;padding-top:12px}</style><main>
<h1>Pitgun · Tire thermal qualification</h1><p><strong>Native experimental model · coefficients provisional · no game activation.</strong> A prepared modern slick starts at 70°C. Cold/hot initial states are explicit diagnostics. The green band means at least 95% of the peak <em>temperature-only</em> grip factor; it does not mean that a worn tire retains 95% of its effective grip.</p>
<img src="figures/overview.png" alt="Comparison of initial temperature, compounds, circuits and prepared pit replacement">
<h2>Inspect a scenario</h2><label for="scenario">Recorded case </label><select id="scenario">OPTIONS</select><img id="detail" src="figures/soft-70.png" alt="Selected native tire temperature, grip and wear traces">
<p class="note">Curves select native points every 1s during the first 300s, then every 10s plus lap endpoints. Small local cooling/heating can be visually smoothed. Metrics and events use all accepted native points; no physics sampling was reduced.</p>
<h2>Driving time in each thermal band</h2><p class="note">Time is weighted by native segment duration using its starting temperature. Stops are excluded. “Floor” is time at the temperature-grip floor, which can hide sensitivity far from the optimum. Wear is the raw final-set state, not a universal pit threshold.</p>
<div class="scroll"><table><thead><tr><th>Case</th><th>Outcome</th><th>Cold</th><th>Near peak</th><th>Hot</th><th>Floor</th><th>Final wear</th></tr></thead><tbody>TABLE</tbody></table></div>
<p class="bar"><a href="README.md">Policy, sources and limitations</a> · <a href="RESULTS_V1.md">Qualification results</a> · <a href="results/screen-v1.json">Frozen native receipt</a></p><p class="note">Source SOURCE. This exposed screen does not identify physical coefficients, prove balanced strategies or measure browser/WASM/VPS performance.</p></main><script>document.getElementById('scenario').addEventListener('change',function(){document.getElementById('detail').src='figures/'+this.value+'.png';});</script></html>'''
(HERE/'index.html').write_text(page.replace('OPTIONS',options).replace('TABLE',''.join(table)).replace('SOURCE',report['identity']['source_commit']))
print(out/'overview.png')
