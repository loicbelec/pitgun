# Pneus : correction de l’incitation aux arrêts

**Candidat natif retenu : `progressive4`.** Il passe le protocole physique réduit ; il n’est pas encore admis dans le jeu, WASM ou Authority/Verifier. Les caractéristiques et le plateau des neuf IA restent à évaluer sous ce modèle.

## Résultats de la campagne

- 2 502 exécutions natives fraîches, dont 100 GP source exacts, 2 contrôles historiques et 4 répétitions choisies.
- 0 simulation en échec ; continuité vitesse/moteur/surface/carcasse/usure : `[0.0, 0.0, 0.0, 0.0, 0.0]`.
- Coût réel conservé : 22 s par arrêt. Résidu maximal du bilan temps total/tours/stands : `3.64e-12 s`.
- Aucun seed réservé du futur plateau n’a été ouvert.

Les stratégies normales comportent un ou deux arrêts. Les plans de 3 à 12 arrêts sont explicitement diagnostiques : ils ne peuvent pas devenir des plans du plateau. La colonne « marge 3+ » donne le meilleur temps diagnostique moins le meilleur temps légal ; une valeur positive favorise le plan légal.

| Courbe | Contexte | Arrêts du meilleur plan légal | Temps légal (s) | Marge 3+ (s) | Critères |
|---|---|---:|---:|---:|---|
| progressive4 | Ère 1 · MONZA | 1 | 7785.17 | +35.40 | Passe |
| progressive4 | Ère 1 · ZANDVOORT | 2 | 8410.69 | +14.80 | Passe |
| progressive4 | Ère 2 · MONTREAL | 1 | 6788.74 | +18.82 | Passe |
| progressive4 | Ère 2 · MEXICO | 2 | 7371.53 | +15.58 | Passe |
| progressive8 | Ère 1 · MONZA | 1 | 7779.14 | +39.62 | Passe |
| progressive8 | Ère 1 · ZANDVOORT | 1 | 8392.38 | +24.18 | Passe |
| progressive8 | Ère 2 · MONTREAL | 1 | 6770.80 | +27.67 | Passe |
| progressive8 | Ère 2 · MEXICO | 1 | 7357.69 | +17.69 | Passe |
| progressive2 | Ère 1 · MONZA | 1 | 7810.51 | +25.73 | Passe |
| progressive2 | Ère 1 · ZANDVOORT | 2 | 8445.99 | +7.64 | Passe |
| progressive2 | Ère 2 · MONTREAL | 2 | 6824.30 | +7.27 | Passe |
| progressive2 | Ère 2 · MEXICO | 2 | 7419.35 | -0.57 | REFINE |

## Sens et limites

La loi progressive conserve l’accumulation énergétique et thermique d’usure. Elle réduit surtout la pénalité prématurée de grip. La fenêtre thermique et les écarts de grip entre composés sont des hypothèses explicites de modèle réduit, communes à tous les circuits ; aucun temps final n’est corrigé artificiellement.

La sélection est globale, selon la priorité et les critères enregistrés avant calcul. Ces comparaisons discrètes ne prouvent pas un optimum global ni une calibration industrielle. Elles couvrent quatre contextes des ères I/II à réglages fixés. Le transfert aux autres circuits, aux neuf pilotes, aux budgets et aux conditions réservées fait partie de la campagne suivante.

Le bilan des modes de conduite et les essais de cooling sur cinq tours sont disponibles dans les reçus ; le +1 cooling hors budget isole un effet physique et ne représente pas un duel légal à budget égal. Une température initiale de 70 °C est une hypothèse de préparation du jeu, pas une reconstitution historique de toutes les ères.

Références, équations et protocole : [README](README.md), [profils](profile-candidates-v1.json), [plan figé](plan-v1.json). Reçus complets : [campagne](results/screen-campaign-v1.json), [résumé JSON](results/summary-v1.json).
