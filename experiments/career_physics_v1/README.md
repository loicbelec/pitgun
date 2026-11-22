# Thermique sur les configurations de carrière — 22 novembre 2026

Ce lot relie le candidat physique natif aux **composants effectivement acquis**.
Il prépare la calibration des adversaires ; il ne modifie ni leurs réglages, ni
les équations et coefficients qualifiés précédemment. Ticket [#466](https://github.com/loicbelec/pitgun/issues/466),
suite de [#461](https://github.com/loicbelec/pitgun/issues/461), préalable à
[#457](https://github.com/loicbelec/pitgun/issues/457).

## Pourquoi cette résolution est nécessaire

Dans une vraie carrière, `classic_v8_1960` reste le véhicule de base alors que le
joueur ou les adversaires peuvent avoir monté `v8_1970`. L'ancien banc thermique
lisait le moteur du véhicule de base : utile sur ses cas synthétiques sans
remplacement, insuffisant pour ces configurations de carrière. Cela ne change
aucun résultat archivé ; ceux-ci conservent leur portée déclarée.

`thermal_career::resolve_thermal_career_v1` réutilise la normalisation et le
résolveur de carrière existants, puis applique le candidat à chaque voiture :

1. Composants réellement installés, moteur et paramètres catalogue immuables.
2. Dimensionnement du radiateur selon ce moteur et les points effectifs de
   refroidissement, bonus HQ inclus. Retrait de l'incrément de traînée de
   l'ancien profil sélectionné, ajout de celui du candidat une seule fois.
3. Gommes réellement présentes dans les relais et remplacements au stand.
4. Pilote V2 explicitement demandé, capacité/coût de correction et hésitations
   déterministes, mode `balanced` par défaut et identité de bruit stable par
   concurrent. Un pilote inconnu ne devient pas silencieusement `default`.
5. Validation physique complète avant de retourner le champ de voitures.

La résolution n'exécute pas la course. Ses reçus portent les digests des requêtes,
politiques, paramètres moteur analysés, entrées avant/après et composants
installés. La provenance globale est séparée des voitures : modifier le joueur
laisse chaque objet adversaire exactement identique. Le digest des paramètres
moteur n'est pas présenté comme un hash des octets bruts de sa ressource.

Les coefficients numériques du nouveau `profile-v1.json` sont repris sans
retouche de `thermal_physics_v2/profile-v1.json`. Le schéma de résolution est
explicitement nouveau ; les catalogues, anciens constructeurs et profils restent
intacts. Le choix d'anticipation entre tours est lié au digest de l'enveloppe
`BoundaryDiagnosticRequestV1`, distinct du digest de l'entrée physique seule.

## Campagne locale

[plan-v1.json](plan-v1.json) est figé avant calcul. Il distingue résolution,
références GP, écran court et confirmations. Les 12 fixtures proviennent des
vrais réducteurs du jeu, sans sauvegarde personnelle. Les trois routes de
doctrine sont des alias physiques dans cet échantillon : 54 requêtes/540 voitures,
mais 18 courses distinctes, pas 54 observations indépendantes.

Précision de provenance : le seed est fixé par requête de circuit (17041 plus
son index dans le calendrier exporté), puis conservé entre variantes appariées.
La mention abrégée «seed17041» du plan gelé ne doit pas être interprétée comme une
graine identique sur tous les circuits ; les requêtes/reçus conservent les valeurs
effectivement utilisées et leurs digests.

```sh
cargo build --release --offline --locked -p pitgun-racing-simulator \
  --example career_physics_probe
python3 experiments/career_physics_v1/run_local.py \
  target/release/examples/career_physics_probe
```

Adapter le chemin si `CARGO_TARGET_DIR` est externalisé. Le lanceur refuse
d'écraser une campagne terminée. Il conserve tous les échecs de simulation et
arrête la campagne sur une erreur de résolution/exécution du banc. Aucun échec
numérique n'est requalifié automatiquement en panne de carburant.

Les gros reçus sont ensuite conservés en `.json.gz`, sans perte, avec
`pack_results.py`. Les digests du manifeste de campagne portent toujours sur
leurs octets JSON décompressés exacts ; `packaging-v1.json` ajoute les digests des
archives. `python3 experiments/career_physics_v1/summarize.py` vérifie ces sources
et reçus avant de régénérer la synthèse. Il ne relance pas de simulation.

Les allocations portent sur le budget **discrétionnaire**, puis les bonus HQ
fixes sont ajoutés. Les curseurs respectent les déblocages et incluent les bornes
0 et 1. Les meilleurs échantillons courts moteur/châssis et diversifiés sont
confirmés sur le GP complet, sans changer leurs relais. Les adversaires ne sont
pas exécutés ni recalibrés dans cet écran de sensibilité du joueur.

Le carburant initial hérité reste 100 kg pour isoler les réglages. La politique
d'auto-préparation du carburant et sa sensibilité restent un travail distinct.
Le montage/stand conserve ici la convention héritée de 90 °C des ères I/II : ce
n'est ni une affirmation réglementaire historique, ni une généralisation de la
politique moderne à 70 °C. Aucune température en course n'est maintenue
artificiellement dans sa fenêtre optimale.

## Interprétation et suite

La [clarification des critères](ACCEPTANCE_CRITERIA.md) fixe la cible : une
importance physique réelle mais contextuelle de chaque axe disponible. Des
victoires moteur/châssis avec un cooling déjà apporté par le HQ peuvent être
cohérentes ; elles ne constituent pas à elles seules un défaut à corriger.

Cet écran compare un nombre limité d'allocations et de curseurs, avec une seule
graine et des conditions fixes. Le meilleur temps sur cinq tours n'est pas un
optimum GP. Un résultat favorable ne suffit pas à exclure un raccourci
moteur/châssis ; un résultat défavorable est conservé, sans forcer un coût caché
au refroidissement ou une dépense sur tous les axes.

Restent à qualifier : recherche entière/fine, stratégies et carburant, conditions
réservées, puis champ fixe de neuf adversaires et saisons complètes. La pertinence
des événements Engineer doit suivre une compétition réelle, sans réglage des IA
destiné uniquement à provoquer des bulles. L'admission native/WASM, Worker,
Authority/Verifier et tout déploiement sont ultérieurs. Aucun service, navigateur
personnel ou sauvegarde de joueur n'est utilisé ici.

Résultats, limites et prochaine décision : [RESULTS_V1.md](RESULTS_V1.md).
