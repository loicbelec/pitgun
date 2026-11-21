# Coût du noyau et cadence d’observation — V1

Mesures locales macOS arm64 / Rust 1.89.0, sources figées dans `346f36b`.
[Rapport et mesures brutes](results/local-cost-v1.json) ·
[SHA-256](results/local-cost-v1.sha256).

## Ce qui est conservé

Les douze sorties physiques complètes du candidat énergie V1 sont identiques aux
archives, y compris les digests des trajectoires et les raisons de panne. Les
onze sensibilités précédentes et les dix sorties canoniques historiques restent
également exactes. Aucun coefficient, nœud spatial, phénomène, nombre de passes de
couplage, tolérance de résolution ou règle de décision n’a été retiré/modifié.
Les dates/états observés plus rarement ne sont pas utilisés pour intégrer la physique.
Une seconde campagne complète `--check` reproduit ces preuves sans réécriture.

## Gain de calcul sans réduction de la physique

Les travaux retirés : évaluations répétées aux extrémités des intervalles de
recherche de racine ; recalcul des charges finales d’un segment ; interpolations
de couple aux nœuds de carte invariants et au régime initial ; calculs historiques
de contact immédiatement remplacés dans le mode énergie ; copies intermédiaires
de vitesses qui n’étaient pas utilisées. Le coefficient d’adhérence tenu constant
dans l’anticipation du prochain tour est évalué une seule fois par passe.

La recherche de racine conserve son algorithme, ses 48 essais maximum et ses
tolérances. Le moteur conserve le derating et l’embrayage ; les pneus conservent
leurs lois et leur boucle de couplage. Le cache de couple n’est valable que pour
le moteur résolu inchangé et ne met pas en cache les facteurs variables de derating.

Médianes de deux processus frais par version, ordre avant/après/après/avant.
Le cœur est la somme des calculs de toutes les voitures du cas, pas du seul joueur.

| Scénario | Cœur avant, ms | Cœur après, ms | Réduction | Processus avant → après, ms |
| --- | ---: | ---: | ---: | ---: |
| `monza-classic-5` | 250.8 | 186.9 | 25.5 % | 551.4 → 494.1 |
| `monza-classic-5-1hz` | 252.2 | 180.4 | 28.4 % | 551.2 → 479.1 |
| `silverstone-classic-5` | 245.5 | 175.7 | 28.4 % | 544.2 → 477.0 |
| `monaco-classic-5` | 127.8 | 93.5 | 26.8 % | 393.6 → 360.8 |
| `suzuka-classic-5` | 235.5 | 169.8 | 27.9 % | 538.3 → 465.3 |
| `monza-classic-53` | 2785.7 | 2063.5 | 25.9 % | 3855.2 → 3125.0 |
| `monza-modern-5` | 236.1 | 172.6 | 26.9 % | 536.0 → 477.7 |
| `monza-hybrid-resource-5` | 264.2 | 182.5 | 30.9 % | 563.7 → 481.5 |
| `monza-classic-grid10-5` | 2555.2 | 1826.5 | 28.5 % | 3619.1 → 2880.2 |
| `monaco-classic-78` | 2091.2 | 1569.6 | 24.9 % | 3004.9 → 2497.8 |
| `spa-classic-5-seed17` | 306.9 | 215.7 | 29.7 % | 626.9 → 536.0 |

Sur ces ancres, le coût du cœur baisse d’environ 25 à 31 %. La physique plus riche
reste plus chère que l’ancien modèle. La baisse du RSS n’est pas démontrée :
Monza 53 reste autour de 401 MiB dans ce banc à historique natif complet. Supprimer
quelques copies temporaires ne retire pas les historiques et matérialisations
qui fixent le pic. Les variations de quelques MiB ne sont pas un gain robuste.

Un échantillonnage OS d’une seconde du processus natif avant optimisation a permis
de repérer le chemin de couple/interpolation dans `SegmentContext::advance`. Sa
courte fenêtre de démarrage et son coût d’instrumentation interdisent d’en tirer
un pourcentage fiable pour la course entière. Les timings ci-dessus sont mesurés
sans le profiler. Deux répétitions indiquent un ordre de grandeur, pas un intervalle
de confiance ni une promesse sur toutes les machines.

## Ce que fait une baisse de fréquence

