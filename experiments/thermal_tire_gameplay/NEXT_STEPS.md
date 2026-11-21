# Suite proposée avant Worker/WASM

Le verdict de l’audit est `REFINE`. La priorité demandée par le propriétaire est
la pertinence des réglages et des pneus ; le raccordement WASM du nouveau noyau
vient après ce travail. Les optimisations de coût déjà qualifiées sont conservées.
Aucune nouvelle équation ou calibration n’a été intégrée par cet audit.

## 1. Fixer des critères de gameplay vérifiables

Le joueur doit arbitrer suivant le moteur, le circuit, les conditions et son
objectif. Zéro point peut être rationnel dans un contexte : cela ne signifie ni
absence d’équipement ni paramètre inutile. L’objectif de refroidissement est une
frontière lisible entre capacité suffisante et perte de performance, avec un coût
réel de surdimensionnement et de budget. Une allocation positive imposée partout
sans bilan physique ne démontrerait pas l’intérêt du modèle.

Vérifier les meilleurs réglages **avec zéro et avec cooling positif**, au même
budget, avec exploration des autres axes et sliders. Les lignes à un seul donneur
et les pas grossiers ont montré leur limite. Examiner aussi la domination du
châssis et les moteurs installés assez puissants pour gagner avec zéro point
moteur. Les points ne remplacent pas le choix de la technologie installée.

Pour les pneus : mettre en évidence des fenêtres de fonctionnement et des choix
court/long relais ; demander une stratégie explique sa décision par grip, usure,
température ou situation tactique, pas un pourcentage arbitraire obligatoire.

## 2. Reprendre le bilan thermique moteur et le mapping de cooling — #420

Tracer travail moteur, chaleur produite, énergie stockée, échange air, seuil et
réduction effective de puissance sur une voiture physique identique. Le V8 testé
ne s’approche pas du seuil : séparer capacité de base, puissance réellement
sollicitée et coût des développements avant de toucher au derating.

Vérifier unités et hypothèses des lois de chaleur et d’échange, régime de roulage
et de service, dépendance à la vitesse/air et propriétés de chaque groupe moteur.
Le futur lien rendement/consommation/chaleur devra s’appuyer sur un bilan énergétique
explicite ; l’ancien `alpha_heat × shaft_work` ne ferme pas un bilan chimique.
Étudier capacité du radiateur, coût en traînée et, si retenu, masse. Le moteur doit
pouvoir être davantage refroidi sans transformer systématiquement le maximum de
cooling en meilleur choix. Les références et données doivent justifier les plages.

Conserver des V8 robustes si c’est physiquement cohérent ; si le gameplay exige un
compromis plus tôt, corriger le dimensionnement ou le domaine de charge, sans
inventer une pénalité cachée au réglage zéro.

## 3. Reprendre l’équilibre et la réponse des pneus — #421

Distinguer surface/carcasse/air/piste selon la précision utile. Comparer une loi à
un état améliorée et deux états thermiques avant de retenir davantage de variables.
La température utilisée par la loi de grip doit représenter la même zone que sa
fenêtre optimale : ne pas comparer sans justification une température de masse
agrégée à une cible de surface.

Documenter les flux et leur origine : contact/glissement ou proxy clairement nommé,
roulement, échanges air/piste, et éventuel transfert des freins. Ne pas attribuer
toute l’énergie dissipée aux freins au pneu. Les paramètres doivent appartenir au
bon sous-modèle et pouvoir différer entre gommes si la physique le justifie.

Les coefficients actuels produisent souvent des pneus froids avec un facteur de
grip thermique saturé ; modifier seulement ce plancher déplacerait le problème.
Éprouver échauffement et refroidissement sous des charges différentes, températures
initiales/ambiantes différentes, pression de conduite et évolution d’usure. La
littérature NASCAR et les autres références de la fondation restent à lire au
niveau des équations retenues ; leurs paramètres ne sont pas transférables tels
quels à toutes les voitures de Pitgun.

Identifier ensuite usure, perte de grip et durée compétitive utile : gommes tendres
avantageuses sur certains relais courts et dégradation plus rapide ; solutions
moyennes/dures compétitives ailleurs ; arrêts dont le meilleur instant n’est pas
systématiquement le dernier. Vérifier le sens de l’échelle0–100% avant tout
changement de contrat d’affichage/observation. Une usure linéaire, une température
et la durée de vie utile sont des grandeurs distinctes.

## 4. Calibrer et valider séparément, puis raccorder

Les scénarios publiés ici sont désormais exposés : ils servent au diagnostic,
pas de nouveaux cas « inconnus ». Figer de nouvelles conditions de validation
avant la sélection des coefficients. Exécuter localement les scénarios de référence,
les recherches de réglages, des relais et des distances complètes, avec tous les
échecs conservés. Les limites carburant modernes (#445/#246) doivent être résolues
ou clairement séparer les conclusions qu’elles empêchent ; pas de réservoir caché.

Publier les équations/unités, les références effectivement adoptées, les données
et plages identifiables, les marges numériques, le coût CPU/RSS et l’arbitrage de
fidélité. Les bilans, les tests analytiques, la reproductibilité et une validation
sur données sont des preuves différentes.

Une fois ces mécanismes acceptés : nouvelle identité de modèle/profil, export
versionné, qualification native/WASM, replay Authority/Verifier et recette du jeu.
Les vieux moteurs restent nécessaires aux preuves passées. Les fréquences des
courbes, les agrégats natifs, les événements et les observations d’agent restent
distincts. Le framework générique reste indépendant des règles automobiles.
