# Qualification thermique pneus —22novembre2026

Source compilée `e4d4d2b`, plan/profil figés avant mesure. **Préparation et lecture
du grip qualifiées nativement ; paramètres physiques et équilibre REFINE.**
Les conditions du profil sont modernes et explicitement sélectionnées ; les
références anciennes restent inchangées. Aucune activation dans le jeu.

[Banc visuel](index.html) · [Vue d'ensemble](figures/overview.png) ·
[Rapport natif](results/screen-v1.json) · [SHA256](results/screen-v1.sha256) ·
[Sources et hypothèses](README.md)

## Ce qui est implémenté

- Politique `pitgun.racing-tire-preparation/v1` : surface70°C au montage et aux
  remplacements, limite externe70°C, carcasse homogène70°C par simplification.
  Le départ froid/chaud du diagnostic n'est plus implicitement repris aux stands.
- Politique d'événements `pitgun.racing-tire-window-policy/v2` : bornes dérivées
  du facteur thermique réel,>=95% de son maximum. Ce seuil est une convention
  Pitgun. Aucun clipping des températures vers la fenêtre ; l'usure reste
  irréversible et distincte du facteur thermique réversible.
- Courbes surface/carcasse, fenêtre, facteur thermique, grip effectif et usure,
  avec zoom initial. Statistiques et191faits thermiques utilisent les points
  natifs acceptés ; seules les courbes sélectionnent moins de points.
- Ancienne politique V1 et défauts conservés. Aucun nouveau paramètre de gomme,
  bilan thermique, tolérance de convergence ou cadence physique modifié. La
  fonction thermique existante a seulement été extraite pour partager sa lecture.

## Fenêtres du candidat et comportements observés

Ces bornes proviennent du profil **provisoire de Pitgun**, pas de spécifications
Pirelli identifiées. Le facteur thermique exclut usure et multiplicateur de gomme.

| Gomme | Optimum | Bande>=95% | Première entrée,Barcelona70°C | Temps chauffe/refroidissement |
| --- | ---: | ---: | ---: | ---: |
| Soft |90°C |81.39–98.61°C |195.08s,tour2 |45.3% /54.7% |
| Medium |95°C |85.49–104.51°C |339.93s,tour4 |55.3% /44.7% |
| Hard |100°C |89.13–110.87°C |489.75s,tour5 |61.4% /38.6% |

Les pourcentages de chauffe/refroidissement couvrent tout le relais de20tours.
Le premier franchissement n'est pas le maintien durable dans la fenêtre. Une
lecture des seules fins de tours masque les baisses locales ; le tracé à l'échelle
du relais peut sembler croissant alors que la surface refroidit fréquemment.

Pour le medium à70°C initial,27.1% du temps natif de roulage est proche du pic,
15.6% froid et57.3% chaud ; pas de temps au plancher thermique0.65. Le grip effectif
continue néanmoins de baisser avec l'usure, qui atteint76.33%. Le départ35°C
termine107.68°C, avec16.8% du roulage au plancher ; le départ110°C refroidit
initialement puis réchauffe, et finit110.15°C. Aucune hausse monotone imposée.

L'environnement froid15°C air/20°C route finit93.00°C de surface et passe60.4%
du temps proche du pic ; l'environnement chaud45/60°C finit116.64°C et passe74.1%
du temps au-dessus de la bande. Ce sont des scénarios couplés : air modifie aussi
le moteur, et les trajectoires changent. Pas une identification causale d'un seul
coefficient d'échange pneu.

Sur le medium20tours,Manage/Balanced/Attack finit respectivement73.16/76.33/79.07%
d'usure, pour2179.841/2178.515/2181.365s. La sensibilité au pilotage est réelle,
mais un seul setup/seed ne qualifie pas l'équilibre global des modes.

## Remplacement et stratégies

Le cas départ35°C/soft→medium au tour10 monte jusqu'à100.98°C surface et114.08°C
carcasse avant service. Le nouveau train revient explicitement à70/70°C, et le
tour suivant reprend exactement ces deux états. Les continuités surface/carcasse
entre états acceptés après service et tour suivant sont exactement nulles dans
tous les cas. Il s'agit d'un nouveau train, pas d'une perte de mémoire des pneus
inchangés à chaque tour.

Les softs conservent un avantage initial :295.803s sur3tours contre313.660 medium
et327.331 hard. Le hard reste le meilleur de ces trois relais20tours sans arrêt :
2076.173s contre2178.515 medium et2439.784 soft. Dans les trois medium→soft testés,
l'arrêt au tour10 est cette fois meilleur :2062.939s, contre2073.404 au tour15 et
2234.121 au tour5. Ce changement de classement apparaît avec les conditions de
préparation explicites et le profil pilote déjà qualifié, sans nouveau réglage
des coefficients. Ce n'est pas la résolution générale de la domination du hard
ou des arrêts tardifs ; les courses réglementaires/stratégies légales restent à
qualifier séparément.

## Reproductibilité et contrôles

-23cas×2passages : les23terminent à chaque passage ; totalité des données,
  préfixes acceptés, métriques,191faits et digests exactement répétés.
-4références thermiques inchangées,56cas pilotes et22résultats/erreurs historiques
  restent exacts, dont les deux épuisements carburant attendus.
-8cas d'événements V1 reproduisent exactement résultats physiques, journaux,
  identités, ordres, aperçus et comparaison service/checkpoint ; leurs16exécutions
  normales/reprises restent inchangées. Ce n'est pas une reprise de processus.
-591tests du workspace passent,0échec,22ignores existants. Les3nouveaux tests
  portent sur la préparation/reset réel, la correspondance courbe/fenêtre et la
  restauration de la nouvelle identité d'événement. Compilation de tout le
  workspace/toutes cibles locales, lints natifs ciblés et formatage passent.
- Le contrôle CI parent avait échoué avec Clippy1.100 sur une ancienne vérification
  `is_some` suivie de `unwrap`. Refactorisation sans effet physique `06f7fe7`,
  reprise dans ce candidat et couverte par les régressions ci-dessus. La réussite
  locale n'annonce pas une CI distante entièrement terminée.
- Banc statique contrôlé dans un Chrome headless isolé,1440×1000 et390×844 :
 23scénarios proposés, changement de figure effectif, aucune erreur JavaScript
  ou débordement horizontal de page. Les figures scientifiques ont aussi été
  inspectées directement. Aucune recette du nouveau moteur en navigateur.

Le premier lancement de l'outil a été bloqué par le garde-fou de sources de
l'ancien banc d'événements. Un nouveau mode explicitement nommé
`--requalify-physical-source` conserve ce garde-fou par défaut et permet une
requalification par égalité complète des anciennes entrées/sorties/service et
journaux. Le reçu indique honnêtement `physical_sources_match_frozen:false` :
les fichiers Solver/énergie ont changé depuis le premier écran, même si les
résultats gelés restent exacts. Aucune ancienne archive remplacée.

## Limites et suite

La préparation ne simule pas les couvertures, leur durée, le transport vers la
grille, la pression ou quatre pneus distincts. Les valeurs70°C carcasse et95%
du pic ne sont pas des prescriptions mesurées. Les capacités/conductances,
proxies de déformation et optimums restent exploratoires. Les températures de
carcasse peuvent ici dépasser la surface, avec un pic medium128.94°C ; ces
magnitudes doivent être confrontées à des données avant toute revendication de
fidélité physique.

La mise en température demande encore2/4/5tours pour les premiers franchissements
soft/medium/hard de ces conditions. Ce résultat doit être qualifié, pas ramené
arbitrairement à un nombre de tours souhaité. Le plancher thermique masque une
partie de la sensibilité dans certains départs froids. La prochaine évolution
physique devra réserver des cas et des critères avant d'ajuster ces paramètres.

Prochaine étape : anticipation bornée #461, à partir des vrais états pneus et
conditions de montage, avec faisabilité vérifiée avant de commettre le tour.
Ensuite campagne réelle allocations/sliders/gommes et adversaires #457. Le gain
modeste de cooling au début reste le choix validé ; aucune taxe artificielle
sur zéro point. Admission Worker/WASM/Authority et déploiement restent ultérieurs.
Ce lot n'a pas mesuré coût/RSS navigateur ni gain de performance supplémentaire.
