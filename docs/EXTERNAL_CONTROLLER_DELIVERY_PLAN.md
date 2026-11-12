# Contrôleurs externes — trajectoire validée

## Séquence prioritaire — 21 septembre 2026

Le [plan détaillé en trois livraisons](https://github.com/loicbelec/pitgun-game/blob/docs/progressive-simulation-plan/docs/design/PROGRESSIVE_SIMULATION_DELIVERY_PLAN.md) est soumis à validation :
**départ anticipé avec tampon → frontière de décision réelle → Gemini et messages contextuels**.
Le tampon (game [#273](https://github.com/loicbelec/pitgun-game/issues/273)) et la reprise
([#274](https://github.com/loicbelec/pitgun-game/issues/274)) précèdent désormais la
frontière [framework #415](https://github.com/loicbelec/pitgun/issues/415).
Puis [#253](https://github.com/loicbelec/pitgun-game/issues/253) apporte les actions et
[#275](https://github.com/loicbelec/pitgun-game/issues/275) la cadence des conseils dans
le même Debrief, rappelés dans le modal de résultat. Aucun badge de mode supplémentaire.

Cette séquence remplace les anciennes mentions de première livraison ci-dessous ;
les garanties des contrôleurs et les étapes benchmark/BYO restent valides ensuite.
La préparation du tampon conserve la physique actuelle. Les audits et manifests
conditionnent les conclusions de benchmark, pas le prototype local. La preuve
batterie précède le gel public ; les workers hébergés ne bloquent pas un rapport local.
La recette staging #256/#271 reste distincte. Tous ces lots sont en Backlog ; aucun
contrôleur, migration ou déploiement n'est livré par cette mise à jour documentaire.


Mise à jour du 20 septembre : la validation locale est acquise. Le [plan compétitif](COMPETITIVE_RACING_AND_ENGINEER_PLAN.md)
et le [backlog lié](COMPETITIVE_BACKLOG_2026-09.md) ajoutent les audits physiques,
les règles de comparaison et le protocole d'évaluation avant publication. Les
quatre livraisons Engineer ci-dessous restent valides ; les mentions historiques
de playtest préalable ne constituent plus un blocage. Aucun contrôleur n'est
livré par cette mise à jour documentaire.

> 17 septembre 2026 : le propriétaire valide le gameplay saisonnier et autorise son intégration locale dans le jeu. La durée cible et l’idle sont retirés des critères. Les replays du navigateur restent un confort borné ; les artefacts complets du futur benchmark appartiennent à la plateforme. Cette décision ne livre aucune commande de contrôleur ni modification physique.

16 septembre 2026. Décision produit validée par Loïc ; **planification, pas implémentation**. Epic transverse : [game #161](https://github.com/loicbelec/pitgun-game/issues/161). La validation de l'économie saisonnière par le propriétaire précède le démarrage du prochain chantier. Le présent document remplace l'ordre de livraison de la [proposition initiale](BRING_YOUR_OWN_ENGINEER_PROPOSAL.md), qui demeure une analyse historique utile.

## Premier résultat attendu

Une course dont Gemini décide certains arrêts et choix de pneus pendant le calcul, avec observations bornées, décisions appliquées par Racing, archive rejouable sans inférence et comparaison à une stratégie de référence. Le moteur reste déterministe une fois les décisions externes fixées. Le LLM n'est ni un solveur physique ni une source d'autorité.

L'ordre retenu est **frontière de décision → Gemini en course → benchmark local → BYO ouvert**. Un petit démonstrateur de batterie éprouve la frontière commune avant de figer son contrat public. La stratégie avant-course du plan initial peut servir de référence, mais n'est plus un lot produit préalable obligatoire. Aucun changement d'agressivité, de setup ou de paramètres physiques n'entre implicitement dans le premier périmètre pneus/stands.

## Existant à réutiliser, limite à franchir

- Les [sessions incrémentales Racing](RACING_INCREMENTAL_SESSION_ENGINE_V1.md) et leur [API WASM pull](RACING_WASM_PULL_SESSION_V1.md) conservent l'état entre les tours. Le host peut attendre avant le prochain pull sans modifier le temps simulé.
- Les [instructions déterministes](RACING_DETERMINISTIC_DRIVER_INSTRUCTIONS_V1.md) et l'autorisation existante lient une timeline de modes à des preuves. **La timeline est actuellement connue au départ** ; une API nommée `dynamic` n'implique pas l'acceptation de commandes nouvelles pendant la course.
- Pneus et stands disposent de plans validés avant le départ. Modifier ce plan pendant l'exécution nécessite une sémantique Racing et une autorisation propres : le contrat des modes de pilotage ne suffit pas.
- Réutiliser identité de modèle/catalogue/input, journal canonique, Authority, Verifier et bundle de preuves. Le débrief Gemini du jeu fournit un précédent de transport, quotas et enregistrement ; son schéma de prose ne devient pas un schéma d'action.

Le moteur doit s'arrêter **avant** la partie de simulation affectée par la décision. Arrêter le curseur d'un replay dont la course est déjà calculée ne constitue pas une interaction avec la simulation.

## Responsabilités

| Couche | Possède | Ne possède pas |
| --- | --- | --- |
| Framework commun | Identités, ordre causal, enveloppe d'observation/proposition, hooks de validation, journal et replay | Tour, pneu, voiture, MW, stratégie fournisseur |
| Domaine Racing | Frontières de tour, observations autorisées, règles de pit/pneu, transition d'état et effets physiques | HTTP, Gemini, quota commercial, chronomètre réseau |
| Domaine énergie | Pas de conduite, bilan énergétique, limites de stockage et consignes de batterie | Types ou dépendances Racing |
| Host / runner | Cycle attendre/recevoir/reprendre, fournisseur, délais réels, quotas, authentification et persistance | Réinterprétation physique ou résultat fabriqué |
| Jeu / Lab | Délégation choisie, présentation, challenge, comparaison et progression HQ | Pouvoir supplémentaire sur la simulation ou secrets BYO |

Commencer avec la plus petite interface permettant un chemin Racing réel ; ne pas créer un framework universel d'agents ni déplacer les concepts métier dans le core. Éprouver puis extraire la partie effectivement partagée avec la batterie.

## Découpage et acceptation

| Livraison | Ticket | Dépendance de livraison |
| --- | --- | --- |
| Frontière et commandes Racing | [framework #415](https://github.com/loicbelec/pitgun/issues/415) | Gameplay accepté ; intégration locale du jeu avant démarrage |
| Gemini en course | [game #253](https://github.com/loicbelec/pitgun-game/issues/253) | #415 |
| Benchmark local | [game #254](https://github.com/loicbelec/pitgun-game/issues/254) | #415 et game #253 |
| Preuve batterie | [framework #416](https://github.com/loicbelec/pitgun/issues/416) | Première boucle Racing ; à éprouver après le premier chemin Gemini |
| BYO public | [framework #417](https://github.com/loicbelec/pitgun/issues/417) | #415, #416 et game #254 |

Tous restent en Backlog lors de cette mise à jour. L'epic #161 demeure ouvert ; ces liens et critères ne constituent pas une livraison fonctionnelle.

### 1. Frontière déterministe et commandes Racing

Créer une observation figée après un tour résolu, suspendre le prochain calcul et accepter une proposition bornée pour le concurrent délégué. Commencer par `keep` et une demande de pit avec pneu autorisé à une frontière future explicite. Les noms sont illustratifs jusqu'au schéma exécutable.

Après `k` tours terminés, une demande ne peut pas modifier ces tours. La première convention candidate place le pit à la fin du tour à venir, après `k+1` tours ; documenter indices, dernier tour, coûts de service, changement de pneu et plan restant. L'acceptation finale doit montrer que cette convention correspond réellement à la transition du Solver. Définir une seule proposition finale par frontière, son accusé d'acceptation et les cas de doublon/conflit.

Un contrôleur déterministe de référence exerce d'abord ce même chemin. Les tests doivent démontrer une différence physique attendue avec/sans commande, la conservation de l'état, aucune recomputation cachée du passé, le refus des actions rétroactives/interdites et le replay exact dans le runtime épinglé. Des cadences de pull différentes ne changent pas le résultat pour le même journal appliqué.

Faire évoluer explicitement le contrat initial d'autorisation et la finalisation des preuves : permissions et état initial au départ, journal réellement appliqué lié au résultat final. Préserver les anciens contrats batch et timelines préprogrammées. Une preuve locale n'est pas une acceptation Hosted ; les nouveaux schémas, bindings native/WASM et vérifications nécessitent leurs vecteurs de conformité.

### 2. Gemini décide en course

Le jeu/Lab propose une délégation explicite, active uniquement pour les actions effectivement disponibles. Réutiliser l'adaptateur Gemini Flash Lite local avec modèle/configuration épinglés, sans changement automatique de fournisseur. Archiver le payload exact, la réponse, les coûts et chaque proposition. Un refus ou timeout garde le plan existant si cette action de repli est légale et déclarée ; sinon terminer proprement selon le contrat. Ne pas présenter un repli comme une décision Gemini.

Le temps réseau ne s'ajoute pas au chrono de course. Il peut déterminer qu'un timeout a été retenu ; cette sélection et son motif sont donc archivés. Le replay applique la sélection enregistrée, sans reproduire les délais ni contacter le fournisseur. Une nouvelle inférence est une nouvelle expérience.

Ne pas appeler le modèle à chaque pas du solveur. Fixer les fenêtres et plafonds d'appels dans le scénario de test ; les vérifier contre le quota réellement disponible lors de l'implémentation. Mistral/Qwen et le Mac mini sont des adaptateurs ultérieurs, sans prérequis pour cette livraison.

### 3. Benchmark local reproductible

Figer circuit, voiture/composants, adversaires, distance, modèle/catalogue/runtime, conditions, observations disponibles, actions et budgets. Choisir une distance du catalogue ayant passé le précontrôle physique ; une fixture courte de protocole ne vaut pas benchmark de Grand Prix.

Comparer plan fixe, politique déterministe et Gemini sur un ensemble de seeds publié avec ses règles avant l'évaluation. Conserver toutes les tentatives, y compris erreurs, DNF, invalides et replis. Pour les jeux de tests tenus secrets, archiver les seeds et les publier après clôture ; ne pas exposer l'état RNG au contrôleur. Classer d'abord selon le statut d'arrivée puis le temps total, en rendant visibles coût, latence, taux d'actions valides et delta à la référence. Définir DNF/égalités avant de lancer les essais. Les budgets d'information et d'appels au simulateur sont identiques.

Le premier rapport est local. Un leaderboard public demande vérification gouvernée, stockage durable des preuves et règles de publication ; le cache de replays de carrière ne suffit pas. Comparer des Engineers mesure aussi prompts, mémoire et politiques ; comparer des modèles seuls exige un harness fixe. Aucune supériorité du LLM n'est présupposée.

### Preuve multi-domaine avant stabilisation publique

Une **batterie sur un bilan production/demande synthétique**, avec puissance de charge/décharge bornée, état de charge, rendement, capacité et réserve finale. Les profils exogènes réalisés, générateurs et prévisions autorisées sont versionnés et archivés séparément. Pas de visibilité sur des valeurs futures non autorisées. Un seed seul ne remplace pas ces données.

Comparer au minimum une politique fixe et une heuristique via la même boucle de host, puis rejouer les actions. Une consigne illégale est rejetée ; une mauvaise consigne légale conserve ses conséquences. Définir les métriques d'énergie non servie/écrêtée et la réserve terminale pour éviter un gain artificiel au dernier pas.

Le test échoue si le host commun dépend de crates Racing ou de noms de tours/pneus. Ce démonstrateur prouve un second domaine énergie, **pas un réseau électrique** : pas de flux sur branches, tension, fréquence ou modèle RTE. Il ne nécessite ni solveur réseau complet ni données opérationnelles. Pod/Drone et l'ère VII restent une trajectoire distincte ; ils ne sont plus un préalable à cette preuve.

### 4. BYO ouvert

Pitgun expose des jobs d'observation et reçoit des propositions JSON par polling HTTPS. Le client du participant, derrière NAT si nécessaire, appelle Pitgun ; il peut être écrit dans n'importe quel langage. Pitgun conserve l'exécution officielle. Pas de callback vers une URL arbitraire, d'exécution de code fourni ni de chargement automatique de poids utilisateur.

Documenter JSON Schema, exemples, erreurs, authentification limitée à un run/contrôleur, expiration, idempotence, concurrence, quotas et reprise. Les endpoints et versions exacts ne sont pas figés par ce plan. Les secrets restent chez leur propriétaire. Un modèle BYO est déclaré par le participant, sans prétendre attester son identité. Les adaptateurs internes de modèles Lab utilisent la même frontière avec destinations configurées.

Le mode bac à sable local et le mode classé restent distincts. Pour une publication classée, la session officielle contrôle les observations divulguées sans confier son état futur au runner participant ; la chaîne Authority/Verifier vérifie les artefacts. Cela ne garantit pas l'absence de calcul auxiliaire chez le participant. Le replay prouve les effets du journal ; il ne prouve pas à lui seul la provenance d'une inférence ni l'absence d'assistance humaine.

## Contrat minimal à formaliser

| Élément | Contenu à lier et vérifier |
| --- | --- |
| Observation | Version, domaine, run, décision séquentielle, frontière logique, empreinte des capacités et de l'état visible, données métier avec unités/absences |
| Proposition | Run/décision/observation exacts, action métier bornée, identité de requête idempotente ; explication facultative sans pouvoir exécutable |
| Validation | Action déléguée, bon concurrent, fenêtre encore ouverte, limites, absence de conflit et légalité métier |
| Application | Frontière canonique effective, événement accepté et lien au journal ; refus/repli séparés de l'événement physique |
| Archive | État initial, versions et artefacts, seed et entrées exogènes, contrôleur/modèle/config/prompt, observations, propositions, refus/replis, actions appliquées et résultat |

L'observation Racing initiale se limite à l'état du joueur disponible au point de décision. Les adversaires avancent aujourd'hui à une frontière commune de tours, pas nécessairement au même temps physique : une comparaison adverse ultérieure doit préciser son horloge et exclure les informations futures. Le runtime officiel est épinglé ; la conformité portable native/WASM ne doit pas être annoncée comme égalité exacte de tous les flottants V3.

## Priorités et portée de cette mise à jour

Le propriétaire valide actuellement l'économie locale ([game #252](https://github.com/loicbelec/pitgun-game/issues/252)). Le démarrage du contrôleur suit l'acceptation du gameplay et de son intégration locale, pas une clôture artificielle de tous les sujets historiques. [Monaco #414](https://github.com/loicbelec/pitgun/issues/414) reste reporté : examiner les protections de calibrage existantes à sa reprise. Ce défaut limite la couverture du benchmark ; il ne justifie ni raccourcissement silencieux ni blocage de toute conception indépendante.

Cette mise à jour ne livre aucun contrôleur, schéma public, endpoint ou nouveau domaine, ne modifie aucun coefficient et n'autorise aucun déploiement. Le document compagnon du jeu est `game/docs/design/BYO_ENGINEER_DELIVERY_PLAN.md` dans le workspace ; l'[epic #161](https://github.com/loicbelec/pitgun-game/issues/161) tient la hiérarchie transverse.
