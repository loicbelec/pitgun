# Refonte physique — lot 0, 21 novembre 2026

Le lot 0 établit une base de comparaison locale : inventaire du chemin physique
actuel, bibliographie commentée et banc exécutable. Il ne modifie aucune équation,
ressource du catalogue, identité de modèle, fréquence exportée ou configuration
du jeu. Aucun service staging/prod, résultat public ou fournisseur LLM n'est appelé.

Suivi du lot : [framework440](https://github.com/loicbelec/pitgun/issues/440).
Parent : [framework413](https://github.com/loicbelec/pitgun/issues/413).
Ce document actualise l'état des chemins exécutés ; l'ancien
[audit des approximations](../RACING_MODEL_APPROXIMATION_AUDIT_V1.md) conserve ses
constats historiques. Une ancienne lacune corrigée par V3 n'est pas réintroduite
comme justification d'une nouvelle réécriture.

## 1. Identités à distinguer

| Niveau | Référence de départ | Signification |
| --- | --- | --- |
| Sources auditées | main26ef213, avec moteur Racing inchangé depuis8b4648f | Sources de la livraison compacte, pas une nouvelle physique |
| Staging applicatif | frontend94eb396 ; catalogue1.8.1 ; modèle logique0.14 | Références du reçu de livraison, non relues en ligne par le banc |
| WASM interactif de référence |9af4de2f… ; bridge99691425… | Artefacts navigateur figés ; pas l'identité du binaire natif de ce banc |
| Sélecteur carrière | `pitgun.local-career-power-unit-thermal/v1` | Branche explicitement choisie par `seasonCareerControl.ts` |
| Profil physique effectif | profil V9, cœur composant0.11 ; thermique par moteur installé | Sélection de `LocalControlledRacingSession::start`, sans timeline de conduite |
| Banc ajouté | exemple natif `physics_baseline_probe`, SHA du vrai exécutable | Même branche physique, entrées synthétiques publiques, sans permission/signature hébergée |

Le catalogue0.14/timeline, le cœur0.11 utilisé par la carrière et la branche
carburant0.15 sont des identités distinctes. Le champ `physics` et la résolution
de catalogue sont nécessaires pour savoir ce qui tourne. Le banc ne remplace pas
ce sélecteur par le profil expérimental par défaut0.9 ou le modèle carburant0.15.
Il n'interprète pas ces différentes couches comme une preuve de fraude ou de
non-déterminisme : il les rend explicites pour éviter des comparaisons invalides.

## 2. Propriété des calculs

```text
Développements / réglages / composants sélectionnés
        -> Simulator + catalogue immuable : véhicule physique résolu
        -> Solver Rust : trajectoire, états et diagnostics par tour
        -> Simulator : chronologie, stands, classement, télémétrie
        -> Worker / interface : transport, lecture et rendu

Contrôleur -> proposition bornée -> validation domaine -> effet futur journalisé
```

Les points, doctrines et prix restent hors des équations du Solver V3. Un
contrôleur humain/ML ne change pas les équations ; il propose des commandes
autorisées. La cadence du rendu, le temps mur et le réseau ne pilotent pas le
temps physique. Toute nouvelle physique aura sa propre identité et conservera
les anciens moteurs nécessaires au replay.

## 3. Inventaire des équations exécutées

Sources principales : [Solver](../../crates/pitgun-racing-solver/src/lib.rs),
[Simulator](../../crates/pitgun-racing-simulator/src/lib.rs),
[sélecteur carrière](../../crates/pitgun-racing-simulator/src/control_prototype.rs),
[profil thermique](../../crates/pitgun-racing-simulator/src/thermal_profile.rs).
Les identifiants ci-dessous sont des points d'entrée précis pour la revue.

| Sous-modèle / entrée source | État et équation actuels | Décision initiale / limite |
| --- | --- | --- |
| Géométrie, `segment_distance`, `vertical_acceleration` | Distance par segment ; courbure plane ; pente ; accélération verticale V3 `v² × d(pente)/ds` | Conserver la structure ; contrôler qualité/géométrie réelle, unités et sensibilité au maillage. Le chemin ancien à pas uniforme reste historique. |
| Mouvement, `run_resolved_simulation_kernel_incremental` | Enveloppe de virage, passe de freinage arrière, accélération avant ; vitesse finale minimum ; temps intégré sur distance | Conserver comme modèle réduit ; mesurer convergence/couplage et cohérence des états calculés avec les vitesses finales. |
| Masse, `VehicleState::total_mass_delta` | Masse vide + carburant ; carburant constant pendant le tour, mis à jour à sa fin | Limite explicite ; distinguer mise à jour intra-tour et précision utile avant de complexifier. |
| Aéro, `aero_forces`, `resolve_v3_mechanical_params` | `D=q CdA`, `L=q ClA`, `q=ρv²/2` ; paire fixe résolue en V3 ; coût d'appui et effets des développements | Conserver les lois de force ; réexaminer la carte réglage→surfaces et son identification. Le choix implicite par courbure concerne les anciens chemins. |
| Contact, `aggregate_tire_force_capacity_v3` | `Fcap=μ N (N/Nref)^(-exposant)` | Conserver comme hypothèse agrégée à valider ; aucune roue, transfert avant/arrière ou suspension détaillée. |
| Adhérence combinée, `remaining_longitudinal_force` | Budget circulaire : capacité longitudinale restante `sqrt(max(Fcap²-Fy²,0))` en traction/freinage | Conserver la cohérence commune ; comparer éventuellement cercle/ellipse. Un taux d'utilisation plafonné ne démontre pas seul l'absence de dépassement brut. |
| Grip pneu, `effective_mu` | Facteurs gomme, usure linéaire bornée et fenêtre thermique gaussienne bornée | Réexaminer forme, plages et identifiabilité ; pas une loi universelle du caoutchouc. |
| Thermique pneu, branche `AggregateV2` | Travail proxy `Fcap × v × utilisation²` ; chaleur fraction de ce proxy ; retrait `(h0+hv v)(T-Tamb)` ; capacité thermique unique | À confronter aux références : ce proxy n'est pas une mesure de puissance dissipée par glissement. Comparer un/deux états thermiques seulement si données et utilité le justifient. |
| Usure pneu, `TireDegradationParamsV3` | Usure de base + travail proxy normalisé, corrigés par écart thermique ; état borné0–1 ; remise à zéro au service | Recoupler à la dissipation et au grip ; distinguer usure, température et vie utile affichée. Ne pas imposer un arrêt à100%. |
| Moteur / transmission, `power_kw_from_rpm`, `select_sequential_gear_v3` | Couple interpolé, `P=τ rpm π/30` dans les unités historiques ; rapports séquentiels, interruption de passage, rendement | Conserver ces bases ; formaliser vilebrequin/roue, unités du couple et puissance réellement demandée/délivrée. |
| Thermique moteur, `EngineThermalParamsV3` | `C dT/dt=α Pchargée -(h0+hv v)(T-Tamb)` ; seuil de réduction linéaire ou progressif ; plancher de puissance | Recoupler pertes énergétiques/thermiques ; aucune raison de forcer une surchauffe sur tous les moteurs. |
| Refroidissement, résolution thermique par power unit | Points→capacité d'évacuation ; profils propres au moteur ; coût aérodynamique optionnel | Mesurer influence et coût à budget égal, dans le domaine concerné. Les V8 peuvent légitimement ne pas dégrader leur puissance sur les cas nominaux. |
| Carburant, `FuelMassParamsV3` | Travail moteur intégré × consommation spécifique + débit de ralenti ; kg retirés à la fin du tour ; panne explicite si manque | Conserver un bilan inspectable ; efficacité globale, pouvoir calorifique et répartition des pertes ne ferment pas encore un bilan thermodynamique complet. |
| Freins | Capacité mécanique et adhérence, sans température de disque/étrier dans ce chemin | Nouveau sous-modèle réduit à étudier ; prévoir partage futur freinage électrique/mécanique. |
| Batterie / récupération | Aucun état de charge ni flux électrique dans la branche carrière ; le nom `v6t_hybrid` ne les implémente pas | À construire dans246 après clarification des puissances ; pas de correction par simple bonus de puissance. |
| Conducteur | Enveloppes physiques d'utilisation issues des traits classiques ; V9 ne sélectionne pas les ressources de contrôle V11/timeline | Documenter le profil réellement actif ; distinguer agressivité, erreur déterministe et futurs incidents. Aucun accident/VSC ajouté ici. |
| Course / environnement | Voitures calculées indépendamment, hiérarchisées par chronologie ; arrêts et commande de service ; ambiance pneu35 °C, départ pneu90 °C sur ce chemin | Pas de sillage, défense, collision, pluie ou réseau météo physique. Températures environnementales devront devenir un scénario cohérent, pas des constantes dispersées. |

### Unités et bilans à verrouiller

Distance m, vitesse m/s dans le solveur et km/h dans le canal5005 ; forces N ;
énergie J dans les intégrations puis kJ/MJ dans certains diagnostics ; consommation
spécifique kg/kWh ; températures absolues stockées en°C et différences en K/°C.
Les surfaces `CdA/ClA` sont en m², pas des coefficients sans dimension isolés.
La formule couple/régime actuelle interprète les valeurs de couple comme kN·m
pour produire des kW : le champ historique `trq` ne porte pas cette unité dans
son nom. Ce contrat doit devenir explicite dans une évolution versionnée.

Chaque futur sous-modèle doit fournir un bilan avec une tolérance issue d'une
étude numérique. La somme de diagnostics de chaleur et consommation n'est pas
déjà une preuve de fermeture énergétique. La récupération ne peut ni doubler
le travail des freins, ni charger une batterie pleine, ni dépasser les limites
de machine/stockage. Aucun résidu physique n'est inventé par le présent banc.

### Couplage numérique et trois fréquences

V3 utilise quatre itérations fixes de couplage pneu. Cela rend le coût borné et
l'ordre reproductible, sans constituer un critère de convergence mesuré. Les
états thermiques/travail sont calculés dans la marche avant ; une revue devra
quantifier leur accord avec l'enveloppe finale et le temps réintégré. Ces limites
sont des hypothèses à tester, pas des erreurs quantifiées par le lot 0.

Le pas spatial/les temps locaux de l'intégrateur, l'export et le rendu sont
distincts. **L'export du chemin actuel est fixé à 5 Hz dans `advance` et
`build_output`**, malgré le champ `hz` accepté par les requêtes. Le couple1Hz/5Hz
du banc vérifie l'égalité physique mais constate aussi les mêmes nombres
d'échantillons : changer ce champ seul ne réduit donc pas aujourd'hui le volume.
Cette information doit guider une future correction versionnée, pas une baisse
silencieuse de la fréquence au milieu de ce chantier.

## 4. Banc exécuté et interprétation

Le [banc local](../../experiments/physics_foundation/README.md) utilise exclusivement
les bibliothèques Rust actuelles et le catalogue local1.8.1. Dix cas publics sont
répétés dans deux processus frais chacun : quatre circuits, trois générations
de ressources,5 tours, Monza 53/arrêt26, Monaco 78 et une grille dix voitures/5 tours.
Entrées légales synthétiques, seed42, géométrie du catalogue sans override GPS du
navigateur ; ce ne sont pas des courses sauvegardées de joueurs.

Le [rapport JSON](../../experiments/physics_foundation/results/local-baseline-v1.json)
conserve entrées, résolutions, hashes et mesures. Son sidecar SHA permet de
contrôler les octets archivés. La sortie complète n'est pas exportée en JSON,
mais le moteur conserve encore ses historiques en mémoire : les pics restent
mesurés, pas effacés par le petit format de rapport. Le banc peut se relancer
avec `--check` pour comparer les valeurs déterministes sans comparer les temps.

Constats observés sur cette matrice :

- Les dix cas se terminent et leurs deux répétitions donnent exactement le même
  résumé et digest canonique dans le même binaire natif.
- Le canal accélérateur atteint 120 % sur ces cas : défaut de reconstruction
  reproductible, distinct d'une puissance hybride. Le code de resampling calcule
  le rapport disponible/puissance issue de la solution, plafonné1,2, puis réutilise ce rapport
  pour reconstruire la puissance. Ne pas corriger cela par un clamp React.
- Les diagnostics de ces cas donnent 0 s de réduction thermique moteur. La matrice
  ne varie pas le refroidissement ni l'ambiance et les cas modernes ne durent
  que 5 tours : aucune conclusion globale sur l'efficacité du cooling n'en découle.
- Monaco 78 classique réussit avec ce réglage. Cela ne résout pas414 : des moteurs,
  budgets, géométries et stratégies différents peuvent toujours épuiser le fuel.
- La paire `hz=1/hz=5` conserve les mêmes métriques physiques **et le même nombre
  d'échantillons exportés**, conformément au 5 Hz codé en dur.

| Cas | Temps médian processus (ms) | Pic RSS processus (Mio) | Accélérateur max (%) |
| --- | ---: | ---: | ---: |
| monza-classic-5 | 469.6 | 148.0 | 120.0 |
| monza-classic-5-1hz | 473.0 | 147.9 | 120.0 |
| silverstone-classic-5 | 483.2 | 162.1 | 120.0 |
| monaco-classic-5 | 393.3 | 118.9 | 120.0 |
| suzuka-classic-5 | 482.4 | 152.0 | 120.0 |
| monza-classic-53 | 2862.3 | 1239.2 | 120.0 |
| monza-modern-5 | 442.7 | 130.7 | 120.0 |
| monza-hybrid-resource-5 | 427.7 | 123.5 | 120.0 |
| monza-classic-grid10-5 | 839.1 | 206.4 | 120.0 |
| monaco-classic-78 | 3021.5 | 1346.7 | 120.0 |

Mesures de ce Mac arm64, Rust1.89.0/Cargo1.89.0, build release, un job,
deux processus frais par cas. Le passage `--check` suivant rejoue encore les
vingt exécutions et retrouve les sorties déterministes. Ce tableau ne mesure
aucun gain, aucune performance navigateur et aucun budget du VPS.

Les mesures native/processus sont des références initiales sans comparaison avant/
après, sans garantie de vitesse du navigateur ni du VPS. La qualification des
artefacts WASM et du Verifier reste distincte. Ces cas déjà vus ne seront jamais
rebaptisés validation indépendante d'une future calibration.

## 5. Bibliographie commentée

Les [références structurées](REFERENCES_V1.json) distinguent publication primaire,
documentation officielle, profondeur de lecture et limites. Elles sont toutes
**candidates à une adoption**, pas des équations déjà implémentées dans Pitgun.

| Référence | Utilisation envisagée | Limite de la lecture actuelle |
| --- | --- | --- |
| [Limebeer et al., NASCAR,2024/2025](https://doi.org/10.1080/00423114.2024.2342534) | Couplage usure/température/grip et thermique des freins ; données instrumentées et identification | Résumé/extraits indexés consultés ; accès direct intégral403. Lire les dérivations avant adoption ; ne pas recopier les coefficients NASCAR. |
| [Sorniotti,2009](https://doi.org/10.4271/2009-01-0441) | Température et réponse d'un modèle de contact de type brush | Résumé éditeur/notice universitaire ; équations complètes à examiner. |
| [Brooker et al., FASTSim,2015](https://doi.org/10.4271/2015-01-0973) | Bilans véhicule, rendements, limites de puissance et récupération | Résumé primaire du laboratoire ; validation drive-cycle différente du temps au tour. |
| [Sulzer et al., PyBaMM,2021](https://doi.org/10.5334/jors.309), [Thévenin25.12](https://docs.pybamm.org/en/v25.12.0/source/api/models/equivalent_circuit/thevenin.html) | Référence batterie hors ligne ; comparer stock d'énergie simple et circuit équivalent si nécessaire | Structure documentée ; paramètres cellule et validation toujours nécessaires. Aucune dépendance ajoutée au jeu. |
| [NASA, traînée induite](https://www1.grc.nasa.gov/beginners-guide-to-aeronautics/induced-drag-coefficient/) | Cohérence des surfaces de référence et principe du coût de portance/appui | Une aile finie n'est pas une carte aéro validée de voiture à effet de sol. |
| [pypowsybl](https://powsybl.readthedocs.io/projects/pypowsybl/en/stable/) | Futur adaptateur d'un solveur de réseau électrique existant | Aperçu des API ; aucun couplage exécuté. Version et réseau à figer avant expérience. |

Des références spécialisées restent à sélectionner pour l'aéro automobile et le
refroidissement moteur/radiateur avant de figer leurs nouvelles lois. Une
bibliographie est un début de travail, pas une certification industrielle.

## 6. Ordre des prochains lots et critères de décision

1. **Mécanique/énergie**419 : définir commande pédale, charge, puissance au
   vilebrequin/roues, pertes et consommation ; traiter d'abord le défaut
   observé 120 %. Séparer correction de canal et changement de performance.
2. **Pneus/freins**421 : comparer lois de grip/usure et un/deux états thermiques,
   dimensionner le niveau minimal de frein thermique ; tests élémentaires,
   sensibilité et données avant calibration de stratégies.
3. **Aéro/refroidissement**420 : rendre explicite le compromis et les conditions
   de validité, tester à budget égal, conserver les comportements plausibles
   sans imposer un minimum universel de cooling.
4. **Hybride**246 : stock borné, récupération et déploiement, rendements,
   auxiliaires, limite de charge et thermique ; politique déterministe avant
   commandes joueur/ML. Puis comparer ICE/hybride/électrique si les données le
   permettent. Gains d'efficacité et bilan carbone de cycle de vie restent
   deux questions distinctes.
5. **Qualification** : annoncer critères ACCEPT/REFINE/REJECT après pilote puis
   réserver des scénarios distincts de calibration ; mesurer erreur numérique,
   CPU/RSS, nombre d'évaluations et bénéfice du niveau de fidélité ; qualifier
   native/WASM/Verifier avant promotion. Recalibrer les IA concurrentes ensuite.

Les campagnes se font localement avec manifestes et résultats compacts. Aucun
service Databricks/MLflow/lakehouse n'est une porte obligatoire. La réexécution
exacte et la validation physique sont deux preuves différentes ; une politique
de comparaison inter-architectures doit déclarer ses tolérances au lieu de
promettre la parité bit à bit sans mesure.

Le moteur de jeu et un futur modèle de référence conservent des identités et
domaines de validité distincts. Un modèle plus détaillé ne devient la référence
que pour des phénomènes et données où sa valeur a été démontrée.

## 7. Énergie, drones et publication

Le propriétaire travaille désormais chez RTE après Alpine F1 ; le lien avec
pypowsybl rend la piste énergie intéressante. Aucun accès à des données internes,
engagement RTE ou partenariat LF Energy n'est déduit de ce contexte. Le choix du
second cas sera fait avec lui ;416 ne force pas une batterie synthétique si un
autre problème énergétique éprouve mieux le framework.

Le commun reste identité/temps logique, horizon observable, événements,
commandes admissibles, frontière d'effet, journal et replay. Les lois, unités,
états, géométrie et solveurs restent du domaine. Un réseau peut être un graphe
et un drone un état vectoriel/une trajectoire3D ; aucune API commune ne doit
supposer un tour ou une position1D. Ne pas extraire maintenant une bibliothèque
énergétique universelle. Un futur adaptateur peut réutiliser un solveur existant
et conserver ses versions/convergence, sans exiger qu'il soit réécrit en Rust.

`pitgun.dev` pourra publier une fiche par phénomène : références exactes,
équations retenues, adaptations, paramètres/données, validation, coût, limites
et version. Son discours actuel sur AST universel, boucle100ms, Mistral et
calibration collective devra être rapproché de l'architecture exécutée avant
publication. Le portail n'est pas modifié par ce lot 0.

## Portée de la livraison

Vérifications locales :33 tests existants du noyau physique,8 tests du banc,
Clippy de la sonde avec warnings refusés et rustfmt passent. Le rapport initial
et sa seconde campagne de contrôle passent ; aucune suite du frontend n'est
relancée puisque ni le jeu ni ses sources ne sont modifiés.

Livré : cartographie actualisée, références consultées et lacunes de lecture
explicites, plan public, sonde native, runner local, rapport initial et recette
de répétition. Pas de nouveau coefficient, module hybride, règle de pneus,
optimisation, dépendance physique ou livraison hébergée. Le résultat utile du
lot 0 est une base explicite pour décider ce qui doit changer et mesurer pourquoi.
