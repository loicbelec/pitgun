#!/usr/bin/env python3
"""Render the human review from recorded evidence, without selecting or simulating."""
import collections
import gzip
import json
import pathlib

EXP=pathlib.Path(__file__).resolve().parent

def main():
 s=json.loads((EXP/'results/summary-v2.json').read_text())
 lock=json.loads((EXP/'results/selection-lock-v2.json').read_text());p=lock['policy']
 if s['status']!='COMPLETE':raise RuntimeError('completed evidence required')
 rows=['# Plateau fixe de neuf IA — résultats de la seconde calibration','',
  f"**Verdict : `{s['verdict']}`.** Candidat natif des ères I/II, à examiner avant admission dans le jeu.",'',
  f"La campagne V2 représente **{s['native_process_gp_executions']:,} exécutions natives de GP complets**, contrôles compris, après les **6 155** calculs d'entraînement de la V1 conservée. Total : **{s['combined_v1_v2_gp_executions']:,}**. Les **{s['execution_counts']['unchanged_imports']:,}** préparations inchangées importées ne sont pas comptées comme de nouveaux calculs. Les imports sont justifiés par 14 contrôles natifs exacts ; aucun résultat d'IA nommée n'est importé.",'',
  f"Source figée V2 : `{s['source_commit']}`. Identité de sélection : `{s['selected_policy_digest']}`. Les capacités finales, budgets, réglages et stratégies ont été verrouillés avant les résultats réservés. La physique, les capacités du joueur et les tolérances n'ont pas changé.",'',
  '## Les neuf personnalités','',
  'Les trois capacités physiques restent identiques sur tous les circuits, seeds et ambiances. Tous utilisent le mode Balanced, comme le joueur de référence (`.84/.84/.84`). Une seule identité de ressource effective par pilote est vérifiée dans les résultats réservés. Aucun bonus de chrono ni ordre d’arrivée imposé. Les noms sont des personnages fictifs, pas une estimation des capacités de pilotes réels.','',
  '| Pilote | Exploitation de la limite | Régularité | Gestion des pneus | Points du benchmark | Position moyenne parmi 9 | Premières places parmi 9 |',
  '| --- | ---: | ---: | ---: | ---: | ---: | ---: |']
 for a in s['hierarchy']:
  d=s['fixed_roster'][a];t=d['resource']['traits'];v=s['field_only_scores'][a]
  rows.append(f"| {d['name']} | {t['limit_exploitation']:.2f} | {t['consistency']:.2f} | {t['tire_management']:.2f} | {v['points']} | {v['rank_sum']/v['races']:.2f} | {v['wins']}/{v['races']} |")
 rows+=['',
  'Les points ci-dessus utilisent seulement le barème de comparaison annoncé (25/18/15/12/10/8/6/4/2), pas la réputation ni les récompenses économiques du jeu. Ils agrègent 120 classements des neuf IA : dix contextes circuit/ère × quatre seeds réservés × trois ambiances fixes. Ces cas corrélés ne sont pas 120 observations statistiques indépendantes de joueurs.','',
  f"Budget discrétionnaire commun aux neuf : **{p['budgets_by_era']['1']} en ère I**, **{p['budgets_by_era']['2']} en ère II**. Offsets HQ fixes respectifs `[0,0,0,0]` et `[2,1,4,1]` (aéro/châssis/moteur/refroidissement) ; moteur 1960 en I et V8 1970/aéro basic en II. Le budget ne suit ni l'allocation, ni les points non dépensés, ni le résultat du joueur.",'',
  'Les réglages et plans de pneus varient par circuit. Les deux favoris retiennent leur meilleur choix échantillonné ; les sept autres expriment leur préférence de préparation à moins de 0,15 % du meilleur temps mesuré pour eux. Ce sont des configurations réellement simulées, sans pénalité ajoutée après calcul. Le JSON de politique expose les 90 préparations.','',
  '## Progression face au même plateau','',
  '| Étape de carrière | Réglage initial | Recette fixe moteur/châssis | M/C échantillonné et stratégie améliorée | Préparation informée |',
  '| --- | ---: | ---: | ---: | ---: |']
 for stage,label in [('era1-entry','Entrée I'),('era1-exit','Sortie I'),('era2-entry','Entrée II, ancien moteur'),('era2-exit','Sortie II')]:
  def cell(style):
   v=s['player_progression'][stage][style];return f"P{v['mean_position']:.2f} · {v['wins']}/{v['races']} victoires"
  rows.append('| '+label+' | '+' | '.join(cell(style) for style in ['routine','fixed_mc','sampled_mc','informed'])+' |')
 rows+=['',
  'Chaque colonne utilise les mêmes neuf adversaires dans les mêmes conditions. Le joueur conserve ses budgets réellement acquis, les offsets HQ, composants et réglages débloqués des snapshots craft_engineering. L’entrée II peut encore utiliser le moteur 1960 : ce déficit technologique est volontairement séparé d’un essai à matériel identique. Ce sont des contrôleurs expérimentaux et des préparations transférées/améliorées, pas des comportements observés de joueurs ni une preuve d’optimum global.','',
  '## Recherche et garde-fous','',
  '- V1 : 27 calibrations rejetées, aucune victoire informée en sortie d’ère. Les données négatives restent [publiées](../fixed_opponents_v1/RESULTS_V1.md). Aucun résultat réservé n’avait été ouvert.',
  '- V2 : base des trois capacités diminuée de 0,06 pour les neuf ; trois niveaux globaux des deux favoris et trois budgets par ère, sélectionnés uniquement sur l’entraînement. Les écarts de personnalité sont conservés.',
  '- 1 551 réglages initiaux, raffinement borné et 4 500 plans de course complets. Les plans incluent 1–8, 12 et le nombre maximal légal d’arrêts ; toutes les combinaisons de composés ne sont pas explorées au-delà de trois arrêts. Adaptation locale de chaque pilote à partir des meilleurs réglages/plans de référence.',
  '- Cible annoncée : les favoris sont les deux premiers agrégés ; le joueur informé de sortie d’ère gagne au moins un GP, a une position médiane ≤3 sur l’entraînement et améliore son temps moyen face au réglage initial. La calibration éligible la plus exigeante en budget est verrouillée avant les essais réservés.',
  '- Réserve : quatre seeds non utilisés dans la sélection, ambiances héritée / air20°C-sol30°C / air40°C-sol50°C. Les plans verrouillés ne s’adaptent pas après observation de ces résultats.',
  f"- **{len(s['failed_simulations'])} échec(s)** parmi les entrées diagnostiques publiées. Ruptures maximales vitesse/moteur/surface/carcasse/usure : `{s['maximum_carry_jumps_speed_engine_surface_carcass_wear']}`.",
  f"- {s['native_ten_car_fields_checked']} lots natifs de dix simulations indépendantes comparés exactement aux calculs séparés ; {s['reorder_and_player_independence_checked']} contrôles d’invariance à l’ordre des pilotes et à une modification des points du joueur. Aucun couplage trafic/collision n’est ajouté par ce regroupement.",
  f"- {s['phase_stats']['reserved-exact-repeat']['requests']} répétitions réservées en processus neufs, toutes exactes. {s['conservative_recompute_laps']} tours recourent au recalcul conservateur du solveur borné, au plus {s['maximum_cap_updates']} mises à jour du cap. Les mêmes tolérances sont conservées.",'',
  '## Tendance stratégique à revoir avant intégration','',
  'L’extension au-delà de trois arrêts révèle des meilleurs plans de référence à **5–8 arrêts** selon le contexte, pour **18–240 secondes** gagnées face au meilleur plan limité à trois arrêts. La pénalité de stand héritée est bien de 22 s par arrêt ; elle n’a pas été supprimée. Ce nombre élevé appelle une revue de l’équilibre usure/performance des composés/coût du service avant admission. Ce n’est ni une validation de stratégie F1 réelle ni un motif pour modifier les lois pendant la sélection du plateau.','',
  'La hiérarchie des neuf pilotes et la progression du joueur sont des résultats conditionnels à ce modèle figé. Elles peuvent être revues après une évolution physique : les ressources finales ne doivent pas être présentées comme une politique publique déjà équilibrée.','',
  '## Limites et admission suivante','',
  'Cette campagne qualifie une première politique native I/II, sur les circuits réels des snapshots de carrière. Elle ne qualifie pas les ères III+, un championnat de carrière complet avec achats intermédiaires, les trois doctrines, ni une optimisation exhaustive des points/curseurs/stratégies. Les préférences de préparation sont des règles déterministes, pas des agents qui apprennent des réglages du joueur.','',
  'Les simulations indépendantes n’ont pas de trafic partagé, aspiration, collision, pluie dynamique, VSC ou accident réel. Le risque pilote représente des hésitations récupérables de la commande moteur. Tous les modes restent Balanced ; une diversification des modes réclamera une qualification distincte et des commandes équitables côté joueur. Les quatre seeds ne suffisent pas à étalonner une loi d’accident industriel.','',
  'Le protocole conserve la préparation thermique historique des snapshots (90°C), distincte de la politique moderne explicite à 70°C. Les questions de réserve carburant et d’applicabilité par ère restent ouvertes. Aucun coefficient physique n’est modifié pour forcer une répartition des points : l’aéro précoce faible et les extrémités de boîte restent suivies dans #469. Un optimum concentré disposant déjà de refroidissement HQ peut être légitime.','',
  'Avant jeu : compiler les ressources finales une seule fois (ne pas réappliquer elite_level à leur snapshot déjà développé), versionner la politique/catalogue, raccorder la résolution par voiture et les identités stables, puis prouver la parité native/WASM et le replay Authority/Verifier. Les reçus distinguent la résolution mécanique commune intermédiaire et la ressource pilote effectivement appliquée ; le futur serveur devra résoudre et lier cette dernière lui-même. Un verdict descriptif du client ne suffit pas.','',
  'Le jeu local, les sauvegardes personnelles et les services déployés ne sont pas modifiés. Aucun coût navigateur, WASM ou VPS n’est déduit des huit processus natifs de campagne.','',
  '## Pièces de revue','',
  '- [Politique finale candidate](policy-candidate-v2.json) et [sélection verrouillée](results/selection-lock-v2.json).',
  '- [Résumé vérifié](results/summary-v2.json), [provenance](results/provenance-v2.json) et [reçu de campagne](results/campaign-v2.json).',
  '- [Import contrôlé](results/control-import-v2.json), [qualification](results/probe-qualification-v2.json) et reçus compressés de chaque phase dans results.',
  '- [Hiérarchie](figures/fixed-field-hierarchy-v2.png) et [progression](figures/player-progression-v2.png).',
  '- [Protocole](plan-v2.json), [méthode et reproduction](README.md).',
  '- `summarize.py` vérifie les sources, reçus, imports, verrouillage, répétitions et traits constants ; `plot_report.py` et `render_results.py` dérivent les figures et ce rapport, sans recalcul de simulation.','']
 strategies=json.loads(gzip.decompress((EXP/'results/strategy-search-v2.json.gz').read_bytes()));groups=collections.defaultdict(list)
 for r in strategies:groups[(r['case']['fixture_id'],r['case']['circuit'])].append(r)
 strategy_table=['| Contexte de référence | Arrêts du meilleur plan testé | Gain face au meilleur ≤3 arrêts | Pic d’usure brute |', '| --- | ---: | ---: | ---: |']
 for (fixture,circuit),cases in sorted(groups.items()):
  good=[r for r in cases if r['data']['outcome']['ok']];b=min(good,key=lambda r:r['data']['outcome']['time_s']);old=min((r for r in good if len(r['case']['strategy']['pit_laps'])<=3),key=lambda r:r['data']['outcome']['time_s']);gain=old['data']['outcome']['time_s']-b['data']['outcome']['time_s'];era='I' if 'era1' in fixture else 'II'
  strategy_table.append(f"| {circuit} · {era} | {len(b['case']['strategy']['pit_laps'])} | {gain:.2f} s | {100*b['data']['metrics']['peak_wear']:.1f} % |")
 strategy_table+=['','Le pic d’usure brute est une variable physique du diagnostic, pas une jauge de durée de vie restante. La fréquence des arrêts et cette sémantique doivent être examinées ensemble. La courbe des [plans à 3–12 arrêts](figures/strategy-stop-count-v2.png) rend visible le biais de la première borne.','']
 at=rows.index('## Limites et admission suivante');rows[at:at]=strategy_table
 (EXP/'RESULTS_V2.md').write_text('\n'.join(rows))
 print(EXP/'RESULTS_V2.md')

if __name__=='__main__':main()
