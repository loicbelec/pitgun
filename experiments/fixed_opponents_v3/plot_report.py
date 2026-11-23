#!/usr/bin/env python3
"""Scientific figures derived from checked field evidence, never new calibration."""
import collections
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

from report import EXP, OUT, checked_campaign
from common import load


def save(fig, out, name):
    fig.savefig(out / (name + ".png"), dpi=170)
    fig.savefig(out / (name + ".svg"), metadata={"Date": None})
    plt.close(fig)


def main():
    checked_campaign()
    summary = load(OUT / "summary-v3.json")
    out = EXP / "figures"
    out.mkdir(exist_ok=True)
    plt.rcParams.update({"font.size": 10, "axes.spines.top": False, "axes.spines.right": False, "svg.hashsalt": "pitgun-fixed-field-v3"})
    if "hierarchy" in summary:
        actors = summary["hierarchy"]
        names = [summary["fixed_roster"][a]["name"] for a in actors]
        scores = [summary["field_scores"][a]["points"] for a in actors]
        fig, ax = plt.subplots(figsize=(10, 5.8), layout="constrained")
        ax.barh(np.arange(len(actors)), scores, color=["#b66e50" if a in ["franz_hermann", "charles_leclair"] else "#416c78" for a in actors])
        ax.set_yticks(range(len(actors)), names)
        ax.invert_yaxis()
        ax.set_xlabel("Points agrégés entre les neuf IA · conditions réservées")
        ax.set_title("Neuf personnalités constantes · hiérarchie observée\nMême budget par ère, plans scellés avant les conditions réservées")
        save(fig, out, "field-hierarchy-v3")
        fig, ax = plt.subplots(figsize=(11, 5), layout="constrained")
        stages = ["era1-entry", "era1-exit", "era2-entry", "era2-exit"]
        for style, label, color, marker, line in [
            ("routine", "Répartition uniforme", "#9ba2a0", "^", "--"),
            ("fixed_mc", "Moteur/châssis 50:50, plan par défaut", "#b66e50", "x", ":"),
            ("sampled_mc", "Moteur/châssis échantillonné", "#65877a", "s", "--"),
            ("informed", "Préparation informée", "#416c78", "o", "-"),
        ]:
            values = [summary["player_progression"][s][style]["mean_position"] for s in stages]
            ax.plot(range(4), values, label=label, color=color, marker=marker, linestyle=line, linewidth=2 if style == "informed" else 1.5)
        ax.set_xticks(range(4), ["Entrée I", "Sortie I", "Entrée II", "Sortie II"])
        ax.set_yticks(range(1, 11))
        ax.set_ylim(10.4, .6)
        ax.set_ylabel("Rang moyen face aux mêmes neuf adversaires")
        ax.set_title("Progression observée · les préparations du joueur comptent\nBudgets, équipements, bonus et commandes réellement disponibles à chaque étape")
        ax.legend(frameon=False, fontsize=9, loc="lower right")
        save(fig, out, "player-progression-v3")
    else:
        candidates = load(OUT / "calibration-candidates-v3.json")
        levels = sorted({r["elite_level"] for r in candidates})
        fig, axes = plt.subplots(1, len(levels), figsize=(12, 4.3), layout="constrained")
        for ax, level in zip(np.atleast_1d(axes), levels):
            cells = [r for r in candidates if r["elite_level"] == level]
            n1 = sorted({r["budgets"]["1"] for r in cells})
            n2 = sorted({r["budgets"]["2"] for r in cells})
            matrix = np.zeros((len(n1), len(n2)))
            for r in cells:
                i, j = n1.index(r["budgets"]["1"]), n2.index(r["budgets"]["2"])
                matrix[i, j] = r["eligible"]
                ax.text(j, i, f'{r["difficulty"]["1"]["wins"]}/{r["difficulty"]["2"]["wins"]}\n{r["pooled_exit_rank_gain_over_sampled_mc"]:+.2f}', ha="center", va="center", fontsize=9)
            ax.imshow(matrix, vmin=0, vmax=1, cmap="Greys", alpha=.25)
            ax.set_xticks(range(len(n2)), n2)
            ax.set_yticks(range(len(n1)), n1)
            ax.set_xlabel("Budget II")
            ax.set_ylabel("Budget I")
            ax.set_title(f"Niveau favoris {level}")
        fig.suptitle("Entraînement conservé · cellules : victoires informées I/II puis gain de rang vs MC\nUne case claire n’est pas acceptée ; aucun résultat réservé ouvert")
        save(fig, out, "training-calibrations-v3")
    qualification = OUT / "training-qualification-v3.json"
    if qualification.exists():
        attempts = load(qualification)
        fig, ax = plt.subplots(figsize=(12, 5), layout="constrained")
        by_attempt = []
        for attempt in attempts:
            margins = collections.defaultdict(list)
            for p in attempt["pit_checks"]:
                margins[p["actor"]].append(p["extra_stop_margin_s"])
            by_attempt.append(margins)
        identities = list(by_attempt[0])
        names = load(EXP.parent / "fixed_opponents_v2/roster-v2.json")["drivers"]
        for i, (attempt, margins) in enumerate(zip(attempts, by_attempt)):
            values = [min(margins[a]) for a in identities]
            width = .75 / len(attempts)
            ax.bar(np.arange(len(identities)) + (i-(len(attempts)-1)/2)*width, values, width=width, color=["#416c78" if v >= 0 else "#b66e50" for v in values], alpha=1-i*.18, label=f'Budgets {attempt["budgets"]["1"]}/{attempt["budgets"]["2"]}, niveau {attempt["elite_level"]}')
        ax.axhline(0, color="#626a6b", linewidth=1)
        ax.set_xticks(range(len(identities)), [names[a]["name"] for a in identities], rotation=30, ha="right")
        ax.set_ylabel("Plus faible marge 3+ − plan normal sur les dix contextes (s)")
        ax.set_title("Contre-épreuves sous les véritables traits de chaque pilote\nUne marge négative bloque la configuration ; aucun chrono corrigé")
        ax.legend(frameon=False, fontsize=9)
        save(fig, out, "pit-incentives-by-driver-v3")
    for p in out.glob("*.svg"):
        p.write_text("\n".join(line.rstrip() for line in p.read_text().splitlines()) + "\n")
    print(out)


if __name__ == "__main__":
    main()
