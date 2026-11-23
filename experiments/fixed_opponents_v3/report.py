#!/usr/bin/env python3
"""Check retained evidence and derive the native field verdict without retuning."""
import collections
import json
import pathlib

import common as c
from common import EXP, ROOT, key, load, sha, write

OUT = EXP / "results"


def checked_campaign():
    receipt = load(OUT / "campaign-v3.json")
    for name, digest in receipt["files"].items():
        if sha((OUT / name).read_bytes()) != digest:
            raise RuntimeError("retained evidence changed: " + name)
    prov = receipt["provenance"]
    for path, digest in {**prov["sources"], **prov["kernel_pin"]["sources"]}.items():
        if sha((ROOT / path).read_bytes()) != digest:
            raise RuntimeError("protocol or physical source changed: " + path)
    transfer = load(ROOT / "experiments/tire_transfer_v1/results/campaign-v1.json")
    for path, digest in transfer["provenance"]["sources"].items():
        if sha((ROOT / path).read_bytes()) != digest:
            raise RuntimeError("reference transfer source changed: " + path)
    return receipt


def summarize(receipt):
    identities = set()
    numeric = set()
    physical = set()
    failed = []
    carry = [0.] * 5
    thermal = collections.defaultdict(float)
    residual = 0.
    resource_hashes = collections.defaultdict(set)
    for phase in receipt["phase_stats"]:
        for row in load(OUT / (phase + "-v3.json.gz")):
            case, data = row["case"], row["data"]
            identity = key(case)
            if case["id"] != identity or data["id"] != identity:
                raise RuntimeError("case identity changed")
            if identity in identities:
                continue
            identities.add(identity)
            numeric.add(data["diagnostic_input_digest"])
            physical.add(data["physical_input_digest"])
            if not data["outcome"]["ok"]:
                failed.append({"case": case, "outcome": data["outcome"]})
            else:
                residual = max(residual, abs(data["outcome"]["time_s"] - sum(data["lap_times_s"]) - len(case["strategy"]["pit_laps"]) * data["receipt"]["resolved_pit_loss_s"]))
            for i, value in enumerate(data["maximum_carry_jumps"]):
                carry[i] = max(carry[i], value)
            for name, value in data["thermal_ledger"].items():
                if name.startswith("maximum"):
                    thermal[name] = max(thermal[name], value)
            if case["actor"] in c.ROSTER["drivers"]:
                driver = data["receipt"]["driver"]
                if driver["controls"]["driver_id"] != case["actor"] or driver["controls"]["requested_commitment"] != .8:
                    raise RuntimeError("named identity/mode mismatch")
                resource_hashes[(case["actor"], case["elite_level"])].add(driver["driver_resource_digest"])
    if any(len(h) != 1 for h in resource_hashes.values()):
        raise RuntimeError("driver traits changed within a fixed calibration level")
    executions = collections.Counter()
    with (OUT / "native-executions-v3.jsonl").open() as stream:
        for line in stream:
            entry = json.loads(line)
            executions["completed_native_gp"] += 1
            executions["native_field_gp" if entry["native_field"] else "forced_repeat_gp" if entry["forced"] else "ordinary_fresh_gp"] += 1
    initial = set(receipt["provenance"]["initial_cached_case_ids"])
    return {"schema": "pitgun.fixed-field-summary/v3", "campaign_status": receipt["status"], "performance_profile": receipt["provenance"]["performance_profile"], "source_commit": receipt["provenance"]["source_commit"], "unique_semantic_cases": len(identities), "unique_diagnostic_inputs": len(numeric), "unique_physical_inputs": len(physical), "new_unique_case_results": sum(k not in initial for k in identities), "reused_initial_cache_cases": sum(k in initial for k in identities), "execution_counts": dict(executions), "failed_simulations": failed, "maximum_carry_jumps_speed_engine_surface_carcass_wear": carry, "maximum_pit_accounting_residual_s": residual, "thermal_maxima": dict(thermal), "phase_stats": receipt["phase_stats"], "scope": "Native I/II Craft Engineering snapshots, static plans, independent cars. Not a runtime/catalog/game admission or all-era/doctrine qualification."}


