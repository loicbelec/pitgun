# Frontières de tour bornées —22novembre2026

Source `0efdfa3` (algorithme/tests `3395538`, seuls les extraits de courbes ont
ensuite été bornés). **Qualification numérique native réussie sur ce domaine ;
anticipation thermique encore prudente, validation carrière/classement ouverte.**

[Banc comparatif](index.html) · [Méthode et limites](README.md) ·
[Rapport gelé](results/screen-v1.json) / [SHA256](results/screen-v1.sha256) ·
[Coût natif](results/cost-v1.json) / [SHA256](results/cost-v1.sha256)

## Changements ciblés

Le mode natif explicite `reachable_wear_v1` borne les états pneus atteignables
depuis la fin réellement calculée du tour courant. Il ne suppose plus une usure
de100% pour tout le tour suivant. Il retire aussi la fermeture périodique locale
qui identifiait implicitement les contraintes de départ et d'arrivée du même tour,
alors que les pneus évoluent. Le prochain horizon garde un ancrage conservateur.

Le candidat converge, propose une borne, puis recalcule uniquement le tour encore
non commis. Avant publication, la vitesse réellement obtenue doit respecter un
nouveau certificat issu du nouvel état final. Le tour suivant vérifie la présence
de ses deux températures et de son usure dans l'enclosure annoncée. Aucune vitesse
n'est coupée à la ligne ; aucun ancien échantillon publié n'est réécrit.
La température minimale de grip reste utilisée comme marge de prudence. Les
coefficients thermiques/pilotes, le maillage natif et les tolérances restent fixes.

## Trajectoires et freinage de fin de tour

Même entrée physique, même pilote/mode/seed, deux algorithmes d'anticipation :

| Cas | Vitesse ligne,tour1 référence→candidat | Travail frein final300m,tour1 | Temps total référence→candidat |
| --- | ---: | ---: | ---: |
| V8 précoce,Monza5 |95.93→133.48km/h |415.38→130.53kJ |807.208→805.232s |
| V8 classique,Monza5 |105.14→157.55km/h |1289.25→854.51kJ |615.944→613.427s |
| Moderne,Silverstone5 |232.26→294.19km/h |833.85→0.00kJ |572.163→570.614s |
| Moderne,Barcelona20 |277.77→277.77km/h |0.00→0.00kJ |2178.187→2178.187s |
| V8 classique,Monza53 |105.14→157.55km/h |1289.25→854.51kJ |6632.779→6607.143s |

Silverstone ne présente plus le freinage final ajouté par l'ancienne anticipation
dans ce cas. Barcelona n'est pas modifié. Monza garde un freinage conservateur,
nettement réduit : le plancher thermique n'est pas une estimation optimale des
états du prochain virage. Un temps plus court ne prouve pas à lui seul une
meilleure précision physique ou l'absence de biais sur tous les réglages.

La géométrie du catalogue explique aussi pourquoi un freinage peut rester
nécessaire à cette frontière : dans Monza1.8.1, la courbure atteint environ
0.00966m⁻¹ dans les50premiers mètres après l'origine du tour natif. Cette origine
n'est pas une preuve de positionnement de la ligne de départ réelle. On ne doit
pas imposer une courbe plate à tous les passages sans lire la géométrie suivante.
Les coordonnées/courbures de l'ancien catalogue n'ont pas été retouchées ici.

Le dernier tour n'a plus de contrainte de tour suivant. Monza classique passe
ainsi230.87km/h à la dernière ligne contre157.55 au tour1 dans le candidat ; cette
différence ne démontre pas à elle seule une rupture de calcul. Il faut lire la
géométrie, les états pneus et le travail de freinage avec les horizons différents.
Le panneau de certificat expose séparément la vitesse réellement commise et sa
borne, avec les tours de service explicitement distingués.

Le compromis de cooling précoce reste modeste : transfert des deux points moteur
vers cooling,Monza5, gain1.010s dans ce candidat contre1.155s dans la référence.
Pas d'amplification artificielle des lois pour imposer des dépenses sur tous les axes.

## Qualification, frontières et répétition

-26scénarios×2modes×2passages,104exécutions principales : les deux modes terminent
  partout. Toute la donnée hors horloge est exactement répétée : inputs, sorties,
  tours, compteurs, certificats, états et extraits natifs.
-8scénarios réservés avant leur première exécution sous ce candidat : downforce
  bas/haut, chaleur, arrêt tardif, autre seed de GP complet, châssis, hard très
  chaud/usé et départ froid en Attack. Aucun coefficient/algorithme ajusté après
  lecture de ces huit résultats.
