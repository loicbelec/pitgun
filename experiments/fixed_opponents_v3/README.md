# Plateau fixe de neuf IA · campagne native V3

**Campagne terminée : candidat natif I/II qualifié. Aucun modèle jouable admis.** Voir [les résultats et limites](RESULTS_V3.md). Le protocole reste celui enregistré avant calcul ; aucune sélection n'a été retouchée après ouverture des données réservées.

Cette campagne reprend après [le transfert pneus validé](../tire_transfer_v1/RESULTS_V1.md), sous le même profil global `progressive4`, le même binaire de recherche et les mêmes équations. Elle ne modifie ni le jeu, ni les catalogues publiés, ni les services. Suivi : [#457](https://github.com/loicbelec/pitgun/issues/457), prérequis [#472](https://github.com/loicbelec/pitgun/issues/472).

## Identités et capacités constantes

Les neuf ressources de départ sont celles de [V2](../fixed_opponents_v2/roster-v2.json), pas ses anciens résultats de course. Toutes utilisent Balanced, engagement 0,8, comme le joueur de référence. Les trois traits restent identiques entre circuits, ères, seeds et températures.

| Pilote | Exploitation de la limite | Régularité | Gestion des pneus | Préférence de préparation |
|---|---:|---:|---:|---|
| Franz Hermann | 0.84 | 0.88 | 0.84 | balanced |
| Charles L’Éclair | 0.87 | 0.83 | 0.83 | power |
| Luis Amilton | 0.79 | 0.83 | 0.86 | tire_manager |
| Smooth Operator | 0.79 | 0.86 | 0.82 | smooth |
| Isa Kadjar | 0.81 | 0.75 | 0.76 | power |
| Pedro Gaseoso | 0.78 | 0.79 | 0.79 | balanced |
| Battery Voltas | 0.75 | 0.83 | 0.84 | tire_manager |
| Daniel Enchantier | 0.77 | 0.74 | 0.79 | mechanical |
| Goat Tifi | 0.69 | 0.71 | 0.73 | balanced |

Franz Hermann et Charles L’Éclair doivent être les deux meilleurs au score agrégé, sans résultat individuel imposé. Le protocole teste trois niveaux globaux pour ces deux favoris : traits de base, +0,02 ou +0,04 sur chacun des trois traits, plafonnés à 0,99. Un seul niveau est choisi sur entraînement, puis les ressources finales sont développées une fois et figées. Ne pas réappliquer ce niveau lors d’une future admission de ces ressources.

## Préparation et stratégie

Le [plan](plan-v3.json) couvre dix couples circuit/ère, sept circuits distincts, dans les ères I/II. La base véhicule reste `classic_v8_1960`. Les adversaires partagent le moteur de base en I, puis le moteur `v8_1970` et l’aéro basic en II ; les composants du joueur restent ceux de ses vrais snapshots de carrière. Les budgets discrétionnaires candidats sont 6/7/8 en I et 12/14/16 en II, constants par ère pour les neuf pilotes, séparés des offsets HQ. Aucun ajustement aux résultats ou dépenses du joueur.

Les réglages et stratégies de référence du transfert sont réutilisés par identité exacte. On croise ensuite les trois meilleurs réglages distincts avec trois plans distincts pour chaque pilote et contexte, puis on explore leurs voisins à un point et les dates de stand voisines. Les préférences fixes distinguent les pilotes parmi les solutions proches ; les favoris prennent le meilleur temps échantillonné.

Les 18 préparations authored de la doctrine Craft Engineering couvrent entrée/sortie des deux ères : budgets 8→11 et 11→16, bonus et composants réels, commandes verrouillées respectées. On compare une recette uniforme, une recette moteur/châssis 50:50, une recette concentrée échantillonnée et une préparation informée. Ces deux dernières reçoivent les mêmes deux tours de recherche locale, sous leurs contraintes respectives. À la sortie des ères, leurs réglages finaux passent ensuite les 114 plans à un/deux arrêts.

## Sélection et contre-épreuves

Sur entraînement, le joueur informé doit gagner au moins un GP de sortie par ère, avoir une médiane de rang au plus égale à trois et améliorer le temps moyen par rapport à la recette uniforme. Son rang moyen ne doit pas être moins bon que celui de la recette concentrée dans chaque ère ; le gain moyen regroupé doit atteindre 0,5 position. La recette fixe 50:50/MH ne doit pas gagner tous les GP de sortie d’une ère. Une bonne recette concentrée peut rester légitime sur certains circuits, particulièrement avec les bonus de refroidissement déjà acquis en II. Aucun critère n’impose une dépense sur les quatre axes.

Parmi les configurations approximativement éligibles, garder au plus trois couples de budgets distincts, dans l’ordre de difficulté déclaré, avec le plus petit niveau de favoris éligible par couple. Avant toute ouverture des seeds réservés, chaque configuration tentée reçoit, sous les vrais traits de chacun des neuf pilotes :

- **114 plans à un/deux arrêts et 84 diagnostics à 3–12 arrêts par pilote et contexte**, soit 17 820 demandes de famille par tentative, plus les plans locaux déjà retenus lorsqu’ils sont absents de cette grille ;
- une nouvelle sélection des stratégies normales et une nouvelle vérification de la hiérarchie et de la progression ;
- le rejet de la configuration si les arrêts supplémentaires restent avantageux pour un pilote. Une préférence de style peut céder au meilleur plan normal si nécessaire, sans correction de chrono.

La physique n’est pas modifiée pour faire passer ce protocole. Tous les échecs et toutes les configurations rejetées restent dans les reçus. Si aucune des trois tentatives ne passe, le résultat est REFINE et les données réservées restent fermées. Le protocole V3 précédent, non lancé, a été révisé avant calcul pour intégrer le transfert terminé et cette vérification des vrais traits ; aucune validation réservée n’a été consultée pour ce changement.

## Évaluation indépendante

Une politique qualifiée sur entraînement est scellée avant les quatre seeds et les trois environnements réservés : environnement hérité, air/route 20/30 °C, puis 40/50 °C. Ses traits, budgets, réglages et plans ne changent plus après ouverture. Les 120 champs de neuf adversaires sont comparés aux préparations de carrière enregistrées. Les plans ne sont pas réoptimisés sur les conditions réservées.

Contrôles supplémentaires : 18 répétitions fraîches des pilotes avant calibration des budgets, répétitions exactes des champs réservés, cinq assemblages natifs de dix voitures indépendantes, invariance à l’ordre du plateau et à un changement de réglage du joueur dans les deux ères. Il n’y a pas encore de trafic, aspiration, collision, VSC ni météo dynamique. Une évaluation réservée défavorable reste un résultat négatif, sans retouche après coup. La qualification des autres doctrines/ères et l’admission WASM/Authority/Verifier sont des étapes distinctes.

## Mémoire, traces et conservation

Les grosses comparaisons de stratégies sont découpées par contexte : environ 1 800 cas par phase, sans accumulation des dix blocs en mémoire. Seuls les résultats retenus sont gardés entre ces blocs. Le journal des exécutions natives distingue les vrais recalculs des lectures du cache, y compris après reprise, les répétitions forcées et les assemblages natifs. Aucun chiffre de performance navigateur/VPS n’est déduit de cette campagne concurrente.

Les phases terminées sont épinglées par empreinte. Le protocole, les sources effectives, les ressources, le binaire et la séparation entraînement/réservé sont enregistrés. Les sorties conservent résultats et métriques par tour, sans télémétrie point à point. La politique de conservation est décrite dans [le transfert](../tire_transfer_v1/README.md) ; le catalogue de runtime ne distribue pas cette campagne aux joueurs.

```sh
python3 experiments/fixed_opponents_v3/check_protocol.py
python3 experiments/fixed_opponents_v3/run_local.py --preview
python3 experiments/fixed_opponents_v3/run_local.py BINAIRE_NATIF_FIGE CACHE_KERNEL
```

Ne pas modifier common.py, run_local.py, plan-v3.json, les sources physiques, les ressources ou le binaire pendant le calcul. Une reprise conserve les phases terminées et leur identité. Les anciennes campagnes V1/V2 et le diagnostic des stands restent disponibles dans leurs dossiers et branches.

## Restitution après achèvement

La campagne complète représente environ 269 Mio de reçus. Les synthèses, le protocole, la politique scellée et le [manifeste d'archive](evidence-archive-v3.json) restent dans Git. Les reçus compressés, le journal de toutes les exécutions et le binaire macOS exact sont conservés dans une archive de recherche distincte, avec empreinte SHA-256 et empreintes de chacun des 34 fichiers. Aucun fichier de télémétrie point à point n'est ajouté.

L'archive est attachée à la **release de recherche en brouillon** `research-fixed-opponents-v3-20261123` du dépôt `loicbelec/pitgun`. Elle demande donc un accès mainteneur authentifié tant qu'elle reste en brouillon ; sa publication n'est pas une admission de modèle. Garder ce statut visible et prévoir l'accès public aux preuves avant toute annonce publique de qualification. La copie locale reste également conservée ; le cache n'est pas l'unique exemplaire.

Pour contrôler les preuves depuis un nouveau checkout, télécharger l'archive avec `gh release download research-fixed-opponents-v3-20261123 --repo loicbelec/pitgun --pattern fixed-opponents-v3-evidence-2026-11-23.tar --dir DOSSIER_TEMPORAIRE`, puis comparer son SHA-256 au manifeste avant extraction dans ce dossier temporaire. Copier ensuite les fichiers `results/*-v3.json.gz` et `results/native-executions-v3.jsonl` extraits dans ce dossier d'expérience. Les JSON compacts sont déjà dans Git. `report.py` contrôle alors les empreintes du reçu final avant toute restitution. Le binaire original extrait sous `native/` sert aux reproductions exactes sur une machine compatible ; un binaire reconstruit ou destiné à un autre système doit recevoir sa propre qualification, sans substitution silencieuse.

`report.py` contrôle les empreintes du reçu final, des sources et des résultats avant de produire `RESULTS_V3.md` et `results/summary-v3.json`. Il restitue les configurations rejetées et leurs marges, les traits réellement exécutés, les résultats réservés et la progression du joueur. Il ne choisit aucun nouveau réglage. `plot_report.py` produit ensuite des figures PNG/SVG à partir de ces mêmes données. Ces commandes exigent une campagne terminée ; elles ne transforment pas une phase encore en cours en résultat validé.

```sh
python3 experiments/fixed_opponents_v3/report.py
python3 experiments/fixed_opponents_v3/plot_report.py
```

Le statut `COMPLETE_NATIVE_EVALUATION` signifie que le protocole est terminé. Le rapport détermine séparément si les critères réservés passent (`NATIVE_I_II_CANDIDATE`) ou échouent (`REFINE_NO_ADMISSION`). Une campagne rejetée avant le scellement conserve `REFINE_TRAINING_NO_HOLDOUT`, sans consultation des seeds réservés.

## Futur raccordement du plateau

L'export candidat, lorsqu'il existe, reste explicitement non admis. Un futur raccordement devra conserver ensemble les ressources de pilotes avec leurs traits finaux déjà développés, le budget fixe de chaque ère, les composants et offsets distincts des dépenses, ainsi que les réglages et plans par contexte. Il devra aussi conserver les namespaces aléatoires `fixed-field:<driver_resource_id>` et `career-driver:player`, et compiler le même profil physique pour les trois gommes, au départ comme aux arrêts.

La validation native n'autorise pas à déduire une identité de moteur du seul numéro d'ère ou du véhicule de base, à doubler le bonus de traits des favoris, ni à substituer l'ancien pneu lors d'un arrêt. L'identité du modèle et du SimPack, le coût et la parité WASM, les commandes en course et le replay Authority/Verifier restent à qualifier avant publication dans les catalogues et utilisation par le jeu.
