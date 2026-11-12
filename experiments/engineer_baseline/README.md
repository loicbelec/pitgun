# Engineer baseline — premier écran local

Première tranche de l’étape 1 Bring Your Own Engineer : mesurer des stratégies
témoins avant de figer le challenge et d’intégrer Gemini. Aucune connexion à un
LLM, aucun changement physique, aucune publication de catalogue ou de leaderboard.

## Scénario candidat

- Catalogue `pitgun.racing@1.9.0`, modèle `pitgun.racing-v3-candidate@0.15.0`.
- Suzuka, 12 tours, seed `42`, dix concurrents.
- Voiture `classic_v8_1960`, pilote joueur `balanced_reference`, budget 27 et
  réglages du scénario public `suzuka-mid-42-balanced` déjà présent dans le dépôt.
- Tous les autres inputs sont conservés depuis ce scénario, sauf la distance et
  les plans de relais, explicitement ramenés à 12 tours. Les adversaires gardent
  leurs voitures, pilotes et réglages, un plan medium 6 / hard 6 et le mode balanced.
- Carburant fixé par le catalogue ; aucune réduction artificielle pour la course
  courte. Cette charge prévue par le modèle est une limite du scénario candidat.
- Sept plans joueur : soft, medium ou hard sans arrêt ; medium/hard avec arrêt
  après 4, 6 ou 8 tours ; soft/medium/hard avec deux arrêts après 4 et 8 tours.
- Quatre timelines : balanced sans transition ; attack dès l’index de tour 1 ;
  manage dès l’index 1 ; manage dès l’index 1 puis attack à l’index 6. Le tour
  initial garde toujours balanced, conformément au profil d’instructions.

Soit 28 cas appariés, exécutés chacun deux fois dans des processus natifs séparés.
Ce n’est ni un tournoi multi-seeds, ni une optimisation exhaustive, ni un challenge
officiel. Les résultats peuvent seulement départager les stratégies testées.

## Reproduire

Depuis la racine du dépôt framework :

```bash
cargo build --offline --locked --release \
  -p pitgun-racing-simulator --example engineer_baseline_probe
python3 experiments/engineer_baseline/screen.py --check
```

`--offline` suppose les dépendances Cargo déjà en cache. Pour la première génération,
omettre `--check`. Le script refuse de remplacer des preuves existantes différentes.
Une modification volontaire de l’expérience doit créer une nouvelle version.

Le probe Rust appelle directement le workload gouverné et sa projection d’evidence.
Python prépare les inputs et compare les sorties ; il ne calcule aucune physique.
La répétition compare les bytes canoniques complets du probe, y compris digests de
résultat et de synthèse télémétrique. `--check` compare aussi le manifeste reconstruit
et le rapport aux fichiers archivés.

- `manifest.json` conserve les 28 inputs complets, timelines, identités physiques
  et hash du scénario source.
- `results.json` conserve résultat, statut, diagnostics, digests et classement des
  arrivées ; les hashes logiques sont calculés en Rust avec JCS.
- `runtime.json` conserve le commit framework, le digest du binaire, la version et
  cible Rust et le hash du rapport de la génération initiale. Le source du probe
  expérimental ajouté au checkout est identifié séparément dans le rapport.

Les digests de fichiers du script identifient leurs bytes de stockage, pas une
implémentation Python de JCS. Ces résultats ne sont pas des Run Bundles complets,
ne portent pas de verdict Authority/Verifier et ne prouvent pas la parité WASM.
La commande actuelle `pitgun replay` ne lit pas ce format expérimental.

## Résultat initial : affiner le challenge court

Les 28 configurations terminent et reproduisent leurs bytes dans des processus
séparés. Le témoin déclaré est `balanced-mh--balanced` : medium 6 / hard 6,
mode balanced, temps total **1 424 639 ms**.

| Stratégie | Temps total | Écart au témoin |
| --- | ---: | ---: |
| Hard sans arrêt, attack après le premier tour | 1 403 869 ms | −20 770 ms |
| Medium sans arrêt, attack après le premier tour | 1 407 139 ms | −17 500 ms |
| Medium 6 / hard 6, attack après le premier tour | 1 411 523 ms | −13 116 ms |
| Medium 4 / hard 8, attack après le premier tour | 1 412 200 ms | −12 439 ms |
| Medium 8 / hard 4, attack après le premier tour | 1 412 992 ms | −11 647 ms |

À plan de pneus égal, la timeline attack gagne dans les sept groupes face aux
trois autres timelines testées. L’arrêt après 6 tours bat les arrêts après 4 ou 8
tours à mode attack, mais reste 7 654 ms derrière hard sans arrêt.

Les décisions ont donc un effet mesurable, mais ce premier scénario ne démontre
pas encore un intérêt suffisant de la délégation stratégique ou de l’adaptation
live. Il ne faut pas imposer un pit artificiel ou modifier les coefficients pour
faire gagner un LLM. Le résultat concerne une distance, un circuit, un pilote et
un seed ; il ne démontre pas une domination universelle.

## Prochain livrable de l’étape 1

Comparer une distance plus longue avec les mêmes ressources physiques, puis
contrôler les observations sur plusieurs seeds appariés. Retenir un challenge
dont le résultat dépend de compromis observables ; sinon documenter la limite
du modèle avant de poursuivre l’ambition benchmark. Figer ensuite le briefing,
les droits délégués et le JSON Schema du plan avant-course, avec cas valides et
invalides. L’intégration Gemini est l’étape 2.

La preuve Grid reste un jalon d’architecture avant stabilisation du contrat
générique live ; elle n’est pas une dépendance de cet écran ou du premier MVP.
Voir la [proposition d’ensemble](../../docs/BRING_YOUR_OWN_ENGINEER_PROPOSAL.md).
