# Plateau fixe de neuf IA · résultats natifs V3

**Verdict : `NATIVE_I_II_CANDIDATE`.** Profil physique `progressive4` inchangé. Le jeu, les catalogues publiés, WASM et Authority/Verifier ne sont pas modifiés par ce rapport.

## Calculs et stabilité

- 24 966 exécutions natives terminées, dont 180 répétitions forcées et 90 simulations dans les assemblages natifs.
- 24 696 entrées distinctes de campagne ; la réutilisation des références du transfert est enregistrée séparément.
- 0 simulation(s) exploratoire(s) en échec, toutes conservées.
- Continuité vitesse/moteur/surface/carcasse/usure : `[0.0, 0.0, 0.0, 0.0, 0.0]`.
- Résidu maximal tours/stands/temps total : `3.64e-12 s`.

## Qualification réelle des stratégies

Les marges comparent le meilleur diagnostic à trois arrêts ou plus au plan normal retenu. Une marge négative est un contre-exemple ; elle bloque la configuration avant toute évaluation réservée.

| Budgets I/II | Niveau favoris | Contrôles réussis | Marge minimale | Préférences de style abandonnées | Qualification complète |
|---|---:|---:|---:|---:|---|
| 8/16 | 0 | 90/90 | +3.78 s | 0 | Passe |

[Comparaisons par pilote](figures/pit-incentives-by-driver-v3.png) · [Reçus de qualification](results/training-qualification-v3.json).

## Plateau constant

Budgets discrétionnaires : **8 en I**, **16 en II**, identiques pour les neuf pilotes et indépendants du joueur. Niveau global des deux favoris : 0. Tous utilisent Balanced, engagement 0,8.

| Pilote | Limite | Régularité | Pneus | Points réservés | Rang moyen entre IA |
|---|---:|---:|---:|---:|---:|
| Charles L’Éclair | 0.87 | 0.83 | 0.83 | 2920 | 1.10 |
| Franz Hermann | 0.84 | 0.88 | 0.84 | 2234 | 1.92 |
| Smooth Operator | 0.79 | 0.86 | 0.82 | 1688 | 3.35 |
| Luis Amilton | 0.79 | 0.83 | 0.86 | 1433 | 4.15 |
| Pedro Gaseoso | 0.78 | 0.79 | 0.79 | 1116 | 5.35 |
| Isa Kadjar | 0.81 | 0.75 | 0.76 | 913 | 6.20 |
| Battery Voltas | 0.75 | 0.83 | 0.84 | 740 | 6.92 |
| Daniel Enchantier | 0.77 | 0.74 | 0.79 | 708 | 7.05 |
| Goat Tifi | 0.69 | 0.71 | 0.73 | 248 | 8.97 |

## Progression du joueur

Les budgets, composants et bonus sont ceux des snapshots de carrière, avec les commandes alors disponibles. Les rangs ci-dessous proviennent des conditions réservées, pas de nouveaux réglages ajustés sur leurs résultats.

| Étape | Préparation | Rang moyen | Victoires | Courses terminées |
|---|---|---:|---:|---:|
| era1-entry | fixed_mc | 10.00 | 0/36 | 36/36 |
| era1-entry | informed | 2.83 | 0/36 | 36/36 |
| era1-entry | routine | 10.00 | 0/36 | 36/36 |
| era1-entry | sampled_mc | 8.19 | 0/36 | 36/36 |
| era1-exit | fixed_mc | 10.00 | 0/36 | 36/36 |
| era1-exit | informed | 1.08 | 33/36 | 36/36 |
| era1-exit | routine | 10.00 | 0/36 | 36/36 |
| era1-exit | sampled_mc | 7.14 | 0/36 | 36/36 |
| era2-entry | fixed_mc | 10.00 | 0/72 | 72/72 |
| era2-entry | informed | 10.00 | 0/72 | 72/72 |
| era2-entry | routine | 10.00 | 0/72 | 72/72 |
| era2-entry | sampled_mc | 10.00 | 0/72 | 72/72 |
| era2-exit | fixed_mc | 9.51 | 0/72 | 72/72 |
| era2-exit | informed | 2.69 | 7/72 | 72/72 |
| era2-exit | routine | 9.58 | 0/72 | 72/72 |
| era2-exit | sampled_mc | 5.00 | 8/72 | 72/72 |

Hiérarchie des favoris : `True`. Progression : `True`. Toutes les préparations réservées terminent : `True`. Gain de rang regroupé du joueur informé face au concentré échantillonné : `3.56`.

5 assemblages natifs et 2 contrôles d’ordre/changement du joueur. Les traits effectivement exécutés correspondent aux ressources finales et leurs empreintes restent constantes.

Les traits exportés sont déjà développés avec le niveau global retenu. Ne pas leur réappliquer ce niveau lors de leur future intégration.

## Limites et conservation

Cette campagne n’établit pas un optimum global. Les réglages sont issus d’une recherche bornée, les budgets inférieurs sont dérivés par redimensionnement des préparations de base, et la grille de stratégies reste discrète. Les résultats indépendants ne comportent pas de trafic, aspiration, collision, VSC ni météo dynamique. Seules les situations authored Craft Engineering des ères I/II sont comparées. Les autres doctrines/ères, le jeu dans le navigateur et son admission vérifiée restent des étapes distinctes.

Les contre-épreuves à davantage d’arrêts sont réellement calculées avant acceptation en entraînement ; elles ne sont pas exclues pour masquer une meilleure stratégie. Les conditions réservées évaluent ensuite les plans scellés, sans réoptimisation des alternatives sur ces mêmes conditions.

Les entrées, résultats, empreintes et métriques par tour sont conservés ; pas de télémétrie point à point exportée. [Protocole et reproduction](README.md) · [Résumé JSON](results/summary-v3.json) · [Reçu de campagne](results/campaign-v3.json).

## Points à vérifier en jeu

La qualification porte sur les critères annoncés de sortie d’ère. À l’entrée de l’ère II, les quatre préparations du snapshot terminent P10 dans les 72 situations réservées chacune ; le joueur conserve alors ses composants d’entrée, dont l’ancien moteur. Le playtest doit vérifier que cette transition technologique est lisible et que les développements restent financièrement accessibles. Ce résultat ne valide pas à lui seul la difficulté de toute une carrière.

À la sortie de l’ère I, la préparation informée gagne 33/36 essais ; en II, 7/72 contre 8/72 pour la préparation moteur/châssis échantillonnée, mais son rang moyen reste meilleur (2,69 contre 5,00). Une recette concentrée peut donc gagner certains contextes. Ne pas transformer le critère de rang moyen en promesse de davantage de victoires partout.

[Hiérarchie](figures/field-hierarchy-v3.png) · [Progression](figures/player-progression-v3.png).
