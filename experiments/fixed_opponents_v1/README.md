# Plateau fixe de neuf IA — campagne native I/II

Demande propriétaire : neuf personnalités constantes, Franz Hermann et Charles
L'Éclair les plus compétitifs. Les capacités ne sont plus des rôles permutés selon
la course. Identités et traits, style et politique sont enregistrés séparément.
Les neuf utilisent d'abord le même mode Balanced que le joueur, sans avantage
Attack caché. Régularité/gestion des pneus/limite et erreurs physiques diffèrent.

Le [protocole](plan-v1.json) est figé avant calcul, puis la politique finale avant
validation réservée. Le [roster de base](roster-v1.json) donne les trois traits
persistants. Trois niveaux de calibration globale des deux favoris sont testés
sur l'entraînement uniquement ; un seul est ensuite conservé partout. Ce sont
des ressources physiques explicites, jamais un bonus de chrono ni une obligation
de finir devant à chaque course.

Les budgets sont choisis parmi des constantes par ère pour les neuf, avec offsets
HQ et composants communs prédéfinis, puis verrouillés. Aucune dépendance aux points
actuellement affectés par le joueur, son résultat, son nom ou son futur plan.
Le joueur conserve les configurations et composants réellement acquis dans les
18préparations de carrière. L'entrée II avec son ancien V8 est explicitement un
déficit technologique, distinct d'un essai à matériel identique.

## Méthode

Recherche sur10contextes publics I/II, GP complets : allocations/curseurs,
stratégies légales du lecteur actuel (1à3arrêts, deux premiers composés distincts),
adaptation et amélioration bornées par pilote, styles proches du meilleur temps
échantillonné (écart réel maximum0,15%), budgets fixes et progression du joueur.
Les favoris retiennent leur meilleur choix échantillonné ; les autres conservent
leur préférence de préparation dans cette même bande, sans multiplicateur de
temps. La recherche reste partielle en continu et pour les stratégies au-delà
de trois arrêts, permises par l'éditeur mais non explorées ici.

Les ressources moteur/thermique/énergie et les tolérances restent inchangées.
Le nouveau probe résout d'abord la mécanique/thermique avec la ressource commune
connue, puis applique explicitement la ressource V2 nommée avant tout calcul.
Le reçu porte ces deux étapes et l'identité effective. Pas de fallback vers le
pilote legacy pour un nom inconnu. Aucun ancien constructeur/source/probe ou
catalogue gelé n'est modifié.

Huit processus natifs maximum sur12CPU logiques. Sortie compacte : temps par tour,
états finaux, extrema/bilans agrégés, travail numérique, nombre d'hésitations
récupérables, digests d'entrée/résultat/source. Pas d'historique point à point ni
de simulation graphique. Tous les échecs sont conservés ; une erreur de résolution
arrête le banc. Les profils fixes sont évalués dans des processus neufs pour les
répétitions ; le cache est privé et épinglé aux sources/binaire.

Les temps individuels composent un classement indépendant, comme ce noyau sans
trafic partagé. Des lots natifs de dix voitures vérifient l'assemblage, l'ordre
du roster et l'indépendance aux réglages du joueur. Aucune collision, aspiration,
pluie dynamique, VSC ou probabilité d'accident réel n'est revendiquée.

## Exécution locale

```sh
cargo build --release --offline --locked -p pitgun-racing-simulator --example fixed_field_probe
python3 experiments/fixed_opponents_v1/qualify.py target/release/examples/fixed_field_probe
python3 experiments/fixed_opponents_v1/run_local.py --preview
python3 experiments/fixed_opponents_v1/run_local.py \
  target/release/examples/fixed_field_probe /private/tmp/pitgun-fixed-field-cache
```

Adapter le binaire au `CARGO_TARGET_DIR` externalisé. Le runner refuse d'écraser
une campagne terminée. Modèle natif candidat uniquement : aucun Worker/WASM,
Authority/Verifier, jeu, sauvegarde personnelle, staging ou production modifié.
Une qualification de plateau I/II n'est pas une qualification des ères III+,
de la physique expérimentale réelle ou de l'admission signée du futur jeu.
