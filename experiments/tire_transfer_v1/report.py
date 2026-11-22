#!/usr/bin/env python3
"""Derived transfer report; never selects or changes model coefficients."""
import collections
import pathlib
import sys

EXP = pathlib.Path(__file__).resolve().parent
ROOT = EXP.parents[1]
sys.path.insert(0, str(ROOT / "experiments/tire_performance_v1"))
from kernel import key, load, sha, write


def evidence():
    out = EXP / "results"
    receipt = load(out / "campaign-v1.json")
    for name, digest in receipt["files"].items():
        if sha((out / name).read_bytes()) != digest:
            raise RuntimeError("retained receipt changed: " + name)
    provenance = receipt["provenance"]
    for path, digest in {**provenance["sources"], **provenance["kernel_pin"]["sources"]}.items():
        if sha((ROOT / path).read_bytes()) != digest:
            raise RuntimeError("physical/protocol source changed: " + path)
    rows = {}
    for phase in receipt["phase_stats"]:
        for r in load(out / (phase + "-v1.json.gz")):
            rows[key(r["case"])] = r
    summary = load(out / "summary-v1.json")
    return receipt, summary, rows


def main():
    receipt, summary, rows = evidence()
    checks = summary["checks"]
    counterexamples = [c for c in checks if not c["passed"]]
    thermal = collections.defaultdict(float)
    for r in rows.values():
        for name, v in r["data"]["thermal_ledger"].items():
            if name.startswith("maximum"):
                thermal[name] = max(thermal[name], v)
    write(EXP / "results/review-v1.json", {"schema": "pitgun.tire-transfer-review/v1", "source_campaign_digest": sha((EXP / "results/campaign-v1.json").read_bytes()), "report_source_digest": sha(pathlib.Path(__file__).read_bytes()), "counterexamples": counterexamples, "thermal_maxima": dict(thermal), "minimum_extra_stop_margin_s": min(c["extra_stop_margin_s"] for c in checks)})
    lines = ["# Transfert pneus · dix contextes I/II", "", f'**Résultat : `{summary["status"]}`.** Le candidat `{summary["profile"]}` est évalué sans changement de coefficients ni de coût de stand. Aucun plateau de neuf pilotes ni modèle jouable n’est admis par ce rapport.', "", "## Périmètre et calculs", "", f'- {summary["contexts"]} contextes, {summary["anchors"]} réglages comparés, pilote de référence Balanced et seed d’entraînement fixe.', f'- {summary["unique_cases"]:_} entrées distinctes, dont {summary["new_unique_case_results"]:_} nouvelles et {summary["reused_existing_case_results"]:_} réutilisations exactes du kernel déjà qualifié.'.replace("_", " "), f'- {summary["fresh_exact_repeat_executions"]} répétitions fraîches, égales champ par champ.', f'- {len(summary["failed_simulations"])} simulation en échec ; sauts involontaires entre tours : `{summary["maximum_carry_jumps_speed_engine_surface_carcass_wear"]}`.', f'- Résidu maximal du bilan temps de course/tours/stands : `{summary["maximum_pit_accounting_residual_s"]:.3g} s`.', '- Seeds et températures réservés du plateau toujours fermés ; aucune mesure de performance WASM/VPS ni validation industrielle.', "", "## Comparaison physique des stratégies", "", "Le premier réglage est le meilleur échantillonné sous un plan medium/hard à mi-course ; le second est le meilleur concentré moteur/châssis. S’ils sont identiques, une alternative avec cooling discrétionnaire positif est utilisée. Les stratégies sont ensuite réévaluées à chaque réglage. Les points ci-dessous sont discrétionnaires, dans l’ordre aéro/châssis/moteur/cooling ; les offsets HQ et composants restent séparés dans les reçus.", "", "`Marge = meilleur temps à 3+ arrêts − meilleur temps à 1/2 arrêts`. Une valeur positive favorise un ou deux arrêts. Toute valeur négative est conservée comme contre-exemple, sans cap caché du solveur ni correction de chrono.", "", "| Contexte | Réglage | Points A/C/E/K | Arrêts 1/2 | Arrêts 3+ | Marge (s) | Critère |", "|---|---|---|---:|---:|---:|---|"]
    labels = {"best_sampled": "Meilleur échantillonné", "engine_chassis_only": "Moteur/châssis", "positive_cooling_alternative": "Alternative cooling"}
    for c in checks:
        lines.append(f'| {c["circuit"]} · Ère {c["era"]} | {labels[c["role"]]} | {"/".join(map(str,c["anchor_case"]["allocation"]))} | {len(c["best_one_two_case"]["strategy"]["pit_laps"])} | {len(c["best_extra_case"]["strategy"]["pit_laps"])} | {c["extra_stop_margin_s"]:+.2f} | {"Passe" if c["passed"] else "REFINE"} |')
    lines += ["", "## Lecture et suites", ""]
    if counterexamples:
        lines.append(f'**{len(counterexamples)} réglage(s) présentent un contre-exemple ou un échec.** Le transfert ne permet donc pas encore de reprendre la calibration finale du plateau. Les cas précis sont dans [le résumé](results/summary-v1.json) et les stratégies/états par tour dans les reçus compressés. Il faut expliquer ces cas avant toute modification ciblée et versionnée ; ne pas masquer leur avantage par une limite d’interface.')
    else:
        lines.append('Les vingt comparaisons satisfont les critères enregistrés : les arrêts supplémentaires ne sont pas rentables dans cet échantillon, avec les mêmes coefficients pneus partout. Ce résultat autorise la prochaine campagne de calibration des neuf pilotes ; il ne la remplace pas et ne prouve pas un optimum global.')
    groups = collections.defaultdict(list)
    for c in checks:
        groups[(c["era"], c["circuit"])].append(c)
    changed = [f'{circuit} (ère {era})' for (era, circuit), cs in groups.items() if len({len(c["best_one_two_case"]["strategy"]["pit_laps"]) for c in cs}) > 1]
    if changed:
        lines += ["", "Le meilleur nombre d’arrêts échantillonné change entre les deux réglages à " + ", ".join(changed) + ". C’est une observation du couplage réglages/stratégie, pas un ordre d’arrivée imposé."]
    lines += ["", "Le meilleur réglage du premier balayage n’est pas nécessairement le meilleur après changement de stratégie. À Silverstone II, la configuration moteur/châssis échantillonnée devient environ 1,64 s plus rapide que l’autre après comparaison des plans. Le prochain réglage du plateau doit donc chercher conjointement préparation et stratégie, sans imposer une dépense sur chaque axe."]
    lines += ["", "## Figures", "", "- [Comparaison des arrêts, vingt réglages](figures/stop-incentives-v1.png)", "- [Écarts entre réglages après comparaison des stratégies](figures/setup-tradeoffs-v1.png)", "- [États par tour du cas à la marge la plus faible](figures/closest-comparison-v1.png)"]
    lines += ["", "Le choix des réglages reste conditionné au plan medium/hard de la première recherche. Les 114 plans et 84 contre-épreuves forment un échantillon discret ; toutes les combinaisons continues de réglages et séquences ne sont pas explorées. Les offsets présents en ère II peuvent légitimement réduire l’intérêt de points discrétionnaires supplémentaires de cooling. Aucun critère n’impose de dépenser sur les quatre axes.", "", "Les chronos comparent des voitures indépendantes : pas de trafic, aspiration, collision ni météo dynamique. Les courbes d’évolution présentent des valeurs en fin de tour et ne décrivent pas les variations instantanées dans chaque virage. L’usure brute est un budget interne, pas une probabilité de crevaison.", "", "Protocole, conservation et reproduction : [README](README.md), [plan](plan-v1.json). Preuves : [campagne](results/campaign-v1.json), [résumé](results/summary-v1.json), [réglages figés](results/anchor-lock-v1.json)."]
    (EXP / "RESULTS_V1.md").write_text("\n".join(lines) + "\n")
    print({"status": summary["status"], "counterexamples": len(counterexamples), "minimum_extra_stop_margin_s": min(c["extra_stop_margin_s"] for c in checks)})


if __name__ == "__main__":
    main()
