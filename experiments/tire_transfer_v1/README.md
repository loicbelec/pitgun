# Transfert du candidat pneus aux dix contextes I/II

Étape physique préalable au plateau fixe de neuf pilotes, suivie par
[#472](https://github.com/loicbelec/pitgun/issues/472) et
[#457](https://github.com/loicbelec/pitgun/issues/457), dans
[la PR native #473](https://github.com/loicbelec/pitgun/pull/473).

**Protocole enregistré avant calcul ; aucun nouveau profil ni plateau sélectionné.**
Le candidat global `progressive4` provient du
[premier écran physique](../tire_performance_v1/RESULTS_V1.md). Ses coefficients,
le coût des stands, les équations et les tolérances ne changent pas ici.

## Question et méthode

Vérifier que les plans à un ou deux arrêts restent meilleurs que les diagnostics
à davantage d'arrêts sur d'autres circuits et réglages. Une restriction de plans
ne doit pas dissimuler une incitation physique aux arrêts répétés.

Le [plan figé](plan-v1.json) couvre trois contextes d'ère I et sept d'ère II.
Les contextes partagent le même pilote de référence Balanced ; les composants et
offsets sont explicites. Le budget discrétionnaire est constant par ère, 8 ou 16,
et n'est pas adapté au joueur. Aucun des neuf adversaires n'est ajusté ici.

1. Calculer **1 551 GP de réglages**, avec le même plan medium/hard à mi-course.
   Inclure les répartitions moteur/châssis et des répartitions avec cooling/aéro,
   ainsi que plusieurs positions de curseurs.
2. Explorer les voisins à un point et les curseurs voisins des deux meilleurs
   réglages par contexte. Les voisins restent dans le même budget.
3. Figer deux réglages distincts par contexte avant les nouvelles comparaisons :
   le meilleur échantillonné et le meilleur concentré moteur/châssis. S'ils sont
   identiques, prendre une alternative avec des points discrétionnaires de cooling.
   Les bonus HQ restent présents même quand le cooling discrétionnaire vaut zéro.
4. Comparer, à chacun de ces **20 réglages**, les **114 plans à un/deux arrêts**
   aux **84 diagnostics à 3–12 arrêts** : **3 960 demandes avant déduplication**.
5. Recalculer exactement les deux meilleurs plans comparés à chaque réglage,
   soit **40 répétitions fraîches**, et vérifier les bilans et la continuité.

L'échantillonnage est déterministe et explicite ; ce n'est pas une recherche
exhaustive de tous les réglages, séquences et dates d'arrêt possibles. Le plan
initial utilise deux composés distincts conformément au protocole de préparation
de course existant. Le champ diagnostic permet bien de calculer plus de deux
arrêts dans le même solveur. Il n'ajoute aucune restriction au jeu actuel.

## Critères et frontières

Toute marge négative `temps_3+ - temps_1/2` constitue un contre-exemple conservé.
Le résultat est alors `REFINE`, sans changement automatique des coefficients ni
qualification du plateau. Aucun échec n'est supprimé pour sélectionner un résultat.
Le coût de stand réellement résolu est vérifié dans chaque bilan de temps.

Les seeds et températures réservés du futur plateau restent fermés. Il s'agit
encore d'entraînement et de transfert natif sous environnement hérité, pas d'une
validation indépendante du plateau final. Pas de trafic, aspiration, collision,
météo dynamique, promotion de catalogue, WASM, jeu ou déploiement dans cette étape.

## Conservation des données de calibration

Les données sont utiles pour expliquer **pourquoi** une version a été retenue,
rejouer les calculs, diagnostiquer une régression et comparer un futur modèle.
Conserver les échecs et les variantes rejetées évite aussi de refaire les mêmes
choix invalides. Une bonne performance dans ces essais n'identifie pas à elle
seule les paramètres d'un pneu réel.

| Ensemble | Contenu conservé | Destination |
|---|---|---|
| Ressources exécutables | Paramètres retenus, modèle, packs/policies nécessaires, identités et digests | Future version immuable du catalogue et artefacts de runtime correspondants |
| Preuves de calibration | Protocole, version du code, identité du binaire/outillage, ressources, entrées/seed, résultats synthétiques, temps et métriques par tour, échecs, sélection et répétitions | Ce dossier d'expérience, reçus JSON compressés et rapport |
| Calculs de travail | Cache exact par entrée et fichiers temporaires | Cache local régénérable, distinct des preuves versionnées |
| Télémétrie détaillée | Seulement si nécessaire à une anomalie précisément identifiée | Contre-exemple ciblé ; aucun export systématique point par point |

Le catalogue de runtime n'est pas un entrepôt de campagnes. Sa future publication
peut référencer le rapport et ses empreintes, sans télécharger ces données chez
chaque joueur. Une nouvelle physique doit recevoir une nouvelle identité et des
ressources cohérentes côté navigateur et vérification ; changer les coefficients
sous une ancienne version empêcherait d'interpréter correctement les résultats.
Les versions historiques restent disponibles selon la politique de rétention des
modèles, indépendamment du cache temporaire de cette campagne.

Le même kernel et le même cache que l'écran pneus permettent de réutiliser les
entrées strictement identiques, avec un décompte séparé des nouvelles entrées et
des réutilisations. Les répétitions exactes sont de vrais nouveaux calculs.
Les phases terminées portent leur empreinte et peuvent être reprises sans être
réécrites ; une modification de source/protocole bloque la réutilisation.

## Reproduction

```sh
python3 experiments/tire_transfer_v1/run_local.py --preview
python3 experiments/tire_transfer_v1/run_local.py CHEMIN_BINAIRE_TIRE_PERFORMANCE CHEMIN_CACHE_KERNEL
```

Utiliser le binaire déjà qualifié ou le reconstruire depuis les sources et
ressources figées dans les reçus du premier écran. Ne pas modifier/recompiler le
kernel, les sources Rust ou ses ressources pendant le calcul. Les reçus épinglent
le code effectif en plus du commit ; une reconstruction d'un autre binaire exige
sa propre qualification/cache, pas une substitution silencieuse.
