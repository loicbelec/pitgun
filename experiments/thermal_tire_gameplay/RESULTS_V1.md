# Audit cooling / pneus — 21 novembre 2026

**Les mécanismes réagissent ; le gameplay physique reste à affiner.** Le derating
agit réellement sur les performances. Le cooling ne peut pas être déclaré
obligatoire partout : les besoins varient, et les V8 testés restent loin du seuil
thermique. Les pneus ont une loi couplée, mais leur équilibre thermique les place
longtemps au plancher du facteur d’adhérence lié à la température. La refonte
énergétique n’a pas remplacé cette loi de pneus.

[Courbes](figures/gameplay-audit-v1.png) · [Protocole](README.md) ·
[Plan primaire](plan-v1.json) · [Résultats primaires](results/local-gameplay-v1.json) ·
[Grille](results/budget-grid-v1.json) · [Raffinement](results/grid-refinement-v1.json)

## Périmètre et solidité des conclusions

Le cœur historique carrière V9 / composant 0.11 / catalogue 1.8.1 et le candidat énergie
natif optimisé sont comparés sur 82 scénarios figés (164 exécutions), puis rejoués
entièrement à l’identique. Le probe d’observation reproduit préalablement 22 sorties
historiques/énergie figées. Sources 6b7311b ; suivi séparé 01cbb86 puis a744fff.
Aucune équation, aucun coefficient ou catalogue n’a changé.

187 allocations légales, à pas 4/3/10 selon le budget, comparent les quatre axes
simultanément : 374 exécutions. Les 16 minima sélectionnés sont rejoués exactement.
Un raffinement indépendant, défini avant ses résultats, transfère 1 à 6 points
d’un seul axe du meilleur zéro-cooling vers le cooling : 36 allocations / 72
exécutions ; les huit références zéro et les huit minima positifs sont aussi
rejoués exactement. Ce n’est pas un optimum continu global : sliders et pilote
restent fixes et la grille ne couvre pas toutes les situations/trajectoires.
Le raffinement corrige notamment une conclusion trop rapide d’une grille à pas 10.

Les budgets et bonus sont synthétiques, résolus par le vrai code Rust. Ils ne
reconstituent pas une carrière gagnée, toutes ses disponibilités ou un adversaire
compétitif. Ce n’est pas un replay des octets effectivement déployés en production
ni une validation expérimentale du véhicule. Les runs négatifs restent archivés.
Aucun LLM, service hébergé ou Databricks n’est appelé ; aucun nouveau WASM n’est livré.

## Cooling : effet réel, obligation universelle non démontrée

Zéro point n’est pas l’absence de radiateur. La résolution actuelle vaut
`Hmult = 0.75 + 0.50 × cooling_points / 20` : zéro conserve 75% de la capacité
physique de référence. Un bonus HQ peut aussi rendre l’allocation effective
positive quand le joueur n’alloue rien. La perturbation antérieure `H × 0.5`
n’équivalait donc pas au passage de 10 points à zéro. Les familles modernes
ajoutent un coût aérodynamique du refroidissement ; les V8 historiques n’en ont pas
dans le profil actuel. Le coût en points est également une opportunité perdue.

Exemple de ligne à budget 20, V6T/Monza/candidat énergie, cinq tours, points pris
sur le moteur (aéro 4/châssis 4 conservés) :

| Cooling | Moteur | Pic moteur, °C | Temps natif sous derating, s | Durée, s |
| ---: | ---: | ---: | ---: | ---: |
| 0 | 12 | 114.66 | 351.11 | 532.654 |
| 2 | 10 | 111.51 | 171.53 | 530.949 |
| 4 | 8 | 108.13 | 22.08 | 530.801 |
| 6 | 6 | 104.16 | 0.00 | 531.388 |

Le seuil de cette famille est 107 °C. Une ablation diagnostique du derating, à
réglages strictement identiques et zéro cooling, fait gagner 2,878 s sur cette ligne.
Le mécanisme est causal, au-delà d’un simple affichage de température.