def heldout(summary, receipt):
    seal = load(OUT / "selection-lock-v3.json")
    policy = seal["policy"]
    if sha(c.enc(policy)) != seal["policy_digest"] or receipt["policy_digest"] != seal["policy_digest"]:
        raise RuntimeError("sealed policy changed")
    rows = load(OUT / "reserved-field-and-player-v3.json.gz")
    by_id = {key(r["case"]): r for r in rows}
    for row in load(OUT / "reserved-exact-repeat-v3.json.gz"):
        if by_id[key(row["case"])] != row:
            raise RuntimeError("held-out exact repeat changed")
    fields, players = {}, {}
    hashes = collections.defaultdict(set)
    actors = policy["roster"]["drivers"]
    for row in rows:
        case, data = row["case"], row["data"]
        era = c.FIXTURES[case["fixture_id"]]["era"]
        group = (era, case["circuit"], case["seed"], c.enc(case["environment"]).decode())
        if case["actor"] in actors:
            if case["budget"] != policy["budgets_by_era"][str(era)] or case["performance_profile"] != policy["performance_profile"] or case["diagnostic_mode"] or len(case["strategy"]["pit_laps"]) > 2:
                raise RuntimeError("field budget/profile/normal-plan policy changed")
            driver = data["receipt"]["driver"]
            if driver["controls"]["driver_traits"] != actors[case["actor"]]["resource"]["traits"]:
                raise RuntimeError("expanded final traits differ from actual runtime resource")
            if driver["lapse_profile"]["noise_stream_id"] != "fixed-field:" + case["actor"]:
                raise RuntimeError("named driver RNG namespace changed")
            hashes[case["actor"]].add(driver["driver_resource_digest"])
            fields.setdefault(group, {})[case["actor"]] = row
        else:
            players[key(case)] = row
    if any(len(hashes[a]) != 1 for a in actors):
        raise RuntimeError("final driver traits changed across reserved contexts")
    scores = {a: {"points": 0, "races": 0, "rank_sum": 0, "wins": 0, "podiums": 0, "finishes": 0} for a in actors}
    era_points = {}
    for group, field in fields.items():
        if set(field) != set(actors):
            raise RuntimeError("field does not contain exactly nine fixed identities")
        for i, row in enumerate(c.rank(list(field.values()))):
            actor = row["case"]["actor"]
            s = scores[actor]
            s["races"] += 1
            s["rank_sum"] += i + 1
            s["wins"] += i == 0
            s["podiums"] += i < 3
            s["finishes"] += c.ok(row)
            points = [25, 18, 15, 12, 10, 8, 6, 4, 2][i] if c.ok(row) else 0
            s["points"] += points
            es = era_points.setdefault(str(group[0]), {})
            es[actor] = es.get(actor, 0) + points
    hierarchy = sorted(scores, key=lambda a: (-scores[a]["points"], a))
    progress, races = {}, []
    for prepared in policy["player_cases"]:
        base = prepared["case"]
        stage = prepared["fixture"].split(":")[1]
        for group, field in fields.items():
            if list(group[:2]) != prepared["context"]:
                continue
            case = c.change(base, seed=group[2], environment=json.loads(group[3]))
            row = players[key(case)]
            ranked = c.rank(list(field.values()) + [row])
            position = next(i + 1 for i, r in enumerate(ranked) if r["case"]["actor"] == "player")
            races.append({"fixture": prepared["fixture"], "style": prepared["style"], "circuit": group[1], "seed": group[2], "environment": json.loads(group[3]), "position": position, "finished": c.ok(row), "time_s": row["data"]["outcome"].get("time_s")})
            s = progress.setdefault(stage, {}).setdefault(prepared["style"], {"positions": [], "times_s": [], "wins": 0, "finishes": 0})
            s["positions"].append(position)
            if c.ok(row):
                s["times_s"].append(row["data"]["outcome"]["time_s"])
            s["wins"] += position == 1
            s["finishes"] += c.ok(row)
    for styles in progress.values():
        for s in styles.values():
            s["races"] = len(s["positions"])
            s["mean_position"] = sum(s["positions"]) / s["races"]
            s["median_position"] = sorted(s["positions"])[s["races"] // 2]
            s["mean_time_s"] = sum(s["times_s"]) / len(s["times_s"]) if s["times_s"] else None
    pooled = []
    progression_ok = True
    for era in [1, 2]:
        stage = progress[f"era{era}-exit"]
        inf, mc, routine, fixed = [stage[name] for name in ["informed", "sampled_mc", "routine", "fixed_mc"]]
        gain = mc["mean_position"] - inf["mean_position"]
        pooled += [gain] * inf["races"]
        progression_ok &= inf["wins"] >= 1 and inf["median_position"] <= 3 and gain >= 0 and inf["mean_time_s"] is not None and routine["mean_time_s"] is not None and inf["mean_time_s"] < routine["mean_time_s"] and fixed["wins"] < fixed["races"]
    pooled_gain = sum(pooled) / len(pooled)
    progression_ok &= pooled_gain >= .5
    all_selected_finish = all(c.ok(row) for row in rows)
    hierarchy_ok = set(hierarchy[:2]) == set(policy["roster"]["front_runners"])
    audit = load(OUT / "native-field-audit-v3.json.gz")
    summary.update({"policy_digest": seal["policy_digest"], "budgets_by_era": policy["budgets_by_era"], "elite_level": policy["elite_level"], "fixed_roster": actors, "hierarchy": hierarchy, "field_scores": scores, "field_points_by_era": era_points, "constant_driver_resource_digests": {a: next(iter(hashes[a])) for a in actors}, "player_progression": progress, "player_races": races, "pooled_exit_rank_gain_over_sampled_mc": pooled_gain, "heldout_field_count": len(fields), "hierarchy_passed": hierarchy_ok, "progression_passed": progression_ok, "all_selected_cases_finish": all_selected_finish, "native_ten_car_checks": len(audit["fields"]), "independence_checks": len(audit["invariance"]), "verdict": "NATIVE_I_II_CANDIDATE" if hierarchy_ok and progression_ok and all_selected_finish else "REFINE_NO_ADMISSION"})
    export = {"schema": policy["schema"], "source_policy_digest": seal["policy_digest"], "not_admitted_to_runtime": True, "performance_profile": policy["performance_profile"], "roster": policy["roster"], "budgets_by_era": policy["budgets_by_era"], "field_contexts": policy["field_contexts"], "profile_derivation": policy["profile_derivation"]}
    export["driver_rng_namespace"] = "fixed-field:<driver_resource_id>"
    export["player_rng_namespace"] = "career-driver:player"
    export["base_vehicle_id"] = "classic_v8_1960"
    baseline = {}
    for row in rows:
        case = row["case"]
        if case["actor"] not in actors:
            continue
        era = str(c.FIXTURES[case["fixture_id"]]["era"])
        value = {name: row["data"]["receipt"][name] for name in ["engine_id", "components", "offsets"]}
        if era in baseline and baseline[era] != value:
            raise RuntimeError("field component/offset baseline changed within an era")
        baseline[era] = value
    export["installed_component_baseline_by_era"] = baseline
    export["tire_identity_note"] = "Baseline component metadata may name medium; the actual starting/replacement compound is selected by each case strategy and the compiled three-compound performance profile."
    write(EXP / "policy-candidate-v3.json", export)


def qualification(summary):
    """Expose every training rejection; this never selects a new candidate."""
    path = OUT / "training-qualification-v3.json"
    if not path.exists():
        return
    attempts = load(path)
    records = []
    expected = {(era, circuit, actor) for era, circuit in c.CONTEXTS for actor in c.ROSTER["drivers"]}
    for attempt in attempts:
        checks = attempt["pit_checks"]
        actual = {(p["context"][0], p["context"][1], p["actor"]) for p in checks}
        if actual != expected or len(checks) != len(expected):
            raise RuntimeError("strategy qualification omits or duplicates a driver/context")
        worst = min(checks, key=lambda p: p["extra_stop_margin_s"])
        records.append({
            "budgets": attempt["budgets"], "elite_level": attempt["elite_level"],
            "qualified": attempt["qualified"], "driver_context_checks": len(checks),
            "passed_checks": sum(p["passed"] for p in checks),
            "style_preferences_suppressed": sum(p["style_preference_suppressed"] for p in checks),
            "minimum_extra_stop_margin_s": worst["extra_stop_margin_s"],
            "worst_check": worst,
            "final_training_assessment": attempt["final_training_assessment"],
        })
    summary["training_qualifications"] = records


def render(summary):
    count = summary["execution_counts"]
    lines = ["# Plateau fixe de neuf IA · résultats natifs V3", "", f'**Verdict : `{summary["verdict"]}`.** Profil physique `{summary["performance_profile"]}` inchangé. Le jeu, les catalogues publiés, WASM et Authority/Verifier ne sont pas modifiés par ce rapport.', "", "## Calculs et stabilité", "", f'- {count["completed_native_gp"]:_} exécutions natives terminées, dont {count.get("forced_repeat_gp",0):_} répétitions forcées et {count.get("native_field_gp",0):_} simulations dans les assemblages natifs.'.replace("_", " "), f'- {summary["unique_semantic_cases"]:_} entrées distinctes de campagne ; la réutilisation des références du transfert est enregistrée séparément.'.replace("_", " "), f'- {len(summary["failed_simulations"])} simulation(s) exploratoire(s) en échec, toutes conservées.', f'- Continuité vitesse/moteur/surface/carcasse/usure : `{summary["maximum_carry_jumps_speed_engine_surface_carcass_wear"]}`.', f'- Résidu maximal tours/stands/temps total : `{summary["maximum_pit_accounting_residual_s"]:.3g} s`.', ""]
    if summary.get("training_qualifications"):
        lines += ["## Qualification réelle des stratégies", "", "Les marges comparent le meilleur diagnostic à trois arrêts ou plus au plan normal retenu. Une marge négative est un contre-exemple ; elle bloque la configuration avant toute évaluation réservée.", "", "| Budgets I/II | Niveau favoris | Contrôles réussis | Marge minimale | Préférences de style abandonnées | Qualification complète |", "|---|---:|---:|---:|---:|---|"]
        for q in summary["training_qualifications"]:
            lines.append(f'| {q["budgets"]["1"]}/{q["budgets"]["2"]} | {q["elite_level"]} | {q["passed_checks"]}/{q["driver_context_checks"]} | {q["minimum_extra_stop_margin_s"]:+.2f} s | {q["style_preferences_suppressed"]} | {"Passe" if q["qualified"] else "REFINE"} |')
            if not q["qualified"]:
                d = q["final_training_assessment"]
                lines += ["", f'Évaluation finale des budgets {q["budgets"]["1"]}/{q["budgets"]["2"]} : favoris `{d["top_two"]}`, victoires informées I/II `{d["difficulty"]["1"]["wins"]}/{d["difficulty"]["2"]["wins"]}`, gain de rang regroupé `{d["pooled_exit_rank_gain_over_sampled_mc"]:.2f}`.']
        lines += ["", "[Comparaisons par pilote](figures/pit-incentives-by-driver-v3.png) · [Reçus de qualification](results/training-qualification-v3.json).", ""]
    if summary["campaign_status"] != "COMPLETE_NATIVE_EVALUATION":
        candidates = load(OUT / "calibration-candidates-v3.json")
        lines += ["## Entraînement à reprendre", "", f'{sum(x["eligible"] for x in candidates)} calibration(s) approximativement éligible(s) sur {len(candidates)}. Aucun plateau final scellé et aucune donnée réservée ouverte. Les stratégies et traits ne deviennent pas des ressources de jeu sur la base de ces essais.', "", "| Budget I | Budget II | Niveau favoris | Victoires joueur I/II | Gain de rang regroupé vs MC | Critères |", "|---:|---:|---:|---|---:|---|"]
        for x in candidates:
            lines.append(f'| {x["budgets"]["1"]} | {x["budgets"]["2"]} | {x["elite_level"]} | {x["difficulty"]["1"]["wins"]}/{x["difficulty"]["2"]["wins"]} | {x["pooled_exit_rank_gain_over_sampled_mc"]:.2f} | {"Passe" if x["eligible"] else "REFINE"} |')
        if (OUT / "training-qualification-v3.json").exists():
            attempts = load(OUT / "training-qualification-v3.json")
            bad = [p for a in attempts for p in a["pit_checks"] if not p["passed"]]
            lines += ["", f'{len(attempts)} tentative(s) complètement évaluée(s) sous les vrais traits ; {len(bad)} comparaison(s) de stratégies en contre-exemple. Voir [les qualifications conservées](results/training-qualification-v3.json).']
    else:
        lines += ["## Plateau constant", "", f'Budgets discrétionnaires : **{summary["budgets_by_era"]["1"]} en I**, **{summary["budgets_by_era"]["2"]} en II**, identiques pour les neuf pilotes et indépendants du joueur. Niveau global des deux favoris : {summary["elite_level"]}. Tous utilisent Balanced, engagement 0,8.', "", "| Pilote | Limite | Régularité | Pneus | Points réservés | Rang moyen entre IA |", "|---|---:|---:|---:|---:|---:|"]
        for actor in summary["hierarchy"]:
            driver = summary["fixed_roster"][actor]
            traits = driver["resource"]["traits"]
            s = summary["field_scores"][actor]
            lines.append(f'| {driver["name"]} | {traits["limit_exploitation"]:.2f} | {traits["consistency"]:.2f} | {traits["tire_management"]:.2f} | {s["points"]} | {s["rank_sum"]/s["races"]:.2f} |')
        lines += ["", "## Progression du joueur", "", "Les budgets, composants et bonus sont ceux des snapshots de carrière, avec les commandes alors disponibles. Les rangs ci-dessous proviennent des conditions réservées, pas de nouveaux réglages ajustés sur leurs résultats.", "", "| Étape | Préparation | Rang moyen | Victoires | Courses terminées |", "|---|---|---:|---:|---:|"]
        for stage, styles in summary["player_progression"].items():
            for style, s in styles.items():
                lines.append(f'| {stage} | {style} | {s["mean_position"]:.2f} | {s["wins"]}/{s["races"]} | {s["finishes"]}/{s["races"]} |')
        lines += ["", f'Hiérarchie des favoris : `{summary["hierarchy_passed"]}`. Progression : `{summary["progression_passed"]}`. Toutes les préparations réservées terminent : `{summary["all_selected_cases_finish"]}`. Gain de rang regroupé du joueur informé face au concentré échantillonné : `{summary["pooled_exit_rank_gain_over_sampled_mc"]:.2f}`.', "", f'{summary["native_ten_car_checks"]} assemblages natifs et {summary["independence_checks"]} contrôles d’ordre/changement du joueur. Les traits effectivement exécutés correspondent aux ressources finales et leurs empreintes restent constantes.', "", "Les traits exportés sont déjà développés avec le niveau global retenu. Ne pas leur réappliquer ce niveau lors de leur future intégration."]
    lines += ["", "## Limites et conservation", "", "Cette campagne n’établit pas un optimum global. Les réglages sont issus d’une recherche bornée, les budgets inférieurs sont dérivés par redimensionnement des préparations de base, et la grille de stratégies reste discrète. Les résultats indépendants ne comportent pas de trafic, aspiration, collision, VSC ni météo dynamique. Seules les situations authored Craft Engineering des ères I/II sont comparées. Les autres doctrines/ères, le jeu dans le navigateur et son admission vérifiée restent des étapes distinctes.", "", "Les contre-épreuves à davantage d’arrêts sont réellement calculées avant acceptation en entraînement ; elles ne sont pas exclues pour masquer une meilleure stratégie. Les conditions réservées évaluent ensuite les plans scellés, sans réoptimisation des alternatives sur ces mêmes conditions.", "", "Les entrées, résultats, empreintes et métriques par tour sont conservés ; pas de télémétrie point à point exportée. [Protocole et reproduction](README.md) · [Résumé JSON](results/summary-v3.json) · [Reçu de campagne](results/campaign-v3.json)."]
    (EXP / "RESULTS_V3.md").write_text("\n".join(lines) + "\n")


def main():
    receipt = checked_campaign()
    summary = summarize(receipt)
    if receipt["status"] == "COMPLETE_NATIVE_EVALUATION":
        heldout(summary, receipt)
    else:
        summary["verdict"] = "REFINE_TRAINING_NO_HOLDOUT"
    qualification(summary)
    summary["source_campaign_digest"] = sha((OUT / "campaign-v3.json").read_bytes())
    summary["report_source_digest"] = sha(pathlib.Path(__file__).read_bytes())
    write(OUT / "summary-v3.json", summary)
    render(summary)
    print(json.dumps({name: summary[name] for name in ["verdict", "unique_semantic_cases", "execution_counts", "maximum_carry_jumps_speed_engine_surface_carcass_wear"]}, indent=2))


if __name__ == "__main__":
    main()
