# Thermique V2 — expérimentation native locale

Le candidat remplace les lois thermiques moteur et pneus, et ajoute une résistance
réduite de déformation explicitement payée dans le bilan mécanique. Les équations,
unités, paramètres, sources et limites sont dans
[THERMAL_PHYSICS_V2.md](../../docs/physics/THERMAL_PHYSICS_V2.md).

**Aucun modèle n'est activé dans le jeu.** Le profil `profile-v1.json` porte le
statut exploratoire, sans identification expérimentale ni admission. L'hybridation
reste future ; ni batterie ni récupération ne sont simulées par ce candidat.

## Procédure

```sh
cargo build --release --locked --offline -p pitgun-racing-simulator \
  --example thermal_physics_v2_probe --example thermal_tire_gameplay_probe
python 3 experiments/thermal_physics_v2/make_plan.py --check
python 3 experiments/thermal_physics_v2/run_local.py \
  --probe target/release/examples/thermal_physics_v2_probe \
  --legacy-probe target/release/examples/thermal_tire_gameplay_probe
```

Adapter les chemins si `CARGO_TARGET_DIR` est externalisé. Les sources/profil/plan
doivent être committés avant le banc. Le second passage ajoute `--check` : il relit
le résultat, répète toute la matrice, exclut uniquement les mesures de processus
de la comparaison et laisse l'archive intacte. Chaque passage compare d'abord les
22 anciennes sorties/erreurs gelées du noyau historique et du candidat énergie.

Le plan comporte 56 cas×2 modèles : noyau énergie V1 conservé et nouveau thermiqueV2.
Les 112 sorties par passage enregistrent succès **et** échecs, requêtes et profils
réexécutables, catalogue, sources compilées, binaire, bilans, statistiques par tour
et état surface/carcasse avant/après service. Aucun historique de télémétrie
complet n'est archivé dans ce rapport.

Le temps/RSS mesure le **processus de banc entier**, incluant résolution du
catalogue, calcul, digest et statistiques. Il ne constitue pas une mesure isolée
du noyau, du Worker, de WASM ou d'un VPS. Le candidat change les trajectoires et
l'anticipation conservative de freinage au passage de ligne ; les écarts V1/V2
ne sont pas des effets thermiques isolés. Les ablations sans derating et les
comparaisons à moteur identique sont explicitement distinctes des budgets égaux.

## Calibration, portée et suite

Un petit pilote privé a servi à sélectionner ces paramètres exploratoires. Il a
révélé des pneus trop chauds, un derating précoce trop faible, ainsi qu'une
anticipation de freinage de tour insuffisante. Les fenêtres de gomme, échanges,
dimensionnements de radiateur et couplage ont été repris avant de figer le plan.
Ces cas sont **exposés** ; leur répétabilité ne les transforme pas en validation
aveugle. Le plan et son profil forment un écran de calibration reproductible.

Le banc conserve 100 kg de carburant : un plein moderne insuffisant reste un échec,
sans contournement par un réservoir augmenté. Les budgets 8/15/20 sont des budgets
effectifs synthétiques ; leur présence ne prouve pas une sauvegarde HQ gagnable.
Un meilleur candidat parmi quelques transferts ne prouve pas un optimum global.

Avant admission : validation réservée circuits/conditions/conduites et distances,
résolution carburant/hybridation pertinente, précision spatiale et coût de
couplage, stratégies légales avec pertes au stand, équilibre des autres points et
sliders, puis export/observations, checkpoint, Worker/WASM et Authority/Verifier.
Les tickets #420/#421 restent ouverts. Le projet électrique et les contrats
d'agents génériques ne reçoivent aucune règle de course.

Résultats et limites mesurées : [RESULTS_V1.md](RESULTS_V1.md).

Pour les véritables configurations de carrière avec moteur remplacé, utiliser
désormais le [résolveur natif des composants installés](../career_physics_v1/README.md).
Le présent banc conserve ses entrées synthétiques et résultats gelés ; son choix
de moteur par véhicule de base ne suffit pas à représenter ces remplacements.