La recherche plus large confirme des besoins différents. Tableau du candidat
énergie : meilleur zéro-cooling de la grille contre meilleur positif observé
(grille + raffinement local). Delta = temps positif − temps zéro ; négatif signifie
que le cooling positif est plus rapide.

| Scénario, cinq tours | Cooling positif retenu | Delta, s |
| --- | ---: | ---: |
| V6T / Monza / budget 20 | 2 | -1.335 |
| V6T / Barcelone / budget 20 | 1 | +0.074 |
| V8 1970 / Montréal / budget 15 | 1 | +0.275 |
| 2026 / Monza / budget 40 | 2 | -1.337 |

Le pas grossier suggérait à tort qu’aucun cooling positif ne battait zéro sur
2026/Monza ; tester 2 points révèle un gain de 1,337 s. Inversement, le V6T à Barcelone
reste presque indifférent entre zéro et 1 point sur les meilleurs réglages testés.
Ce n’est pas une preuve qu’un joueur ne pourra jamais gagner avec zéro cooling.
La compétitivité des adversaires et l’ensemble des réglages jouent aussi.

V8 1970/Montréal, budget 15 : zéro cooling atteint seulement 92,41 °C, contre 110 °C
pour le seuil. Le derating ne s’active pas. Sur Monza 53 avec le même V8/budget,
zéro cooling termine en 6537,997 s contre 6544,756 s pour 2 points pris sur le moteur.
Désactiver le derating sur les V8 testés ne change pas le chrono. Il faut donc
revoir le domaine de pertinence du cooling/V8 si le design veut un arbitrage plus
tôt ; ajouter une pénalité artificielle au zéro n’est pas une validation physique.

Plusieurs meilleurs réglages tardifs de cette grille allouent zéro au moteur et
beaucoup au châssis. Un moteur installé fournit déjà de la puissance sans points
de développement. Ce résultat n’établit pas que le moteur soit inutile partout,
mais impose un audit de l’équilibre global points/composants/rapports avant
l’affirmation que chaque axe est compétitif à toutes les étapes.

## Pneus : couplage présent, bilan thermique et choix à reprendre

La loi utilise les efforts, leur taux d’utilisation et la vitesse pour une charge
de contact empirique. Une fraction produit la chaleur ; l’air refroidit le pneu.
La température et l’usure modulent ensuite l’adhérence ; la température modifie
aussi l’usure. La gomme porte ses coefficients de grip/usure/fenêtre thermique.
C’est déjà plus riche qu’un compteur d’usure lié seulement au nombre de tours.

Il reste **un seul état thermique agrégé pour les quatre pneus**. La source de
chaleur est un proxy de sollicitation, pas une dissipation par glissement mesurée.
Capacité thermique et refroidissement du contact sont ici communs aux gommes ;
les anciens champs `heat_k/cool_k` des ressources ne sont pas les coefficients
utilisés par cette branche Aggregate V2. Il n’y a pas encore de surface/carcasse
séparées, de couplage à la température de piste ou de transfert thermique des freins.

Barcelone 20 tours, même V6T/budget 20, pas d’arrêt, candidat énergie : ce sont des
**relais** comparables, pas la preuve qu’une course complète sans changement de
gomme respecte toutes les règles du jeu.

| Gomme | Température finale, °C | Usure finale | Temps au plancher du facteur thermique | Durée, s |
| --- | ---: | ---: | ---: | ---: |
| soft | 67.15 | 27.54% | 72.9% | 2056.447 |
| medium | 68.37 | 13.89% | 74.0% | 1995.209 |
| hard | 68.56 | 9.47% | 64.5% | 1985.103 |

L’optimum catalogue est 100 °C. Les moyennes par tour diminuent dans cette ancre ;
il existe toutefois de courts intervalles de réchauffement. Un départ diagnostique
à 35 °C réchauffe le medium jusqu’à 53,12 °C : le modèle sait produire de la chaleur,
mais ces conditions l’équilibrent loin de l’optimum. Le simple fait de refroidir
depuis un départ préchauffé ne suffirait pas, seul, à caractériser un défaut.