L’intégration reste sur la même grille. Seules les observations causales produites
après calcul passent à 1, 2 ou 5 Hz. Le terminal est inclus. Chaque fréquence conserve
le digest natif complet, les états finaux et le bilan d’énergie de référence.
Ce JSON d’audit de neuf champs physiques n’est **pas** le format réseau actuel du
jeu ; les octets/temps ne constituent pas une mesure de bande passante réelle.

Monza 53, une voiture :

| Observation | Lignes | JSON d’audit, octets | Construction + JSON + SHA, ms | Cœur natif, ms |
| --- | ---: | ---: | ---: | ---: |
| 5 Hz | 32500 | 7897234 | 20.90 | 2065.9 |
| 2 Hz | 13001 | 3158966 | 8.52 | 2054.5 |
| 1 Hz | 6501 | 1579677 | 4.47 | 2067.8 |

Passer de 5 Hz à 2 Hz retire environ60 % des lignes/octets de cet export, et 1 Hz environ
80 %. Le calcul physique reste autour de 2,06 s ; l’essentiel du gain porte sur le
volume produit. Le temps d’export est ici de l’ordre de 20,9 → 4,5 ms, très inférieur au
coût du solveur. La fréquence seule ne compense donc pas le surcoût de la physique.

Les commandes sont fixées dans ce banc. Réduire la fréquence des observations
fournies à un agent pourrait modifier ses décisions et donc la course. Les courbes
graphiques, les agrégats natifs par tour, les événements exacts et le contrat
d’observation du contrôleur doivent rester distincts et être versionnés ensemble
lorsqu’un changement les affecte. Aucun LLM n’a été appelé pour ces mesures.

La phase `native_digest`, autour de 0,83 s sur ce cas, hache ici le résultat natif
**complet** pour prouver l’égalité bit à bit. Ce travail est propre au protocole de
ce banc ; ce n’est pas une mesure du replay compact de production ni la preuve
qu’il faudrait réintroduire/recalculer un historique complet dans Verifier.
De même, le chargement/résolution à froid du catalogue dans chaque processus frais
n’est pas un coût à additionner à chaque tour d’un Worker déjà initialisé.

## Décision et suites

- Retenir ces suppressions de travail répété : elles conservent les résultats
  physiques sans précision sacrifiée. Leur admission au jeu reste à qualifier.
- Tester ensuite un profil d’observation versionné : éventuellement 2 Hz pour une
  partie de la télémétrie, 1 Hz pour des états thermiques lents, et une cadence de
  position/interpolation choisie par recette visuelle. Ces valeurs sont des
  hypothèses à tester, pas des paramètres changés dans le jeu actuel, encore à 5 Hz.
- Garder les événements et décisions à leurs dates exactes, indépendamment de la
  cadence des courbes. Le banc n’altère aucun journal de contrôle.
- Mesurer les vraies matérialisations et l’historique conservé dans Worker/WASM,
  avec données compactes et détails à la demande lorsque ces optimisations ne sont
  pas déjà présentes. Ne pas extrapoler ce RSS natif au navigateur.
- Étudier ultérieurement un maillage d’intégration piloté par l’erreur, tout en
  conservant les équations, uniquement avec convergence, alignement des événements
  et commandes de conduite indépendantes du nombre de nœuds. Cela change la
  trajectoire numérique et nécessite une nouvelle qualification.

Barcelone 66 échoue toujours carburant au même tour et avec le même message dans
les deux binaires énergie : le problème carburant n’est pas masqué par une
optimisation. Aucun nouveau modèle physique n’est validé expérimentalement par
ce travail et aucun service staging/prod, runtime admis ou sauvegarde n’est modifié.

## Validation finale

[Reçu natif](results/native-receipt-v1.json) :224 tests natifs et16 contrôles Python
passent, dont trois nouveaux tests Rust (cache de couple et observations causales
bornées) et deux contrôles d’archive. Clippy strict passe sur Solver/Simulator et
le nouveau probe ; rustfmt passe sur le workspace. Simulator et Solver dépendant
compilent pour WASM. Ce n’est pas une recette du nouveau modèle dans le navigateur
ni son admission signée. La seconde campagne complète `--check` passe sans modifier
l’archive. Le suivi reste [447](https://github.com/loicbelec/pitgun/issues/447),
ouvert pour les mesures Worker/WASM et le budget avant admission dans le jeu.
