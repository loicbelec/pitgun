# Sensibilité sur Grands Prix complets — campagne locale

Suite de la [résolution de carrière](../career_physics_v1/README.md), selon les
[critères clarifiés par le propriétaire](../career_physics_v1/ACCEPTANCE_CRITERIA.md).
Ce lot mesure des réponses ; il ne modifie aucune équation, aucun coefficient,
catalogue, adversaire, Worker, admission Authority/Verifier ou service déployé.

[plan-v1.json](plan-v1.json) et les scripts sont committés avant la campagne.
Trois étapes de carrière (entrée/sortie I, sortie II), deux circuits de sélection
(Monza/Zandvoort), composants, budget et bonus HQ réellement préparés. Les trois
doctrines sont des alias physiques dans les fixtures : une seule route est
calculée. Source seed propre à chaque circuit conservé ; ce n'est pas une graine
unique sur tous les circuits. Chaque calcul porte sur le GP complet officiel.

1. Allocations à curseurs fixes : toutes les répartitions entières moteur/châssis,
   allocations HQ de référence et transferts locaux déclarés.
2. Curseurs : toutes ces répartitions moteur/châssis croisées avec la grille
   légale, puis trois allocations non concentrées retenues sur leur GP. Une
   correction locale bornée explore ensuite les transferts d'un point.
3. Diagnostic causal : +1 point sur un seul axe, budget autorisé augmenté d'un
   point pour cet essai. Ce diagnostic conserve les autres allocations ; il est
   **hors budget de carrière et exclu de toute sélection compétitive**.
4. Stratégies : pneus et arrêts changent seuls sur les configurations retenues.
   Tous les relais somment à la distance officielle, avec pertes au stand et
   préparation héritées. Catalogue de gommes soft/medium/hard fourni par
   l'exporteur de fixtures ; ceci ne qualifie pas les permissions du futur jeu.
5. Configurations et règles de stratégie scellées avant ouverture des conditions
   réservées. Deux autres graines, froid/chaud (températures constantes, sans
   simulation de pluie), puis transfert vers Silverstone/Montreal. Les données
   de référence de ces circuits sont déjà exposées ; ces réponses aux réglages
   sont réservées, pas un modèle physique entièrement inconnu.

Le meilleur échantillon concentré est exact dans la grille déclarée de splits
entiers/curseurs. La recherche non concentrée et les curseurs continus ne sont
pas exhaustifs. Une victoire concentrée peut être légitime, particulièrement
avec du cooling HQ déjà acquis. Les sensibilités proches de l'incertitude de
couplage sont rapportées comme ambiguës ; répétabilité n'est pas précision.

## Exécution

```sh
cargo build --release --offline --locked -p pitgun-racing-simulator \
  --example career_full_gp_probe
python3 experiments/career_full_gp_v1/self_check.py \
  target/release/examples/career_full_gp_probe
python3 experiments/career_full_gp_v1/run_local.py --preview
python3 experiments/career_full_gp_v1/run_local.py \
  target/release/examples/career_full_gp_probe /private/tmp/pitgun-full-gp-cache
```

Adapter le binaire si `CARGO_TARGET_DIR` est externalisé. Quatre processus natifs
au maximum. Cache privé limité à cette campagne et verrouillé par digests de
sources/binaire ; mêmes requêtes physiques déclarées réutilisées entre phases.
Les confirmations et répétitions réservées contournent ce cache et exigent une
égalité exacte. Les erreurs d'infrastructure/résolution arrêtent le banc ; les
échecs de simulation restent des résultats avec leur préfixe accepté.

Les gros reçus sont conservés en gzip sans perte, pas de télémétrie complète par
point ni sauvegarde personnelle. Les paramètres résolus, entrées/politiques/
sources/binaire, résultats, bilans, extrema et états par tour sont enregistrés.
`selection-lock-v1.json` est écrit avant tout résultat réservé et revérifié en
fin de campagne. Aucun coût natif concurrent n'est présenté comme un gain
navigateur/WASM/VPS. Le carburant100kg et le montage90°C restent des contrôles
hérités, pas une nouvelle politique physique/réglementaire par ère.

Une cohérence physique et de bons compromis ne constituent pas encore un verdict
sur la difficulté : aucun champ de neuf adversaires ou saison n'est simulé ici.
