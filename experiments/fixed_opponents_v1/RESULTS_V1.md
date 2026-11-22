# Premier plateau fixe — résultat négatif conservé

**Décision : REFINE.** Les favoris sont les deux premiers sur l'entraînement,
mais le joueur informé ne peut gagner aucun des GP de sortie d'ère avec les
calibrations proposées. Le plateau n'est donc pas sélectionné ni admis.

Source figée : `6da3783`. Les équations, profils physiques, tolérances, anciennes
ressources et le pilote du joueur restent inchangés. Les nouveaux pilotes sont
des ressources V2 explicites, pas des multiplicateurs de temps.

| Mesure | Résultat |
| --- | ---: |
| Contextes circuit/ère I–II | 10 |
| Calculs natifs de GP complets dans la campagne | 6 155 |
| Entrées diagnostiques distinctes | 6 084 |
| Calibrations budget I × budget II × talents favoris | 27 |
| Calibrations satisfaisant tous les critères annoncés | **0** |
| Échecs de simulation | 0 |
| Discontinuités vitesse/moteur/surface/carcasse/usure aux tours | 0 |
| Résultats réservés ouverts | **0** |

Le décompte exclut les contrôles de qualification et les essais exploratoires
annexes. Les alias de cas sont dédupliqués dans le calcul natif ; les 6 084 entrées
diagnostiques distinguent également les doublons physiques conservés sous
plusieurs préparations. Les reçus compressés restent dans [results](results/).

À la calibration la moins difficile, budgets discrétionnaires fixes **6/12**
et niveau de talents de base :

| Joueur informé en sortie d'ère | Positions sur les GP d'entraînement | Victoires | Gain moyen sur réglage initial |
| --- | --- | ---: | ---: |
| I | 3, 3, 3 | 0/3 | 749,44 s/GP |
| II | 3, 3, 3, 3, 3, 4 | 0/6 | 224,23 s/GP |

Charles L'Éclair et Franz Hermann occupent les deux premières places agrégées
sur les 27 calibrations. Cela ne suffit pas : rendre la victoire inaccessible
au joueur développé contredirait le critère fixé avant les calculs. Les écarts
ci-dessus sont des simulations d'entraînement, pas des observations de joueurs
ni une estimation statistique de difficulté en production.

Les meilleurs plans échantillonnés des 90 couples pilote/contexte utilisent trois
arrêts, borne haute de ce premier espace. Il faut donc aussi tester des plans
au-delà de trois arrêts avant de présenter un plateau plus largement qualifié.

La [V2](../fixed_opponents_v2/README.md) abaisse explicitement l'échelle des talents
de base, préserve les écarts entre personnalités et étend les stratégies. Elle
recalcule les neuf pilotes. Les données négatives et les critères de cette V1
restent intacts ; aucun chrono n'est retouché et aucune physique n'est ajustée
pour imposer la hiérarchie. La validation réservée et l'admission du futur
plateau restent des étapes distinctes.

Les bilans conservent une utilisation de contact maximale de 0,997759, au plus
15 passes de couplage et des résidus énergétiques thermiques inférieurs à
`2.4e-8 J`. Trente-quatre tours utilisent le recalcul conservateur certifié du
diagnostic borné ; ce ne sont pas des arrivées forcées ni des échecs supprimés.
Voir le [résumé reproductible](results/progress-summary-v1.json), les
[27 calibrations](results/calibration-candidates-v1.json) et la
[décision de raffinement](results/training-decision-v1.json).
