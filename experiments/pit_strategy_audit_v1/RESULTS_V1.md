# Audit pneus/stands — cause de la stratégie à nombreux arrêts

**Décision : revoir les courbes d’adhérence avant de reprendre le plateau fixe.** La règle du jeu sera au plus deux arrêts volontaires, pour le joueur et les IA. Aucun plateau n’est sélectionné ni admis.

Source figée `a436a2bc6810e435eb8663961e09c7d1945b654e` ; **714 GP natifs** (700 comparaisons et 14 répétitions fraîches), plus deux contrôles historiques de qualification. **0 échec**, zéro rupture d’état aux changements de tours. Les **100 contrôles inchangés** reproduisent exactement les anciennes entrées physiques/diagnostiques, résultats, temps par tour, bilans et métriques.

## Les arrêts sont bien facturés

La différence entre le temps total et la somme des tours roulés vaut le nombre d’arrêts multiplié par le coût configuré, à **3.64e-12 s** près. Les contrôles héritent de **22 s par arrêt** ; la variante doublée utilise 44 s. Les 20 s mentionnées initialement étaient le défaut générique de SimConfig, pas le coût résolu par ces scénarios.

Le service inclut aussi le carburant au ralenti, les échanges thermiques et une remise à zéro abstraite de vitesse. Le temps reste agrégé ; aucune trajectoire géométrique dans les stands n’est simulée ici.

## Résultats appariés

| Contexte | Contrôle | Sans effet usure→grip | Sans effet direct T→grip | Sans les deux | Pente usure ÷4 | Montage 70°C | Stands 44 s |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| MONZA · I | 7 | 1 | 4 | 1 | 5 | 4 | 5 |
| SILVERSTONE · I | 8 | 1 | 4 | 1 | 5 | 5 | 5 |
| ZANDVOORT · I | 8 | 1 | 6 | 1 | 6 | 6 | 6 |
| SILVERSTONE · II | 6 | 1 | 6 | 1 | 4 | 4 | 4 |
| MEXICO · II | 7 | 1 | 7 | 1 | 4 | 5 | 5 |
| MONTREAL · II | 5 | 1 | 6 | 1 | 3 | 4 | 4 |
| MONZA · II | 5 | 1 | 5 | 1 | 3 | 4 | 4 |
| SAOPAULO · II | 6 | 1 | 6 | 1 | 4 | 5 | 5 |
| SPIELBERG · II | 5 | 1 | 5 | 1 | 4 | 4 | 3 |
| ZANDVOORT · II | 6 | 1 | 6 | 1 | 5 | 5 | 5 |

Chaque valeur est le nombre d’arrêts du meilleur plan testé sous cette intervention. Les mêmes plans source sont recalculés ; ils ne sont pas garantis optimaux sous une loi modifiée. L’ancre medium un tour puis soft isole le cas de grip statique. Ces ablations ne sont pas des propositions de nouveaux pneus : supprimer la perte d’adhérence laisse encore les états d’usure/chaleur calculés. Retirer l’effet direct de température ne retire pas son effet sur l’usure thermique.

La suppression de l’effet direct de l’usure fait gagner un plan à un arrêt dans les dix contextes. Doubler les stands laisse encore trois à six arrêts comme meilleurs choix testés. Le moteur principal de l’incitation est donc la dégradation d’adhérence avec l’usure, amplifiée par son couplage thermique. Réduire seulement la pente, modifier seulement le montage ou augmenter seulement le temps des stands ne suffit pas dans ce diagnostic.

[Comparaison visuelle des arrêts](figures/pit-incentives-v1.png).

## Incohérence de paramétrage identifiée

L’accumulation d’usure a été remplacée par un bilan de travail dissipé et de température, mais la loi héritée d’adhérence est restée linéaire : `max(wear_min, 1 - wear_grip_k × wear)`. Sur le tendre, `wear_grip_k=.6` : `.2` d’usure brute enlève **12 % d’adhérence** avant l’effet thermique. Ce coefficient n’a pas été recalibré pour la nouvelle signification de l’état d’usure.

À Monza I, le témoin à deux arrêts commence à 90°C et termine son huitième tour à environ 70,6°C. L’usure brute est alors proche de 19,9 %. La remise de pneus neufs à 90°C renouvelle à la fois le grip d’usure et l’état thermique. Cette observation ne valide pas un objectif thermique universel par ère ; elle révèle la nécessité d’aligner préparation, chauffe, fenêtre de grip et durée de vie opérationnelle.

[Courbes actuelles d’adhérence](figures/grip-curves-v1.png).

## Références de la correction suivante

L’[appel d’offres FIA 2025–2027, annexe F1 A2/A4](https://legal.fia.com/web/appeloffre.nsf/0F43EC70A0FACADFC1258991006043A9/%24FILE/ITT%20F1%20TYRES%202025-2027%20%28including%20F2%20and%20F3%29_UPDATE%2014.04.2023.pdf?openelement=) distingue durée de performance, usure et durabilité, et envisage une chute de performance non linéaire tardive. Son objectif 2025 conserve au moins 97 % du grip à ±20°C du pic. C’est un cahier de conception daté, pas une mesure applicable à toutes les ères ou un constat sur les pneus 2026. Notre gaussienne tendre actuelle (sigma38°C) ne conserve que 75,8 % à cet écart : son échelle doit être revue explicitement.

Le manuscrit [West et Limebeer, Optimal Tyre Management for a High-Performance Race Car](https://repository.up.ac.za/server/api/core/bitstreams/a08bc69d-ba39-4611-b73d-905d3f494118/content), daté de 2022, sépare les états thermiques et l’usure, et utilise une caractéristique d’adhérence non linéaire. Il motive une perte progressive suivie d’une fin de vie marquée ; ses paramètres ne sont pas transférés directement dans Pitgun. La [description de Pirelli au Qatar 2024](https://www.formula1.com/en/latest/article/what-the-teams-said-race-day-in-qatar-2024.51c2RZOH7QrgX8NhNsAYSI) confirme aussi qu’usure matérielle et baisse de performance peuvent différer fortement selon les conditions.

## Suite nécessaire

1. Donner au candidat thermique une courbe usure→grip explicitement versionnée et une définition de durée de vie opérationnelle ; conserver l’ancienne loi pour les anciens profils et replays.
2. Revoir les fenêtres thermiques et écarts entre composés, et qualifier la préparation au départ/aux stands avec des témoins de chauffe. Conserver les échanges énergétiques et le refroidissement déjà modélisés.
3. Vérifier les coûts agrégés par circuit et la continuité des tours ; comparer un/deux arrêts et garder les plans hors limite comme tests d’incitation, sans les admettre dans le jeu.
4. Raccorder la limite de deux arrêts à l’éditeur, aux plans, aux commandes live joueur/LLM et au replay indépendant, puis reprendre la campagne des neuf personnalités constantes. Franz Hermann et Charles L’Éclair restent les favoris à calibrer sur le modèle ainsi qualifié.

Les sources, anciens résultats, jeu local et déploiements ne sont pas modifiés par ces ablations. Les coefficients des variantes ne sont pas promus. Voir le [plan](plan-v1.json), le [résumé vérifié](results/summary-v1.json), les [reçus](results/paired-ablation-v1.json.gz) et le [reçu de campagne](results/campaign-v1.json).