Le medium historique finit à 64,98 °C et 11,54% d’usure ; le candidat finit à 68,37 °C
et 13,89%. La formule de pneus est inchangée : les forces/temps plus cohérents du
nouveau noyau changent son alimentation, pas sa structure.

Le facteur thermique direct du medium est plafonné vers le bas à 0,85, atteint
sous environ 81,86 °C. Entre 80 et 65 °C, ce facteur ne diminue donc plus ; le couplage
à l’usure continue néanmoins de dépendre de la température. Ce n’est pas un
plancher sur l’adhérence totale, qui dépend encore de l’usure. Saturer longtemps
ce facteur réduit la finesse d’interprétation des réglages et de la télémétrie.

Sur ce relais medium 20, enlever diagnostiquement le retour température→grip/usure
fait gagner 143,269 s ; enlever seulement usure→grip fait gagner 36,029 s. Les
mécanismes influencent donc réellement la course, sans que ces ablations prouvent
leur validité physique ou proposent des réglages accessibles au joueur.

Les hard gagnent le relais 20 testé. Parmi les trois stratégies medium→soft sur 20
tours (arrêt 5/10/15), la plus tardive reste la meilleure. Les arrêts sont exclus
du calcul des intervalles de réchauffement/refroidissement et les remises à zéro
restent distinguées. Cette observation rejoint l’ancien verdict 236 runs/6144 tours
`REFINE` : elle ne justifie pas un nouvel `ACCEPT` des stratégies.

Les changements de points/sliders modifient les chronos. Sur l’ancre Barcelone 5,
les températures finales varient cependant peu (environ 82,26 à 82,41 °C dans les
sliders testés). Les bons paramètres ne garantissent pas encore un levier thermique
clair pour le joueur.

## Autres limites conservées

Les tentatives V6T/Monza 53, réservoir initial natif par défaut 100 kg, s’épuisent aux
tours 43–44. Aucun temps final n’est inventé, aucun carburant ajouté pour forcer
l’acceptation. Ce n’est pas la reconstitution exacte de la requête manquante 445.
L’équilibre de course complète moderne reste à vérifier avec le contrat de
carburant avant de conclure sur la carrière entière.

Les libellés `conservative/aggressive` du premier plan étaient des politiques,
pas des ressources de pilote physique V9 : ils retombent sur le même pilote par
défaut. Le résultat identique est conservé comme limite du diagnostic, pas une
preuve d’absence d’influence de l’agressivité. Le suivi utilise `battery_voltas`
et `franz_hermann`, obtient des enveloppes/températures/usures distinctes et répète
ses quatre sorties exactement. Changer le pilote change aussi son flux aléatoire
seedé : ce suivi n’est pas une dérivée pure de l’agressivité ni un test du bouton
correspondant dans le jeu.

## Décision

Verdict **MECHANISMS_PRESENT / GAMEPLAY_REFINE**. Conserver les preuves, les
équations et les runtimes historiques ; poursuivre d’abord la physique/calibration
locale, puis seulement la qualification Worker/WASM. Les objectifs et critères du
prochain lot sont dans [NEXT_STEPS.md](NEXT_STEPS.md).

## Contrôles de livraison

[Reçu](results/native-receipt-v1.json) : deux tests natifs du nouveau probe et
quatre contrôles d’archives passent ; Clippy strict du probe et rustfmt passent.
Les sources physiques/manifests du candidat de coût déjà qualifié restent
identiques. Ses 224 tests appartiennent à la preuve antérieure ; les nouveaux
contrôles ciblent ici le probe et les archives. Les rapports primaires sont rejoués entièrement ; les
grilles rejouent leurs minima sélectionnés, pas leurs 374+72 points complets.
Les plans/digests/binaires sont conservés, et aucune inférence n’est appelée.

La ressource 2026 sert à éprouver sa famille thermique ; ce cœur n’ajoute pas
l’hybridation batterie/récupération ni un déblocage de carrière par ce test.
