# Transfert pneus · dix contextes I/II

**Résultat : `PASS_NATIVE_TRANSFER`.** Le candidat `progressive4` est évalué sans changement de coefficients ni de coût de stand. Aucun plateau de neuf pilotes ni modèle jouable n’est admis par ce rapport.

## Périmètre et calculs

- 10 contextes, 20 réglages comparés, pilote de référence Balanced et seed d’entraînement fixe.
- 5 682 entrées distinctes, dont 5 088 nouvelles et 594 réutilisations exactes du kernel déjà qualifié.
- 40 répétitions fraîches, égales champ par champ.
- 0 simulation en échec ; sauts involontaires entre tours : `[0.0, 0.0, 0.0, 0.0, 0.0]`.
- Résidu maximal du bilan temps de course/tours/stands : `3.64e-12 s`.
- Seeds et températures réservés du plateau toujours fermés ; aucune mesure de performance WASM/VPS ni validation industrielle.

## Comparaison physique des stratégies

Le premier réglage est le meilleur échantillonné sous un plan medium/hard à mi-course ; le second est le meilleur concentré moteur/châssis. S’ils sont identiques, une alternative avec cooling discrétionnaire positif est utilisée. Les stratégies sont ensuite réévaluées à chaque réglage. Les points ci-dessous sont discrétionnaires, dans l’ordre aéro/châssis/moteur/cooling ; les offsets HQ et composants restent séparés dans les reçus.

`Marge = meilleur temps à 3+ arrêts − meilleur temps à 1/2 arrêts`. Une valeur positive favorise un ou deux arrêts. Toute valeur négative est conservée comme contre-exemple, sans cap caché du solveur ni correction de chrono.

| Contexte | Réglage | Points A/C/E/K | Arrêts 1/2 | Arrêts 3+ | Marge (s) | Critère |
|---|---|---|---:|---:|---:|---|
| MONZA · Ère 1 | Meilleur échantillonné | 0/5/0/3 | 1 | 3 | +35.40 | Passe |
| MONZA · Ère 1 | Moteur/châssis | 0/8/0/0 | 1 | 3 | +36.07 | Passe |
| SILVERSTONE · Ère 1 | Meilleur échantillonné | 0/5/0/3 | 1 | 3 | +28.90 | Passe |
| SILVERSTONE · Ère 1 | Moteur/châssis | 0/8/0/0 | 1 | 3 | +29.42 | Passe |
| ZANDVOORT · Ère 1 | Meilleur échantillonné | 0/3/0/5 | 2 | 3 | +14.80 | Passe |
| ZANDVOORT · Ère 1 | Moteur/châssis | 0/8/0/0 | 2 | 3 | +14.72 | Passe |
| MEXICO · Ère 2 | Meilleur échantillonné | 0/15/0/1 | 2 | 3 | +15.58 | Passe |
| MEXICO · Ère 2 | Moteur/châssis | 0/16/0/0 | 2 | 3 | +16.42 | Passe |
| MONTREAL · Ère 2 | Meilleur échantillonné | 0/6/8/2 | 1 | 3 | +22.48 | Passe |
| MONTREAL · Ère 2 | Moteur/châssis | 0/11/5/0 | 1 | 3 | +19.25 | Passe |
| MONZA · Ère 2 | Meilleur échantillonné | 0/6/8/2 | 1 | 3 | +20.11 | Passe |
| MONZA · Ère 2 | Moteur/châssis | 0/0/16/0 | 1 | 3 | +39.83 | Passe |
| SAOPAULO · Ère 2 | Meilleur échantillonné | 0/6/8/2 | 1 | 3 | +20.49 | Passe |
| SAOPAULO · Ère 2 | Moteur/châssis | 0/16/0/0 | 2 | 3 | +15.02 | Passe |
| SILVERSTONE · Ère 2 | Meilleur échantillonné | 0/15/0/1 | 1 | 3 | +28.31 | Passe |
| SILVERSTONE · Ère 2 | Moteur/châssis | 0/16/0/0 | 2 | 3 | +15.92 | Passe |
| SPIELBERG · Ère 2 | Meilleur échantillonné | 0/6/8/2 | 1 | 3 | +35.89 | Passe |
| SPIELBERG · Ère 2 | Moteur/châssis | 0/16/0/0 | 1 | 3 | +22.03 | Passe |
| ZANDVOORT · Ère 2 | Meilleur échantillonné | 0/14/0/2 | 1 | 3 | +16.28 | Passe |
| ZANDVOORT · Ère 2 | Moteur/châssis | 0/16/0/0 | 2 | 3 | +10.57 | Passe |

## Lecture et suites

Les vingt comparaisons satisfont les critères enregistrés : les arrêts supplémentaires ne sont pas rentables dans cet échantillon, avec les mêmes coefficients pneus partout. Ce résultat autorise la prochaine campagne de calibration des neuf pilotes ; il ne la remplace pas et ne prouve pas un optimum global.

Le meilleur nombre d’arrêts échantillonné change entre les deux réglages à SAOPAULO (ère 2), SILVERSTONE (ère 2), ZANDVOORT (ère 2). C’est une observation du couplage réglages/stratégie, pas un ordre d’arrivée imposé.

Le meilleur réglage du premier balayage n’est pas nécessairement le meilleur après changement de stratégie. À Silverstone II, la configuration moteur/châssis échantillonnée devient environ 1,64 s plus rapide que l’autre après comparaison des plans. Le prochain réglage du plateau doit donc chercher conjointement préparation et stratégie, sans imposer une dépense sur chaque axe.

## Figures

- [Comparaison des arrêts, vingt réglages](figures/stop-incentives-v1.png)
- [Écarts entre réglages après comparaison des stratégies](figures/setup-tradeoffs-v1.png)
- [États par tour du cas à la marge la plus faible](figures/closest-comparison-v1.png)

Le choix des réglages reste conditionné au plan medium/hard de la première recherche. Les 114 plans et 84 contre-épreuves forment un échantillon discret ; toutes les combinaisons continues de réglages et séquences ne sont pas explorées. Les offsets présents en ère II peuvent légitimement réduire l’intérêt de points discrétionnaires supplémentaires de cooling. Aucun critère n’impose de dépenser sur les quatre axes.

Les chronos comparent des voitures indépendantes : pas de trafic, aspiration, collision ni météo dynamique. Les courbes d’évolution présentent des valeurs en fin de tour et ne décrivent pas les variations instantanées dans chaque virage. L’usure brute est un budget interne, pas une probabilité de crevaison.

Protocole, conservation et reproduction : [README](README.md), [plan](plan-v1.json). Preuves : [campagne](results/campaign-v1.json), [résumé](results/summary-v1.json), [réglages figés](results/anchor-lock-v1.json).
