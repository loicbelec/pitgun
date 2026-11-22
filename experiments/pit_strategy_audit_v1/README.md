# Audit causal — incitation excessive aux arrêts

**Audit terminé : 714 GP, 100 contrôles source exacts et 14 répétitions exactes.**
La perte directe d'adhérence liée à l'usure explique l'essentiel de l'incitation ;
le doublement du temps de stand ne la corrige pas. Voir le
[rapport et les figures](RESULTS_V1.md). Aucun coefficient modifié n'est promu.

La calibration des neuf IA est interrompue avant sélection et avant les résultats
réservés. Le propriétaire fixe désormais **deux arrêts volontaires maximum** pour
le joueur comme pour les adversaires. Les diagnostics hors limite restent utiles
pour détecter un déséquilibre ; ils ne deviennent pas des plans de jeu admis.

## Ce qui est déjà établi

- Sur les 4 500 GP de stratégie V2, `temps_total - somme_temps_tours` vaut
  exactement **22 s × nombre d'arrêts**, à `5.46e-12 s` près. La valeur du
  catalogue circuit résolue est 22 s ; 20 s est uniquement le défaut générique
  de `SimConfig`, non utilisé dans ces cas. Les stands ne sont pas gratuits.
- Le coût est actuellement un temps agrégé, avec bilan thermique/carburant du
  service et remise à zéro abstraite de vitesse. Ce n'est pas une trajectoire
  géométrique complète dans la pit lane.
- Les meilleurs plans échantillonnés favorisent 5–8 arrêts, soit 18–240 s gagnées
  face à la borne de trois arrêts. Cela demande une revue physique avant de
  rendre les IA plus compétitives.
- La nouvelle accumulation d'usure énergétique/thermique conserve la loi héritée
  `grip_usure = max(plancher, 1 - k × usure)`. Le tendre garde `k=.6` : à `.2`
  d'usure brute, sa perte directe d'adhérence est déjà de **12 %**. La loi
  température-adhérence et les montages à 90°C peuvent aussi favoriser le pneu
  neuf. Le diagnostic doit distinguer ces effets, sans présumer de leur poids.

## Comparaisons

Le [plan figé](plan-v1.json) utilise les dix contextes I/II, les mêmes voitures,
réglages et seed d'entraînement. Pour chaque contexte, il reprend le meilleur
plan source à chaque nombre d'arrêts 1–8 et 12, et ajoute un témoin medium un
tour puis soft jusqu'à l'arrivée. Sept variantes sont entièrement recalculées :

1. contrôle physique inchangé ;
2. désactivation du seul effet direct de l'usure sur l'adhérence ;
3. désactivation du seul effet direct de la température sur l'adhérence ;
4. désactivation conjointe de ces deux effets directs ;
5. pente usure-adhérence divisée par quatre ;
6. montage initial et aux stands à 70°C ;
7. coût agrégé du service doublé à 44 s.

Ces ablations **ne proposent pas de nouveaux coefficients**. L'usure, la chaleur,
le carburant et les changements de trajectoire sont toujours calculés. Retirer
le facteur direct de température ne retire pas son effet sur l'usure thermique.
Les plans source ne sont pas des optima garantis sous une intervention : il
s'agit de comparaisons appariées et de témoins de mécanismes.

Les contrôles inchangés doivent reproduire les anciens digests, résultats, tours,
bilans et métriques exactement. Les reçus ajoutent les paramètres réellement
résolus et des observations par tour (états thermiques/usure, facteurs de grip,
temps et service). Aucun historique point à point n'est exporté. Tous les échecs
et les répétitions fraîches sont conservés. Les sources/binaire sont épinglés.

## Frontière avec le jeu

La limite de deux arrêts appartient à la **politique Racing du jeu**, pas au
solveur générique Pitgun. Lors du raccordement, elle doit être cohérente dans
l'éditeur, la validation du plan, les décisions live (joueur/LLM) et le replay
indépendant. Un troisième arrêt ne doit pas devenir possible par une commande
d'agent contournant l'éditeur. Ce raccordement n'est pas réalisé dans cet audit.

La limite ne dispense pas d'expliquer l'incitation physique excessive. Avant
reprise du plateau : comprendre les pertes d'adhérence, définir une durée de vie
opérationnelle lisible, vérifier les coûts de service par circuit et documenter
les hypothèses/coefficients. Aucun coefficient ne sera choisi uniquement pour
imposer un nombre d'arrêts. La hiérarchie de Franz Hermann/Charles L'Éclair reste
un objectif du futur plateau, sous le modèle ensuite qualifié.

```sh
cargo build --release --offline --locked -p pitgun-racing-simulator --example pit_strategy_audit
python3 experiments/pit_strategy_audit_v1/qualify.py target/release/examples/pit_strategy_audit
python3 experiments/pit_strategy_audit_v1/run_local.py target/release/examples/pit_strategy_audit /private/tmp/pitgun-pit-audit-cache
```

Adapter `CARGO_TARGET_DIR`. Exécution locale native, huit processus maximum.
Les équations partagées, les anciennes campagnes, le jeu et les déploiements
restent inchangés. Les interventions sont explicites dans ce probe diagnostique.
