# Lot 1 — puissance et comptabilité énergétique, première livraison locale

Cette livraison implémente une projection expérimentale versionnée
`pitgun.racing-power-telemetry/v1`. Elle corrige le calcul de charge/puissance
dans **cette nouvelle voie**, sans modifier la dynamique ni réinterpréter les
anciens canaux5008/5014. Le jeu local/staging/prod continue d'utiliser son moteur
figé : le défaut d'affichage historique n'y est pas encore remplacé.

Parent [419](https://github.com/loicbelec/pitgun/issues/419), refonte
[413](https://github.com/loicbelec/pitgun/issues/413),
[base lot0](FOUNDATION_2026-11-21.md), livraison locale
[442](https://github.com/loicbelec/pitgun/issues/442). La qualification de cette tranche native
n'est pas une clôture du lot mécanique/énergie ou de la livraison navigateur.

## Ce qui a été vérifié

Le canal actuel calcule `p_act/power_t`, le plafonne1,2 puis utilise
`p_act × ratio` pour la puissance exportée. `p_act` est une disponibilité moteur ;
`power_t` est une puissance issue de la solution après rendement/transitoires.
Inverser simplement le rapport ne suffit pas à réconcilier moteur et roues.

Usages relevés : transport et preuve/télémétrie Simulator, graphiques du Pit Wall,
observations déterministes d'ingénieur et projections Gemini. Le solveur ne relit
pas le canal5008 pour déterminer son accélération. Modifier le canal peut donc
conserver le résultat physique tout en changeant digest de télémétrie et analyses.
Une correction globale sous l'ancien runtime ne serait pas une simple retouche UI.

## Sémantique du nouveau contrat

| Grandeur | Définition et unité |
| --- | --- |
| `engine_available_w` | Puissance au vilebrequin issue de la courbe couple/régime et du facteur thermique natif ; W, avant interruption de rapport |
| `engine_delivered_w` | Charge moteur effectivement utilisée par les calculs de consommation/chaleur actuels ; W |
| `drive_budget_w` | Puissance positive stockée dans la solution après rendement et limitations ; W, budget dont le nom historique ne garantit pas `Fv` au démarrage |
| `wheel_propulsion_w` | Travail instantané propulsif `F_drive × v` à la vitesse native réelle ; W |
| `driveline_loss_w` | Différence moteur transmis/budget après rendement ; W |
| `launch_regularization_w` | Différence explicite budget−`Fv` due aux planchers de vitesse du modèle ; W, résidu numérique de modèle, pas une perte de friction mesurée |
| `equivalent_engine_load_pct` | `100 × moteur délivré/moteur disponible`, même base mécanique ;0–100% |
| `shift_power_fraction` | Facteur natif de disponibilité pendant le passage de rapport ; borné0–1 |
| `pedal_command_pct`, `brake_command_pct` | Non observés dans le solveur d'enveloppe actuel : `null`, pas un signal inventé |

La limite moteur après interruption de rapport est contrôlée avant calcul du
ratio. Un dépassement significatif est une erreur de cohérence ; le plafond100%
n'efface que l'arrondi inférieur à la tolérance explicite. Régime et rapport
viennent de la grille native, pas d'un rapport engagé interpolé puis arrondi.
Le facteur thermique est celui réellement enregistré par le solveur.

Pour le chemin actuel, en notant `η` son rendement et `v_safe=max(v,1m/s)` :

```text
P_engine = P_budget / η
F_drive  = P_budget / max(v_safe,10m/s)
P_wheel  = F_drive × v
P_transmission_loss = P_engine - P_budget
P_launch_residual   = P_budget - P_wheel

P_engine = P_wheel + P_transmission_loss + P_launch_residual
```

Ces égalités décrivent le modèle existant. Le résidu de lancement est conservé
visiblement au lieu de prétendre que le budget est toujours la puissance aux
roues. Le passage de rapport réduit une disponibilité dans le modèle actuel ;
on ne lui invente pas en plus une énergie dissipée indépendante.
Ce sont des identités comptables des ports déclarés, pas une validation
expérimentale ni encore la fermeture complète du bilan cinétique, traînée,
roulement et énergie potentielle du véhicule.

La projection et son rééchantillonnage résident dans
[le Solver Racing](../../crates/pitgun-racing-solver/src/power_telemetry.rs).
Le Simulator conserve seulement le petit contexte mécanique/carburant résolu
déjà utilisé par le Solver et propose une lecture facultative du dernier tour.
Aucun calcul physique n'est ajouté dans React, le contrôleur ou le framework
générique. La lecture n'avance pas la simulation et ne change aucun journal.

## Deux horloges, sans correction physique cachée

Le noyau actuel fait évoluer puissance intégrée, température et travail avec
`dt_force = ds/max(v,1)`. Il réintègre ensuite le temps de la trajectoire finale
avec les vitesses aux deux extrémités du segment. La projection conserve les
bilans sur chacune de ces horloges séparément.

Les bilans moteur/transmission/roues/résidu se ferment sur chaque horloge. Le
travail moteur de l'horloge force, les pertes et la chaleur se réconcilient avec
les diagnostics natifs. La consommation reproduit les coefficients existants :
travail moteur × consommation spécifique + débit de ralenti × durée du tour.

Cela ne ferme pas un bilan thermodynamique complet : pouvoir calorifique,
échappement, pertes de ralenti et autres échanges ne sont pas encore tous
modélisés. Le résidu thermique compare capacité×variation de température à la
chaleur et à l'échange **signé** avec l'ambiance ; un milieu plus chaud peut
apporter de la chaleur. Les modèles de carburant et de chaleur restent inchangés.

L'unification de ces horloges affecterait les états, puis potentiellement les
temps au tour : elle appartient à une évolution physique distincte, avec identité,
validation et arbitrage précision/coût. Une petite différence sur ces cas publics
ne garantit pas une petite différence pour tous les scénarios.

## Export et horizons d'observation

La projection s'effectue sur un seul tour natif avant raccordement des tableaux
de tours. Les états de début de tour et de pré-service ne sont ainsi pas perdus.
Les tableaux incomplets, temps/distance non croissants, rapports invalides,
valeurs non finies et historique multi-tour raccordé sont refusés.

Le nouvel export1Hz/5Hz fait un maintien constant du dernier nœud disponible,
commun à tous les ports. Il n'interpole ni un pourcentage ni une disponibilité
de rapport comme une nouvelle commande. Son nombre d'échantillons est borné ;
aucun bilan de référence n'est intégré depuis cet export. La cadence actuelle du
jeu reste5Hz : cette API ne l'a pas silencieusement changée.

Cette lecture est un outil d'audit local. Elle ne remplace pas les règles de
publication d'horizon du contrôleur : un futur raccordement ne pourra envoyer
au modèle que le préfixe observé et validé. Une portion déjà calculée n'est pas
automatiquement une observation autorisée pour un agent.

## Comparaison exécutée

Le [banc](../../experiments/power_telemetry/README.md) reprend les dix entrées
publiques figées du lot0 : ancien/modern/hybride nommé, quatre circuits,
grille dix voitures, Monza53 avec service26, Monaco78. Pour chaque cas :

1. Sans projection, sortie historique intégralement égale au lot0.
2. Avec projection, même résultat et même digest historique.
3. Deuxième exécution avec projection, nouvelle projection déterministe identique.

Le [rapport](../../experiments/power_telemetry/results/local-power-v1.json) et son
SHA archivent les bilans de chaque tour et les identités source/binaire/plan.
La normalisation respecte les puissances natives, sans modifier les vitesses,
rapports, températures, usure, carburant, arrêts ou classement.

| Cas | Charge historique max (%) | Nouvelle charge équivalente max (%) | Écart travail moteur, horloge trajectoire / force (%) |
| --- | ---: | ---: | ---: |
| monza-classic-5 | 120 | 100 | -0.0940 |
| monza-classic-5-1hz | 120 | 100 | -0.0940 |
| silverstone-classic-5 | 120 | 100 | -0.1052 |
| monaco-classic-5 | 120 | 100 | -0.1988 |
| suzuka-classic-5 | 120 | 100 | -0.1069 |
| monza-classic-53 | 120 | 100 | -0.0818 |
| monza-modern-5 | 120 | 100 | -0.1377 |
| monza-hybrid-resource-5 | 120 | 100 | -0.1215 |
| monza-classic-grid10-5 | 120 | 100 | -0.0940 |
| monaco-classic-78 | 120 | 100 | -0.1839 |

Les deux colonnes de charge ne sont pas deux mesures de pédale : la première
est le canal historique erroné, la seconde une charge moteur équivalente.
L’écart des horloges reste ici inférieur à0,2% en valeur absolue, sans garantie
pour d’autres entrées. Ce relevé ne constitue pas une validation expérimentale
du modèle.

Les essais comprennent aussi des cas élémentaires à résultats calculables :
arrêt/démarrage, charge partielle, absence de propulsion, rendement, réduction
thermique et interruption de rapport, refus d'une puissance incohérente,
séparation des horloges et export sans effet sur le bilan natif. Une exécution
incrémentale comparée au calcul direct prouve l'absence de modification de son
résultat physique tout en auditant chaque tour.

Vérifications :111 tests de bibliothèque Simulator et40 tests du noyau physique
passent (33 existants et7 nouveaux). Le build release, rustfmt et Clippy strict
sur les bibliothèques et la sonde passent. Une passe Clippy plus large
`--all-targets` révèle cinq warnings préexistants dans les tests : quatre
initialisations après `Default` et le helper `parity_artifact` à huit arguments.
Ils sont présents dans le commit lot0 ; ils ne sont ni masqués par des `allow`
globaux ni présentés comme une passe verte. Le jeu n'est pas modifié et ses tests
UI ne sont pas relancés. Le banc effectue30 exécutions natives sans réseau.
Les8 contrôles du runner lot0 et2 contrôles de cohérence des archives passent
également. Les seconds sont intégrés à la CI ; ils ne relancent pas une comparaison
bit à bit du modèle Mac sur Linux.

## Raccordement restant avant le jeu

- Décider les noms/canaux versionnés : une charge équivalente n'est pas une
  mesure de pédale. La demande de frein actuelle ne doit pas être déduite d'un
  simple zéro de propulsion. Les informations absentes doivent rester absentes.
- Adapter les graphiques et observations Engineer/Gemini au nouveau contrat,
  puis comparer leurs conclusions ; ne pas présenter les anciennes données
  comme si elles avaient toujours cette sémantique.
- Inclure la politique de télémétrie dans l'identité du nouveau modèle/runtime,
  qualifier les artefacts native/WASM et le Verifier ; conserver les anciens
  moteurs/bridges/contrats pour les sessions et preuves historiques.
- Traiter séparément le résidu de lancement et le couplage des horloges si les
  critères physiques justifient une modification. Cette projection les expose ;
  elle ne les supprime pas.

Aucun nouveau moteur n'est admis par Authority, aucun ancien catalogue n'est
modifié et aucune promotion hébergée n'est réalisée par cette livraison locale.
