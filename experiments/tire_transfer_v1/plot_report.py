#!/usr/bin/env python3
"""Static scientific figures from checked retained transfer results."""
import collections
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np
from report import EXP, evidence


def main():
    _, summary, rows = evidence()
    out = EXP / "figures"
    out.mkdir(exist_ok=True)
    plt.rcParams.update({"font.size": 10, "axes.spines.top": False, "axes.spines.right": False, "svg.hashsalt": "pitgun-tire-transfer-v1"})
    groups = collections.defaultdict(list)
    for c in summary["checks"]:
        groups[(c["era"], c["circuit"])].append(c)
    contexts = list(groups)
    fig, ax = plt.subplots(figsize=(12, 8), layout="constrained")
    values = []
    for i, context in enumerate(contexts):
        for j, c in enumerate(groups[context]):
            v = c["extra_stop_margin_s"]
            values.append(v)
            label = ("Meilleur réglage échantillonné" if j == 0 else "Moteur/châssis ou alternative cooling") if i == 0 else None
            color = ("#416c78" if j == 0 else "#9ba2a0") if v >= 0 else "#b66e50"
            y = i + (j - .5) * .34
            ax.barh(y, v, height=.3, color=color, label=label)
            ax.annotate(f"{v:+.1f}", (v, y), xytext=(4 if v >= 0 else -4, 0), textcoords="offset points", va="center", ha="left" if v >= 0 else "right", fontsize=8)
    ax.axvline(0, color="#5b6364", linewidth=1)
    ax.set_yticks(range(len(contexts)), [f"{circuit} · Ère {era}" for era, circuit in contexts])
    ax.invert_yaxis()
    span = max(max(values), 0) - min(min(values), 0)
    ax.set_xlim(min(min(values), 0) - span * .12, max(max(values), 0) + span * .14)
    ax.set_xlabel("Meilleur temps 3+ arrêts − meilleur temps 1/2 arrêts (s)")
    ax.set_title("Transfert du candidat pneus · vingt comparaisons physiques\nÀ droite : un/deux arrêts meilleurs ; à gauche : contre-exemple conservé")
    ax.legend(frameon=False, fontsize=9, loc="lower right")
    fig.savefig(out / "stop-incentives-v1.png", dpi=170)
    fig.savefig(out / "stop-incentives-v1.svg", metadata={"Date": None})
    plt.close(fig)

    fig, ax = plt.subplots(figsize=(11, 5.5), layout="constrained")
    deltas = []
    for context in contexts:
        cs = groups[context]
        overall = next(c for c in cs if c["role"] == "best_sampled")
        other = next(c for c in cs if c["role"] != "best_sampled")
        if other["role"] == "engine_chassis_only":
            delta = other["best_one_two_time_s"] - overall["best_one_two_time_s"]
        else:
            delta = overall["best_one_two_time_s"] - other["best_one_two_time_s"]
        deltas.append(delta)
    ax.bar(np.arange(len(contexts)), deltas, color=["#416c78" if d >= 0 else "#b66e50" for d in deltas], width=.65)
    ax.axhline(0, color="#5b6364", linewidth=1)
    ax.set_xticks(range(len(contexts)), [f"{circuit}\n{era}" for era, circuit in contexts], rotation=35, ha="right")
    ax.set_ylabel("Temps moteur/châssis − temps de l’autre réglage (s)")
    ax.set_title("Effet des réglages, après comparaison des stratégies à un/deux arrêts\nMême budget et composants ; choix d’ancres conditionné au premier plan medium/hard")
    fig.savefig(out / "setup-tradeoffs-v1.png", dpi=170)
    fig.savefig(out / "setup-tradeoffs-v1.svg", metadata={"Date": None})
    plt.close(fig)

    c = min(summary["checks"], key=lambda c: c["extra_stop_margin_s"])
    fig, axes = plt.subplots(2, 2, figsize=(12, 7), layout="constrained")
    from report import key
    for field, color in [("best_one_two_case", "#416c78"), ("best_extra_case", "#b66e50")]:
        case = c[field]
        data = rows[key(case)]["data"]
        laps = data["lap_metrics"]
        xs = [l["lap"] for l in laps]
        label = f'{len(case["strategy"]["pit_laps"])} arrêt(s) · {data["outcome"]["time_s"]:.1f} s'
        for ax, name, scale in [(axes[0, 0], "surface_end_c", 1), (axes[0, 1], "wear_end", 100), (axes[1, 0], "wear_grip_end", 1), (axes[1, 1], "engine_end_c", 1)]:
            ax.plot(xs, [l[name] * scale for l in laps], color=color, label=label)
    for ax, ylabel in zip(axes.flat, ["Surface pneu (°C)", "Usure brute (%)", "Facteur direct d’adhérence lié à l’usure", "Température moteur (°C)"]):
        ax.set_xlabel("Tour")
        ax.set_ylabel(ylabel)
    axes[0, 0].legend(frameon=False, fontsize=9)
    fig.suptitle(f'{c["circuit"]} · Ère {c["era"]} · marge la plus faible ({c["extra_stop_margin_s"]:+.2f} s)\nComparaison appariée, états en fin de tour, même réglage et mêmes paramètres')
    fig.savefig(out / "closest-comparison-v1.png", dpi=170)
    fig.savefig(out / "closest-comparison-v1.svg", metadata={"Date": None})
    plt.close(fig)
    for p in out.glob("*.svg"):
        p.write_text("\n".join(line.rstrip() for line in p.read_text().splitlines()) + "\n")
    print(out)


if __name__ == "__main__":
    main()
