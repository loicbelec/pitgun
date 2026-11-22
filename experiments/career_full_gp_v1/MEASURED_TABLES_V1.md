# Lecture chiffrée de la campagne GP

Différences brutes, coefficients et tolérances inchangés. Les diagnostics +1 sont hors budget.

## Configurations retenues et stratégies optimisées dans la grille

Écart = concentré − diversifié. Positif : configuration diversifiée plus rapide.

| Étape | Circuit | Écart GP, stratégie source fixe | Écart GP, recettes sélectionnées | Allocation concentrée A/C/E/K | Allocation diversifiée A/C/E/K |
| --- | --- | ---: | ---: | --- | --- |
| era1-entry | MONZA | +78.757 s | +72.161 s | [0, 8, 0, 0] | [0, 5, 0, 3] |
| era1-entry | ZANDVOORT | +229.650 s | +207.325 s | [0, 8, 0, 0] | [0, 3, 0, 5] |
| era1-exit | MONZA | +78.570 s | +71.907 s | [0, 11, 0, 0] | [0, 8, 0, 3] |
| era1-exit | ZANDVOORT | +229.593 s | +206.854 s | [0, 11, 0, 0] | [0, 6, 0, 5] |
| era2-exit | MONZA | +3.476 s | -18.910 s | [0, 16, 0, 0] | [0, 9, 6, 1] |
| era2-exit | ZANDVOORT | -1.628 s | -0.218 s | [0, 16, 0, 0] | [0, 15, 0, 1] |

## Réponse isolée à +1 point, au-delà du budget carrière

| Étape | Circuit | Axe | Δ temps GP | Δ pic moteur | Δ durée derating |
| --- | --- | --- | ---: | ---: | ---: |
| era1-entry | MONZA | aero | -0.031 s | -0.017 °C | -13.930 s |
| era1-entry | MONZA | chassis | -3.378 s | -0.017 °C | -6.926 s |
| era1-entry | MONZA | engine | -1.170 s | +0.022 °C | +14.585 s |
| era1-entry | MONZA | cooling | -0.206 s | -7.737 °C | -1308.658 s |
| era1-entry | ZANDVOORT | aero | -0.098 s | -0.010 °C | -0.370 s |
| era1-entry | ZANDVOORT | chassis | -5.623 s | +0.014 °C | -6.027 s |
| era1-entry | ZANDVOORT | engine | -2.300 s | +0.051 °C | -1.973 s |
| era1-entry | ZANDVOORT | cooling | -27.818 s | -8.904 °C | -297.214 s |
| era1-exit | MONZA | aero | -0.030 s | -0.017 °C | -15.448 s |
| era1-exit | MONZA | chassis | -3.366 s | -0.017 °C | -8.446 s |
| era1-exit | MONZA | engine | -1.136 s | +0.023 °C | +13.515 s |
| era1-exit | MONZA | cooling | -0.220 s | -7.747 °C | -1344.381 s |
| era1-exit | ZANDVOORT | aero | -0.096 s | -0.009 °C | -0.292 s |
| era1-exit | ZANDVOORT | chassis | -5.601 s | +0.008 °C | -6.057 s |
| era1-exit | ZANDVOORT | engine | -2.247 s | +0.053 °C | -1.933 s |
| era1-exit | ZANDVOORT | cooling | -27.431 s | -8.929 °C | -289.932 s |
| era2-exit | MONZA | aero | -1.533 s | +0.000 °C | +0.000 s |
| era2-exit | MONZA | chassis | -2.242 s | +0.009 °C | +0.000 s |
| era2-exit | MONZA | engine | -3.180 s | +0.020 °C | +0.000 s |
| era2-exit | MONZA | cooling | +1.193 s | -0.798 °C | +0.000 s |
| era2-exit | ZANDVOORT | aero | -2.379 s | -0.004 °C | +0.000 s |
| era2-exit | ZANDVOORT | chassis | -4.881 s | +0.000 °C | +0.000 s |
| era2-exit | ZANDVOORT | engine | -2.016 s | +0.036 °C | +0.000 s |
| era2-exit | ZANDVOORT | cooling | +0.580 s | -1.240 °C | +0.000 s |

1235 requêtes sémantiques, 1223 entrées de diagnostic distinctes (physique + anticipation), 1211 entrées physiques distinctes ; 1307 processus natifs de calcul, confirmations/répétitions incluses.
Échecs de simulation conservés : 0. Aucun échec de résolution n'est assimilé à un DNF.

Les témoins de sélection et conditions réservées sont détaillés dans summary-v1.json. Le champ de neuf adversaires n'est pas exécuté.