-Les cinq écarts au passage entre états post-service et prochain tour sont
  exactement nuls : vitesse, moteur, surface, carcasse et usure. Les six reprises
  pré-service inchangées (trois scénarios×deux modes) reproduisent tout le rapport
  et le résultat final exactement. Ce n'est pas une sauvegarde de processus.
-Aucun recalcul conservateur de secours dans les26cas ; au plus une mise à jour
  de borne non commise par frontière. Le mécanisme de resserrement/secours reste
  borné et explicite, sans recours observé dans cette matrice.
-Le maximum du contrôle de sollicitation du contact est0.99894043, sous la
  tolérance inchangée1.0002 ; erreur relative de couplage maximale0.00099750,
  sous0.001. Maximum13passes sur24permises. Ce sont des contrôles numériques sur
  le modèle réduit, pas une validation expérimentale du pneu.
-Les8références de l'ancien cache restent exactement égales en mode historique,
  y compris inputs/digests/tours/compteurs. Les23cas de préparation répétés deux
  fois,56cas pilotes,22anciens résultats/erreurs et8cas d'événements V1 restent
  exacts sous les constructeurs ordinaires. Les deux anciens épuisements carburant
  attendus restent des échecs explicites.
-594tests du workspace passent,0échec,22ignores existants ; compilation complète
  toutes cibles locales, Clippy strict ciblé/CI et formatage passent. Les nouveaux
  tests couvrent l'enclosure avec durées/charges variables, la monotonie de capacité
  de freinage avec masse/grip/assistance passive, le préfixe non réécrit et la
  parité de service. Aucun test ni tolérance affaibli.
-Banc statique vérifié dans un Chrome headless isolé,1440×1000 et390×844 :
  26scénarios, sélection/chargement effectif des figures, aucune erreur JavaScript
  ou débordement horizontal. Figures inspectées directement. Cette recette ne
  constitue pas une exécution du nouveau moteur en WASM dans le navigateur.

Le wrapper diagnostique entier, pas seulement l'input physique, distingue ces
algorithmes. Les compteurs décrivent des tentatives numériques ; ils ne sont pas
des événements de course ou des observations adressées à un Engineer.

## Coût du calcul natif

Mesure après la fin des tests/autres campagnes :7paires sérielles alternées par
scénario, même binaire/input, donnée de chaque mesure comparée au reçu gelé.
Médiane de la somme des appels `advance`, hors résolution/constructeur,
statistiques, digest et JSON. L'activité générale du Mac n'est pas contrôlée.

| Cas | Référence minimum global | Candidat borné | Surcoût |
| --- | ---: | ---: | ---: |
| V8 précoce,Monza5 |228.32ms |301.73ms |+32.2% |
| Moderne,Barcelona20 |856.08ms |871.74ms |+1.8% |
| V8 classique,Monza53 |2735.06ms |3438.59ms |+25.7% |

Ce surcoût achète l'enclosure et la certification/reconvergence avant publication.
Les trajectoires diffèrent : ce n'est pas une optimisation à sortie identique,
ni un gain/perte cumulable directement avec l'ancien cache. Mesure native sur le
Mac local, pas un champ de10voitures, le navigateur, WASM, le VPS ou la mémoire.
Les bornes ajoutent un nombre linéaire de tableaux à la taille d'un tour ; aucun
historique supplémentaire de télémétrie complète n'est conservé pour elles.

## Ce qui reste avant calibration fine et admission

Les cas réservés sont des requêtes physiques du banc, pas les54requêtes réellement
issues des snapshots de carrière. La résolution thermique des composants
effectivement acquis, la matrice carrière des èresI/II et les effets sur le
classement allocations/sliders/stratégies restent à qualifier. Les structures
indépendantes des adversaires restent inchangées ; aucun fitting IA effectué.

Le plancher thermique et le majorant chaleur/usure peuvent encore trop freiner.
Leur prudence est vérifiée dans ce domaine, sans prouver l'optimalité
de l'anticipation ou d'un solveur non linéaire quelconque. Pas de météo variable,
changements libres de contrôle pilote, contact entre véhicules ou nouvelle
trajectoire physique de stands qualifiés. Monaco complet reste hors de ce lot.

Le ticket#461reste ouvert pour ces contrôles/biais. Après validation du candidat
sur les configurations de carrière : écran de réglages puis fitting fixe des IA
#457 et évaluation réservée. Export/checkpoint de processus, admission indépendante
native/WASM/Authority/Verifier et jeu/staging/prod restent des étapes distinctes.
