# Bring Your Own Engineer — proposition d’architecture

> **Mise à jour validée le 16 septembre 2026 :** l'ordre ci-dessous est historique.
> La [trajectoire actuelle](EXTERNAL_CONTROLLER_DELIVERY_PLAN.md) commence par les
> décisions pneus/stands pendant la simulation, puis Gemini, benchmark local et
> BYO ouvert. Une batterie synthétique éprouve le contrat commun avant sa
> stabilisation publique ; Pod/Drone n'est plus le préalable au second domaine.
> Les paragraphes suivants conservent l'analyse et les hypothèses du 12 septembre,
> notamment le MVP avant-course et ses paramètres fournisseur. Ils ne décrivent
> ni l'état livré aujourd'hui ni le prochain ordre d'implémentation.

Proposition du 12 septembre 2026, sans implémentation ni modification des contrats publiés.
Analyse des checkouts locaux : framework `5550c79`, game `34beec5`, site `14343a0`.
Les capacités présentes dans le code ne constituent pas une vérification de leur déploiement en production.

Séquence de livraison validée par le porteur du projet : (1) challenge et baselines,
(2) Gemini avant-course, (3) contrôle live, (4) BYO ouvert, puis campagnes de benchmark.
Les lots détaillés plus bas décomposent cette séquence. La piste multi-domaine de la
section 9 est une proposition complémentaire, pas une implémentation autorisée du domaine Grid.

## Décision recommandée

Faire de l’Engineer un contrôleur externe non fiable : il reçoit une observation bornée, propose une décision JSON, puis Racing valide et applique cette décision. Le LLM reste hors du noyau déterministe.

Commencer par un **MVP de stratégie avant-course**, avec Gemini Flash-Lite : un appel produit un vrai plan de relais, d’arrêts et de changements de mode, que le moteur exécute. Livrer ensuite l’adaptation en course à des frontières de tours. Cette séparation permet une première expérience utile sans présenter le runtime actuel comme déjà capable de recevoir des commandes en cours d’exécution.

Pour le BYO classé, **Pitgun expose une API et le programme du participant vient chercher les observations**. Le participant n’a pas à ouvrir un serveur public. Pitgun n’appelle aucune URL fournie par un joueur et n’exécute aucun code utilisateur. Un runner contrôlé par Pitgun possède la session officielle ; le participant possède son modèle, son framework et ses secrets.

Le résultat est une compétition d’Engineers — modèle, prompt, mémoire et politique de décision — avant d’être un classement scientifique de modèles seuls. Un leaderboard de modèles exige en plus un harness commun et une provenance contrôlée.

## 1. Existant vérifié et réutilisation

| Élément | Constat dans le dépôt | Conséquence |
| --- | --- | --- |
| Runtime et contrats | Identités de modèle, input, Simulation Pack, RNG versionné, seed, JCS/SHA-256, distinction `execution_id` / `run_id` | Conserver ces primitives ; ne pas inventer une identité fondée sur le nom du LLM |
| Racing incrémental | `IncrementalRacingSession` et sessions Solver conservent les états physiques et avancent par tour ; API pull WASM | Frontière naturelle pour une boucle observation/décision ; aucun appel réseau dans `advance` |
| Instructions | Timeline `manage` / `balanced` / `attack`, validation, profil de catalogue, enveloppe Authority, historique dans l’input final | Réutiliser la sémantique et l’acceptation des modes |
| Limite du « dynamic » actuel | `start_authorized_dynamic_racing_session` reçoit déjà `completed_input`, dérive le contrat final avant exécution et extrait toute la timeline | « Dynamic » désigne actuellement l’identité des instructions ; il manque l’injection de décisions live et la finalisation différée |
| Pneus et arrêts | `CompetitorStintStrategy`, validation des relais, résolution du plan pneumatique et du `PitPlan` avant exécution | MVP de planification possible ; changement live du plan à ajouter, avec preuve et autorisation dédiées |
| Vérification hébergée | Authority signe l’essai ; Verifier accepte `/v1/verifications/racing/attempts` et rejoue la timeline ; backend conserve les soumissions exactes, nonces et verdicts | Réutiliser la chaîne de confiance et l’idempotence |
| Leaderboard du jeu | Projection de résultats vérifiés, meilleur tour par carrière/circuit/ère | Réutiliser la discipline de publication ; créer une projection benchmark séparée fondée sur la course complète |
| Pit Wall | Lecture progressive, télémétrie, comparaison de tours, outbox et proposition de bulle Engineer | Réutiliser ces surfaces ; ne jamais confondre curseur de lecture et frontière de calcul |
| Lab / site | Landing centrée sur le framework et Racing ; pas de plateforme BYO repérée dans les sources inspectées | Ajouter ensuite une vitrine de challenge et de preuves, sans reconstruire le jeu |
| API de registre | Client Gemini PHP pour AST / canaux dans `api/src/Services/LlmClient.php` | Précédent d’intégration, mais responsabilité distincte : le registre n’a pas à orchestrer les courses |

Références principales : [contrat déterministe](DETERMINISTIC_RUN_CONTRACT_V1.md), [bundle V1](RUN_BUNDLE_V1.md), [sessions incrémentales](RACING_INCREMENTAL_SESSION_ENGINE_V1.md), [pull WASM](RACING_WASM_PULL_SESSION_V1.md), [instructions](RACING_DETERMINISTIC_DRIVER_INSTRUCTIONS_V1.md), [code de session](../crates/pitgun-racing-simulator/src/lib.rs), [contrats Racing](../crates/pitgun-racing-contract/src/race.rs), [Verifier](../services/pitgun-verifier/README.md).

Sources du workspace adjacent : `game/src/verification/hostedExecution.ts` crée aujourd’hui une timeline vide ; `game/src/app/pitwallSessionController.ts` sépare calcul et lecture ; `game/services/leaderboard-api/README.md` décrit la projection vérifiée ; `game/docs/design/PITWALL_ENGINEER_ADVICE.md` propose déjà une bulle liée aux preuves. Le WASM embarqué annonce le commit framework `68a0c805…`, distinct du checkout inspecté : la synchronisation doit faire partie des critères de livraison.

Certains documents historiques disent encore que le Verifier dynamique reste à implémenter ; le code du service et son README décrivent désormais cette route. Ne pas construire le découpage à partir des seuls statuts historiques.

Deux limites scientifiques sont à préserver dans la communication :

- La fixture stable V1 dispose de garanties natives/WASM publiées. Pour les flux V3, le contrat incrémental distingue l’égalité exacte dans un runtime de la projection de conformité portable : il ne garantit pas l’égalité de tous les flottants de télémétrie. Le benchmark doit épingler son runtime officiel et tester séparément les observations exposées aux modèles.
- Le contrat d’instructions explique qu’un mode `attack` constant a dominé des essais antérieurs. Avant de proclamer un benchmark stratégique, vérifier que les coûts persistants des décisions créent effectivement des compromis. Le modèle courant ne doit pas être présenté comme une simulation complète du trafic, du dépassement ou de la défense en piste.

## 2. Architecture et frontières de confiance

```text
Pit Wall / page Lab ── crée et observe l’essai ──> API Engineer
                                                     │
                                        état durable + quotas
                                                     │
                                               runner Pitgun
                                                     │
                  Authority ── enveloppe ──> Racing incrémental
                                                     │
                                      observation / décision JSON
                                                     │
                          ┌──────────────────────────┴───────────────┐
                          │                                          │
                adaptateur Gemini Pitgun                  API HTTPS de polling
                puis Mistral / Qwen                        ↑ GET / POST sortants
                          │                               agent du participant
                          └────────── décision proposée ─────────────┘

runner ── journal + preuve finale ──> Verifier ──> projection benchmark
```

Ce sont des responsabilités logiques, pas six nouveaux microservices. Pour le MVP : un petit backend/worker Engineer, une file persistée dans PostgreSQL et les services Authority/Verifier existants. Pas de Kafka, de Kubernetes, de moteur de plugins ni d’infrastructure Mac requise. L’API et le worker peuvent partager un binaire, avec la simulation sur un worker borné. Les ressources officielles sont chargées depuis l’archive Pitgun, jamais depuis des chemins ou URL libres.

Le runner utilise le même workload Rust que le WASM. Il conserve l’état de l’essai officiel, produit les observations et transmet les propositions au validateur Racing. L’interface réutilise la présentation Pit Wall des événements et télémétries ; son transport depuis le runner est à ajouter. Pour le premier plan avant-course, le résultat peut être calculé puis affiché en replay, sans streaming réseau.

Un mode local CLI/WASM restera utile pour développer et rejouer gratuitement. Une soumission calculée chez le participant peut être marquée « résultat rejoué et vérifié », mais ne suffit pas à certifier l’absence d’anticipation, de sélection de réponses ou d’intervention humaine.

| Option BYO | Coût / contrainte | Choix |
| --- | --- | --- |
| Pitgun appelle l’endpoint du participant | Hébergement public, DNS, certificats, disponibilité, protection SSRF et rebinding, exposition de credentials | À écarter du chemin principal |
| Participant appelle Pitgun en HTTPS | Polling simple, fonctionne derrière NAT, aucune URL utilisateur à appeler | Architecture cible classée |
| Participant exécute toute la simulation et envoie un bundle | Très simple pour le bac à sable et les expériences ouvertes ; replay possible, causalité/provenance non garanties | Mode local distinct |
| Pitgun exécute un plugin/container fourni | Sandbox, ressources, dépendances et chaîne logicielle supplémentaires | Hors périmètre |

Mistral et Qwen deviennent des adaptateurs internes du même contrat. Le futur Mac mini peut faire tourner un worker qui récupère ses tâches par connexion sortante authentifiée. Seuls les modèles et versions choisis par Pitgun y sont installés. Les modèles BYO restent chez leurs propriétaires ; aucun téléchargement automatique de poids ou de runtime utilisateur côté Lab.

## 3. MVP Gemini : des décisions effectives, un appel par course

Un challenge de démonstration fixe circuit, voiture/composants, pilote, adversaires, distance, météo si supportée, setup, carburant, modèle physique et catalogues. Aucun avantage de carrière. Une première course de 12 tours est une hypothèse de cadrage à valider par les baselines, pas une distance imposée au moteur existant.

Parcours : sélectionner « Engineer Gemini » → voir le périmètre délégué et le budget disponible → lancer → consulter le plan appliqué → regarder la course → comparer au témoin → ouvrir les décisions et télécharger les preuves. L’autorisation de déléguer les actions se choisit avant l’essai. Une modification humaine crée une catégorie assistée ou un nouvel essai, elle ne reste pas dans une entrée autonome.

Le modèle reçoit uniquement le briefing et les capacités autorisées. Il renvoie :

- une liste de relais `{ tire_id, laps }` couvrant exactement la distance ; les arrêts sont dérivés des transitions ;
- une timeline de modes à des débuts de tours futurs, sans déroger au départ commun `balanced` ;
- une courte explication publique facultative, sans rôle dans la simulation.

Les domaines déjà implémentés suffisent. Les sliders de setup, composants, perte de temps au pit, seed et coefficients physiques restent fixés par le challenge. Une extension ultérieure pourra autoriser certains réglages avant-course, dans une catégorie séparée. Aucun réglage sans effet physique démontré n’est proposé au modèle.

Ordre de traitement : réserver l’essai et son quota sous l’identité du challenge → construire le briefing → conserver la requête/réponse Gemini → valider le plan → autoriser l’input avec les relais et l’enveloppe d’instructions → exécuter la timeline existante → soumettre au Verifier → publier si tous les liens de preuve sont valides. L’identité de réservation produit peut précéder l’`execution_id` Authority ; leur lien est explicite et immuable. Un échec d’autorisation ne crée pas un résultat officiel.

Cette première version est annoncée comme **planification avant-course**. Le copilote choisit réellement pneus, arrêts et agressivité programmée ; il ne réagit pas encore à l’usure observée pendant cette même course.

Pour le modèle initial, `gemini-2.5-flash-lite` est une option documentée stable disposant de sorties structurées. La documentation affiche un free tier en entrée/sortie, mais les quotas effectifs dépendent du projet et doivent être lus dans AI Studio. Ne pas transformer un ancien chiffre de RPM/RPD en promesse produit. Sources consultées le 12 septembre 2026 : [modèle](https://ai.google.dev/gemini-api/docs/models/gemini-2.5-flash-lite), [tarification](https://ai.google.dev/gemini-api/docs/pricing#gemini-2.5-flash-lite), [quotas](https://ai.google.dev/gemini-api/docs/rate-limits).

Configuration initiale proposée : un appel maximum par essai, 2 essais/joueur/jour, plafond Lab de 20 appels/jour abaissé si le quota réel l’exige, concurrence d’inférence 1, entrée plafonnée à 4 000 tokens, sortie à 800 tokens, timeout fournisseur 30 secondes. Ce sont des limites Pitgun configurables, à tester sur le plan JSON complet. Le limiteur global respecte aussi RPM et TPM ; il réserve les budgets atomiquement et n’autorise aucune bascule payante automatique. Rejouer un résultat ne consomme aucun appel.

Une réponse tronquée, invalide, bloquée ou une erreur fournisseur produit une trace d’échec. En démonstration, proposer le plan témoin déterministe avec le libellé « repli » ; ne pas créditer cette performance à Gemini. Pas de retry automatique opaque : toute nouvelle génération doit être comptée et enregistrée. Une reprise HTTP de la même demande renvoie son état existant au lieu de recréer un appel.

Utiliser une sortie JSON avec schéma simple, sans outils, recherche, URL context ou exécution de code. Valider de nouveau côté Pitgun : [le support de sorties structurées](https://ai.google.dev/gemini-api/docs/structured-output) n’établit pas la légalité Racing. Clé uniquement côté serveur. Envoyer des données synthétiques de simulation et aucun identifiant personnel : la page de tarification indique que le contenu du free tier peut servir à améliorer les produits Google.

## 4. Engineer Contract : données, décisions et temps logique

Contrat proposé, à formaliser en JSON Schema avec exemples et vecteurs de conformité avant implémentation. Les noms ci-dessous sont nouveaux, ils ne désignent pas des endpoints ou types déjà livrés.

La fonction conceptuelle est `decide(observation) -> proposal`. Deux profils explicites : `pre-race-plan/v1` pour le MVP, puis `lap-control/v1` pour le live. Ils partagent l’enveloppe, l’audit et les erreurs ; leur espace d’actions diffère. Le contrat applicatif reste indépendant des formats Gemini et des frameworks d’agents.

### Observations

| Groupe | Contenu autorisé |
| --- | --- |
| Contexte | Versions du contrat, du challenge et du profil d’observation ; identités immuables circuit/voiture/modèle/catalogue ; distance et objectif |
| Capacité | Identifiants exacts des pneus et modes admis, fenêtres légales, limites de décisions/arrêts, droits effectivement délégués |
| Position logique | `decision_id`, index séquentiel, nombre de tours terminés, prochaine frontière et dernier événement accepté |
| État du joueur | Temps cumulé, derniers tours, pneu et âge, usure, températures et carburant si réellement exposés par le modèle |
| Historique borné | Dernières décisions appliquées et synthèse déterministe des tours terminés ; repli et erreurs précédents |
| Plan courant | Relais/arrêts futurs déjà engagés et mode actif, selon le profil |

Pour le MVP, seules les informations avant-course sont présentes : ne pas inventer une télémétrie initiale ou un signal absent. Pour le live, la projection vient de l’état Solver/Simulator conservé, pas de calculs physiques en TypeScript. Champs indisponibles explicitement absents selon le schéma de capacités ; unités, arrondis, bornes et valeur manquante sont définis.

Exclure l’état RNG, le seed secret d’une évaluation, les trajectoires futures, les futurs incidents, les réponses d’autres contrôleurs, les prompts concurrents et les données personnelles. Le seed reste dans l’archive et peut être publié après clôture. Un modèle externe peut toutefois utiliser le code public ou des essais antérieurs ; cette restriction d’observation ne constitue pas une preuve d’absence de calcul auxiliaire.

La première version live peut ne montrer que l’état du joueur. Le flux actuel avance les concurrents à une frontière commune de **nombre de tours**, pas nécessairement au même instant physique. Un futur tableau d’écarts doit définir s’il compare les passages à tour égal ou un classement au même temps simulé ; ne pas exposer une télémétrie adverse future par rapport au joueur.

### Propositions et application

| Action | MVP avant-course | Live ultérieur |
| --- | --- | --- |
| Plan de relais | Oui : pneus du catalogue, longueurs entières positives, somme exacte | Remplacé par commandes de pit bornées, sans réécriture du passé |
| Mode de pilotage | Timeline préprogrammée via le contrat existant | `set_driver_mode` au prochain début de tour |
| Arrêt | Déduit du plan validé | `request_pit` pour la fin du prochain tour à simuler, pneu explicitement choisi |
| Maintien | Plan témoin ou absence de transition | `keep`, sans événement physique artificiel |
| Setup / paramètres | Fixes dans le premier challenge | Capacités supplémentaires séparées, uniquement là où le moteur les accepte |

Après `k` tours terminés, le prochain tour est d’index zéro-based `k`. Une décision de mode vise `(lap_index=k, segment_index=0)` avant de calculer ce tour, pour `k >= 1`. Le tour initial garde le mode commun du catalogue. Une demande de pit après `k` tours est appliquée à la fin du tour à venir, donc après `k+1` tours terminés ; interdiction d’un arrêt rétroactif. Rejeter un arrêt sans tour suivant et définir la durée/service et les effets thermiques avec la sémantique Racing. Aucun « pit immédiat » ambigu entre tour affiché et tour déjà calculé.

Exemple illustratif de réponse live ; les digests abrégés sont des placeholders :

```json
{
  "schema_version": "pitgun.engineer-proposal/v1",
  "execution_id": "01900000-0000-7000-8000-000000000001",
  "decision_id": "decision-3",
  "observation_digest": "sha256:...",
  "action": {
    "type": "set_driver_mode",
    "effective_at": { "lap_index": 3, "segment_index": 0 },
    "mode": "manage"
  },
  "explanation": "Préserver les pneus avant le relais suivant."
}
```

Le runner lie la décision à l’observation attendue et au concurrent autorisé. Le modèle n’a pas à calculer le hash : l’adaptateur recopie les identifiants de l’enveloppe et produit une proposition typée. Le validateur strict refuse champs inconnus, doublons de clés, nombres non finis, action non permise, mauvaise observation, frontière passée, doublons conflictuels et dépassements de budget. Aucune normalisation permissive du vieux `sanitize_pit_laps` ne doit servir de validation publique du contrat Engineer.

Chaque fenêtre autorise au plus un changement de mode et une demande de pit, dans un ordre canonique fixe. Une proposition composée s’applique atomiquement ou est rejetée entièrement. Le modèle peut utiliser un objet `keep` seul. Prévoir une réponse d’acceptation séparée : statut, code d’erreur, action acceptée, événement réellement appliqué et frontière ; une intention de pit n’est pas encore un pit effectué.

Une explication est un texte public borné, pas une preuve du raisonnement interne du modèle. Elle n’entre pas dans le calcul physique. Une mémoire opaque renvoyée par l’Engineer pourra être ajoutée plus tard ; pour le premier harness hébergé, chaque appel reçoit le même format de synthèse bornée, sans conversation cachée.

### Boucle live à ajouter

`frontière → observation figée → attente externe → validation → journal durable → avance déterministe → événements appliqués`.

La session attend avant l’étape suivante ; l’attente réseau ne fait avancer ni le temps simulé ni l’usure. Le calendrier des fenêtres est fixé par le challenge, par exemple après les tours 2, 4, 6, 8 et 10. Il ne dépend pas de la vitesse de lecture de la vidéo ni des performances du navigateur. Réduire cette fréquence si nécessaire plutôt que simuler une commande arrivée après le calcul.

Timeout proposé : 30 secondes après émission effective de l’observation. Le temps d’attente de la file globale précède cette émission. À expiration, enregistrer `timeout` puis appliquer le repli versionné « conserver mode et plan engagés ». Toute réponse tardive est rejetée. La latence peut ainsi changer les décisions d’une nouvelle tentative ; le replay reproduit l’événement de repli enregistré, sans rejouer le timeout réel. Un incident d’infrastructure Pitgun est distingué d’une défaillance d’Engineer et traité selon le règlement de campagne.

## 5. Transport BYO et sécurité minimale

API cible, après livraison de la boucle live :

```text
POST /v1/engineer/runs                            créer un essai autorisé
GET  /v1/engineer/runs/{id}/observation            polling ou long polling borné
POST /v1/engineer/runs/{id}/decisions/{decision_id} soumettre la proposition
GET  /v1/engineer/runs/{id}                        état, résultat, référence de preuve
```

Un jeton court, révocable et limité à l’essai permet uniquement de lire ses observations et de proposer ses décisions. Un seul `decision_id` ouvert ; les lectures répétées renvoient la même observation. Le premier résultat accepté ferme la fenêtre par transaction atomique. Un renvoi identique retourne le même accusé ; un contenu différent renvoie `409`. Des échéances, tailles maximales, quotas d’essais, limites d’événements et limites de concurrence bornent aussi les agents défectueux.

Un exemple `curl`, puis de petits clients Python et JavaScript suffisent pour démarrer. Un programme dans n’importe quel langage peut appeler son modèle, lire du JSON et répondre en HTTPS. Pas de SDK obligatoire, de MCP obligatoire ou de compatibilité avec une API fournisseur imposée au participant. Un adaptateur JSONL local peut réutiliser les mêmes objets ultérieurement.

| Risque | Réponse de conception |
| --- | --- |
| SSRF / réseau privé | Aucune URL fournie par les joueurs n’est appelée ; destinations hébergées fixées par Pitgun, redirections interdites |
| Exécution de code | Ni scripts, ni imports, ni containers, ni outils LLM fournis par un joueur ; seulement JSON validé |
| Privilèges | L’adaptateur d’inférence n’a pas de secret de signature, de droit de publication ou d’accès direct à la base métier |
| Usurpation / modification | Jeton de session, enveloppe Authority, décision liée à l’observation, historique durable et replay indépendant |
| Épuisement de ressources | Quotas atomiques avant inference, requêtes bornées, une fenêtre ouverte, travailleurs Rust et appels fournisseurs limités |
| Injection de prompt / affichage | Données séparées des instructions système ; aucune autorité accordée au texte du modèle ; explications échappées comme texte |
| Exfiltration de secrets | Secrets fournisseur uniquement dans l’adaptateur ; pas de nom, email, credential, URL privée ni trace d’authentification dans les prompts ou bundles publics |

L’API ne reçoit pas de programmes à évaluer pour calculer le score. Le règlement du challenge choisit des métriques versionnées déjà implémentées par Pitgun.

## 6. Audit, identité, replay et provenance

Trois garanties différentes doivent être affichées :

1. **Résultat vérifié** : le moteur rejoue les inputs et actions, puis retrouve les résultats selon le profil numérique déclaré.
2. **Échanges enregistrés** : les observations émises et propositions reçues sont liées aux actions effectivement appliquées par une trace du runner officiel.
3. **Provenance du modèle** : configuration contrôlée par Pitgun pour un copilote hébergé, identité seulement déclarée pour un BYO distant. Même le runner officiel ne prouve pas qu’un endpoint externe utilise le modèle annoncé plutôt qu’un humain ou un autre programme.

Rejouer ne rappelle **jamais** Gemini. Un seed de simulation et une température d’inférence à zéro ne garantissent pas une nouvelle génération identique. Une réévaluation du modèle est un nouvel essai, avec ses propres décisions et statistiques. Un identifiant API stable ne constitue pas une archive des poids du fournisseur : conserver la version retournée si disponible et déclarer cette limite. Même avec des poids locaux archivés, la reproductibilité de l’inférence nécessite ses propres garanties numériques.

### Identités

- `execution_id` : tentative autorisée, y compris échec ou abandon.
- `run_id` : calcul Racing canonique, contenant l’input final et toutes les actions physiques acceptées. Deux Engineers ayant exactement la même stratégie peuvent légitimement partager ce résultat logique.
- `engineer_config_digest` : provider, identifiant demandé et version retournée si disponible, paramètres effectifs, template système, adaptateur, schéma de sortie et mémoire déclarée. Pour un modèle auto-hébergé : poids, tokenizer, quantification, runtime d’inférence et matériel.
- `benchmark_record_id` : digest d’un manifeste reliant challenge, exécution, run final, configuration Engineer, observations, propositions, verdict et diagnostics d’inférence. Ce manifeste possède une attestation du serveur et un ancrage durable côté backend.

Ne pas ajouter le nom commercial du modèle à la sémantique physique de `run_id`. Ne pas injecter des champs nouveaux dans les schémas V1 stricts. Pour le live, introduire les nouvelles versions Racing d’enveloppe/input terminé nécessaires aux commandes de pneus et pit ; conserver les anciens lecteurs et leurs golden fixtures.

### Contenu conservé

| Artefact | Contenu |
| --- | --- |
| Challenge | Règles/scoring, fenêtres, capacités, budget, adversaires, modèle/catalogue, paramètres et seed ; politique de publication du seed |
| Exécution | Autorisation signée, input initial puis final, enveloppe de décision, reçu et runtime/binaire exact |
| Engineer | Configuration demandée/effective, prompt système et sérialisation exacte du contexte ; artefacts disponibles ou référence attestée |
| Décisions | Observation canonique complète, requête fournisseur expurgée uniquement des credentials, réponse brute bornée, résultat de parsing, validation, repli, événements appliqués |
| Inference | Version renvoyée, tokens, latence, code de fin/erreur, appels et reprises ; mesures opérationnelles séparées de l’identité physique |
| Course | Résultat, télémétrie nécessaire au replay/à l’inspection, métriques et verdict serveur |

Le journal a un ordinal contigu et un lien au digest précédent ; son hash final est ancré au manifeste attesté. Les no-op, rejets et timeouts y restent même s’ils n’ajoutent aucun événement physique. Un simple fichier dont le participant peut réécrire tous les hashes ne prouve aucune provenance.

Conserver les bytes utiles, pas seulement leurs hashes : modèles physiques, packs, schémas, prompts et traces doivent rester récupérables. Les credentials sont exclus dès la capture. Le MVP utilise des prompts publics et de la télémétrie synthétique. Un futur prompt privé doit être explicitement signalé comme une limite de reproductibilité du contrôleur, même si la course reste rejouable.

Le Run Bundle V1 est un format strict avec des noms de fichiers fixes. Concevoir un **Engineer Bundle distinct qui référence le bundle de simulation**, ou une extension versionnée explicite ; ne pas supposer que l’actuel `pitgun replay` accepte un journal Engineer ou les nouvelles preuves live. Réutiliser JCS, manifests, validation atomique et vérificateur pur ; ajouter un lecteur/exporteur Racing Engineer et des fixtures. La vérification locale du bundle établit sa cohérence, tandis que le Verifier hébergé refait effectivement le calcul Racing.

Le vérificateur Engineer doit reconstruire les observations à chaque frontière depuis les seuls événements antérieurs, comparer leurs digests, puis appliquer la décision enregistrée. Vérifier seulement le résultat final manquerait une observation inventée ou une fuite du futur. Le MVP avant-course exige seulement de reconstruire le briefing à partir du challenge et de contrôler la traduction du plan.

Pour le live, écrire l’acceptation durablement avant de faire avancer le moteur. Après crash, recréer la session depuis l’input et rejouer le préfixe accepté, sans recontacter le modèle pour les décisions déjà enregistrées. Un appel fournisseur dont la réponse a été perdue reste une tentative ambiguë tracée ; il ne doit pas être transformé silencieusement en deuxième tirage. Les checkpoints opaques du processus ne sont pas nécessaires au MVP.

## 7. Benchmark et compétition

Créer une projection par `challenge_version`, `controller_profile` et niveau de provenance, indépendante du classement carrière. Le score vient uniquement du résultat vérifié et de la règle de score retenue.

Premier score : statut d’arrivée, puis temps total de course en millisecondes croissant ; conserver explicitement DNF et invalides. Un meilleur tour seul favoriserait une stratégie qui détruit les pneus ou ne termine pas. Afficher aussi delta au témoin, arrêts, taux d’actions valides, replis, tokens et latence. Les égalités au niveau de précision officiel sont des égalités ; ne pas inventer de différence sous la résolution publiée.

Comparer trois niveaux dès le début : plan témoin fixe, politique heuristique sans LLM, Gemini. Tester notamment la timeline vide et le mode `attack` dès la première frontière autorisée. Un LLM doit battre une politique explicable pour que le résultat soit intéressant ; aucune victoire n’est présupposée. Si une stratégie domine partout, publier ce diagnostic et améliorer le modèle gouverné avant de multiplier les modèles évalués.

Progression scientifique : même voiture/pilote/setup et adversaires fixés → plusieurs seeds appariés → plusieurs circuits à compromis différents → répétitions d’inférence → ensemble public de développement et campagne finale réservée. Publier taux de fin, distribution du temps, delta apparié au témoin et incertitude sur des répétitions définies à l’avance. Compter tous les essais officiels et abandons selon le règlement pour limiter la sélection du seul meilleur résultat.

Des seeds réservés réduisent le surapprentissage sans empêcher toute connaissance du simulateur ouvert. Publier leur engagement avant la campagne et les révéler après clôture rend la campagne auditable. Le challenge fixe aussi le budget d’appels, la politique de retry, les délais et la mémoire remise à zéro entre essais. Les tokens n’étant pas une unité strictement comparable entre tokenizers, les présenter avec les limites d’observation et d’appels, sans annoncer une égalité de ressources de calcul.

Deux classements ont un sens : **Open Engineers**, où modèle/prompt/framework/calcul auxiliaire sont libres et déclarés ; **Managed Models**, où Pitgun contrôle adaptateur, contexte, prompt, droits et paramètres autant que possible. Une variante avec intervention humaine est identifiée séparément. Le BYO externe peut être causalement journalisé sans être certifié comme modèle autonome.

Pour la vitrine : page « Bring Your Own Engineer », challenge actif, baseline visible, replay côte à côte, moments de décision annotés et bouton de téléchargement des preuves. La bulle Engineer existante peut montrer « demandé / appliqué / repli » et ouvrir les données ayant motivé la proposition. Les commentaires d’après-course ne sont pas présentés comme des conseils live. Pas de pause modale imposée au Pit Wall : l’attente de calcul peut apparaître comme un état de session.

Un premier challenge communautaire avec goodies devient pertinent après la chaîne complète d’audit et un classement stable. Fixer avant ouverture les configurations admises, tentatives, dates, critères d’égalité et de victoire ; vérifier les résultats primés. Aucune fonctionnalité de paiement n’est requise.

## 8. Découpage incrémental et critères de sortie

Estimations indicatives pour une personne connaissant les dépôts ; elles excluent déploiement non inspecté, disponibilité fournisseur et éventuels changements physiques nécessaires. Les étapes sont arrêtées par leurs preuves, pas par une date promise.

| Lot | Livrable | Réutilisation / travail neuf | Critère de sortie | Ordre de grandeur |
| --- | --- | --- | --- | --- |
| 0 — Contrat et scénario | JSON Schemas des profils avant-course, challenge figé, baseline et vecteurs | Contrats Racing et campagnes existants ; mesures ciblées nouvelles | Plan valide exécuté/rejoué, cas illégaux définis, intérêt de la stratégie mesuré, modèle/catalogue/runtime compatibles | 1–3 jours |
| 1 — MVP Gemini | Un appel, plan réel, runner borné, audit du briefing, replay et page résultat | Timeline/relais et Authority/Verifier existants ; adaptateur, quotas, archive Engineer et projection nouvelles | Clé absente du client ; retry idempotent ; plan et résultat liés ; rejet et panne fournisseur visibles ; reproduction hors ligne sans API | 4–7 jours après lot 0 |
| 2 — Commandes live | Pause à frontière, observation, mode live, puis pit/pneus live | Sessions actuelles ; nouvelle entrée de commandes Solver/Simulator, autorisation et preuve finale à étendre | Préservation fuel/usure/température ; passé immuable ; replay du journal identique ; observation sans futur ; crash/reprise testé | 1–2 semaines, à préciser après spike |
| 3 — BYO ouvert | API pull, jetons par essai, exemples Python/JS, suite de conformité | Même runner et mêmes profils | Agent local derrière NAT ; aucun code/endpoint utilisateur côté serveur ; doublons/timeouts/conflits bornés ; provenance déclarée affichée | 3–5 jours |
| 4 — Benchmark | Campagnes multi-seeds/circuits, heuristiques, métriques, archive publique | Expérimentation gouvernée existante | Toutes tentatives prises en compte ; comparaisons appariées ; score recalculable ; versions et catégories distinctes | Incrémental |
| 5 — Copilotes locaux | Adaptateurs Mistral/Qwen sur matériel disponible | Même contrat Engineer et distribution de tâches | Artefacts épinglés ; latence/mémoire mesurées ; mêmes tests de conformité ; aucune dépendance au Mac pour rejouer | Après disponibilité matérielle |

Le lot 1 peut se limiter à une tranche verticale avant-course en réutilisant les contrats actuels. Le lot 2 ne doit pas être réduit à une callback JavaScript : l’état mutable des contrôles doit entrer dans le moteur conservant l’état, et les pits doivent être engagés avant le tour concerné. Valider d’abord le mode live, puis ajouter les pits, chacun avec ses preuves.

Tests ciblés de livraison : actions non autorisées et limites ; modification/retrait/réordre de décisions ; observation altérée ; timeline native/WASM sous ordonnancements différents ; reprise après journalisation ; budget concurrent ; doublon idempotent ; deux configs Engineer différentes produisant la même stratégie ; aucune publication compétitive avec verdict `PENDING` ou trace incomplète. Pour la portabilité des observations, définir une projection entière à unités explicites et tester les vecteurs : un arrondi arbitraire ne constitue pas une preuve de parité.

Prochaine étape recommandée : lot 0, puis la tranche Gemini avant-course complète. L’architecture prépare le BYO sans attendre le Mac mini, tout en réservant l’adaptation live à un changement explicite et vérifiable du runtime.

## 9. Évolution du framework : simulations pilotées par des contrôleurs externes

Complément après validation des quatre étapes. Le porteur du projet travaille désormais
chez RTE et souhaite prouver à moyen terme que Pitgun s’applique à un second domaine,
notamment l’énergie/réseau électrique ou les drones. Cette orientation justifie de
concevoir la frontière de contrôle au-delà des seules instructions Racing.

### Capacité visée

Le framework peut fournir une boucle de simulation fermée sur un contrôleur :

```text
état simulé → observation autorisée → contrôleur externe
     ↑                                      ↓
évolution physique ← action appliquée ← validation de la proposition
```

« Contrôleur » inclut un humain, une heuristique, un optimiseur, un agent appris ou
un LLM. Le modèle physique reste versionné et fixé pendant l’essai ; le modèle
décisionnel choisit des entrées de commande. L’inférence ne remplace pas le calcul
physique et sa réponse ne devient jamais un résultat de simulation faisant autorité.

Cette boucle permet de mesurer l’adaptation à un état observé, aux incertitudes et
aux conséquences différées des décisions. Elle sert également à comparer une
stratégie préprogrammée avec une politique réactive, même sans aucun LLM.

### Ce qui est déjà générique, ce qui reste à prouver

L’[ADR 0001](adr/0001-runtime-and-domain-workloads.md) sépare déjà runtime, Solver
et Simulator de domaine. Il exige Racing et un second domaine matériellement
différent avant de promouvoir une abstraction de Solver/Simulator dans le runtime.
Cette proposition respecte cette règle et ne modifie pas l’ADR accepté.

Le code possède déjà :

- `LinkedWorkload` dans `pitgun-runtime/src/workload.rs`, pour l’exécution complète
  d’un workload lié à la compilation ; ce trait n’est pas une API de session interactive ;
- un flux incrémental générique dans `pitgun-contract/src/execution_stream.rs`,
  avec ordre, ticks logiques et payloads de domaine ;
- une `decision_envelope` opaque dans `RunAttemptAuthorizationV1`, sans modes Racing
  dans le contrat générique.

Il manque une sémantique partagée de fenêtre de décision et de reprise après
application. La concevoir dans les deux cas d’usage, l’implémenter d’abord dans
Racing, puis stabiliser sa partie commune avec la preuve Grid. Ne pas rendre
obligatoires une session interactive, un contrôleur ou un LLM pour les workloads batch.

| Responsabilité | Propriétaire cible |
| --- | --- |
| Identité de session, ordre causal, références de schémas, digests observation/action, reçu de décision | Contrats génériques, après validation sur deux domaines |
| Cycle avancer / décision requise / reprendre / terminer ; orchestration de vérification | Runtime, via hooks de domaine ; API optionnelle |
| Définition des frontières, état observable, unités, commandes légales, conséquences et repli | Simulator et contrats du domaine |
| Équations, intégration, résolution physique et convergence | Solver du domaine |
| HTTP, polling, fournisseur, attente réelle, quotas, stockage | Host/runner et adaptateurs |
| Score, campagne, classement et UX | Application benchmark/Lab |

L’Engineer Contract devient le profil Racing d’un futur contrat de contrôleur.
L’enveloppe commune doit pouvoir porter un payload typé identifié par un schéma,
sans dictionnaire universel de paramètres et sans `lap_index` dans le runtime.
Le contrat Grid expose ses MW/MWh, celui du drone ses commandes de mission.

### Trois temporalités distinctes

Le pas du solveur, la fenêtre de décision et le temps réel de l’appel fournisseur
ne sont pas interchangeables. Racing peut calculer de nombreux échantillons entre
deux décisions de tour ; Grid peut exposer des intervalles de conduite simulée ;
un drone peut recevoir un objectif de mission au-dessus de sa régulation interne.

Le domaine définit quand une décision est requise et jusqu’où la session peut
avancer. Une fenêtre peut être périodique ou déclenchée par un événement déterministe.
Elle possède un ordre causal, une frontière d’application et un budget explicites.
Le tick de progression n’est pas automatiquement une seconde physique — le flux
Racing utilise actuellement un tick nominal pour ordonner les tours.

La première cible reste une simulation pouvant attendre son contrôleur. La conduite
en temps réel, la cosimulation et le contrôle de matériel sont des exigences
supplémentaires ; aucune garantie de ce type ne découle du replay hors ligne.

Le replay doit conserver également les perturbations et séries exogènes : scénario
de consommation, production, météo ou incidents. Un seed ne suffit pas si le générateur
ou les données ont changé. Les prévisions visibles par le contrôleur sont archivées
séparément des valeurs futures réalisées, pour empêcher une fuite d’information.

### Second domaine recommandé : un petit réseau électrique synthétique

Pour prouver la généralité du framework, un réseau électrique apporte un test
plus éloigné des hypothèses Racing qu’une nouvelle forme de course. La recommandation
est un démonstrateur Grid indépendant du jeu, après le MVP Gemini, avant de figer
le contrat générique du contrôle live. Ce choix de priorité reste à arbitrer :
la [matrice d’ères actuelle](RACING_ERA_CAPABILITY_MATRIX_V1.md) envisage Pod/Drone
comme premier second domaine. Elle n’est pas modifiée par cette proposition.

Périmètre exploratoire : réseau synthétique de quelques nœuds, production pilotable,
production variable, demande et batterie ; horizon de 24 heures avec fenêtres de
15 minutes. D’abord une topologie fixe et un calcul de flux actif simplifié, avec
ses approximations explicites ; pas de revendication sur tension/réactif, stabilité
de fréquence ou transitoires. Un simple bilan production-consommation sans branches
permet de prouver un domaine énergie, mais pas encore le comportement d’un réseau.

Le contrôleur choisit des consignes bornées de production, charge/décharge et
écrêtement. Les observations exposent l’état présent, les marges du modèle et les
prévisions autorisées. Les mesures comprennent énergie non servie, violations de
contraintes, énergie écrêtée, coût et réserve terminale de la batterie, selon un
ordre de priorité fixé avant les essais. La réserve terminale évite de récompenser
une stratégie qui vide artificiellement tout le stockage au dernier pas.

Une action hors contrat est rejetée ; une action légale mais mauvaise doit pouvoir
produire une mauvaise performance. Un éventuel mécanisme automatique de protection
est explicite, versionné et journalisé : il ne corrige pas secrètement la stratégie
du contrôleur. Définir aussi le traitement des états infaisables ou non convergents.

Comparer un plan fixe, une heuristique, un optimiseur de référence et le LLM, avec
les mêmes informations et budgets d’accès au simulateur. Aucun avantage d’un LLM
n’est présupposé. Le LLM peut aussi proposer des objectifs à un optimiseur ; cet
ensemble est alors évalué comme un contrôleur composé identifié dans les preuves.

### S’appuyer sur l’écosystème énergie

[Grid2Op](https://github.com/Grid2op/grid2op) propose déjà un environnement de
décision séquentielle sur réseau électrique et sert aux compétitions L2RPN. Ses
[backends](https://grid2op.readthedocs.io/en/latest/user/backend.html) séparent la
résolution physique de l’environnement de décision. Sources consultées le
12 septembre 2026. Le simple cycle observation/action n’est donc pas une nouveauté
à revendiquer pour Pitgun.

Le positionnement à éprouver est la chaîne de contrats, exécution, télémétrie,
preuves, replay et comparaison commune à plusieurs domaines. Pour le prototype,
évaluer un petit modèle Rust dont la portée est maîtrisée ; comparer ses résultats
à un outil établi avant toute prétention physique plus large. Un adaptateur vers
un solveur externe est une autre trajectoire à instruire, pas un support déjà fourni
par `LinkedWorkload`. Son binaire, ses dépendances et son profil numérique devront
être épinglés ; le WASM et la parité exacte ne sont pas des prérequis à imposer à Grid.
Le profil de comparaison à tolérances exige une implémentation et des fixtures avant
de pouvoir être annoncé comme pris en charge.

### Place des drones et preuve de réussite

L’ère 7 conserve son intérêt comme continuité du jeu : allocation d’énergie,
réserve, thermique et choix de mission sur un parcours fixé. Elle pourra consommer
la même frontière de contrôleur. Un véritable domaine Drone nécessite toutefois
son propre état, Solver, contrat et contraintes ; changer la voiture et le décor
ne prouve pas la généralité du framework.

Le test de réussite du framework serait : charger un scénario Grid sans dépendance
aux crates Racing, le piloter avec plusieurs contrôleurs, conserver ses observations
et actions, puis vérifier le résultat via les mêmes mécanismes génériques. Aucun
concept de tour, voiture ou stand ne doit apparaître dans le host commun. Les scores,
la physique et la granularité temporelle restent propres à Grid. Le rejouage Racing
historique et son chemin batch demeurent compatibles.

Cela ajoute un jalon de preuve multi-domaine à la trajectoire validée, sans faire
d’un simulateur électrique complet une dépendance de la livraison Gemini.

## 10. Premier lot de conseils — état local du 12 septembre 2026

Le lot [game #243](https://github.com/loicbelec/pitgun-game/issues/243) prépare les
observations et leurs preuves dans `game/src/engineer`. Deux règles déterministes
interprètent les données autorisées : plafonnement vitesse/régime en première,
puis hypothèse de retard de génération V8 appuyée sur des pertes répétées en ligne
droite. Le contexte de configuration, l’éligibilité du développement, les mesures,
les versions et les motifs de suppression sont conservés dans l’évaluation.

La validation locale emploie le WASM du jeu et le catalogue 1.9.0, avec six scénarios
contrôlés sur Montréal et Spa exécutés deux fois. Résultats et conseils sont
identiques à chaque répétition ; le replay des observations fonctionne sans LLM.
Il s’agit d’un socle de diagnostic en lecture seule, pas encore d’une boucle de
contrôle live, d’une intégration visuelle ou d’une preuve Authority/leaderboard.
Le détail et les commandes sont dans `game/docs/design/ENGINEER_FOUNDATION_V1.md`.

Ce travail ne modifie pas le découpage cible : le framework hébergera le mécanisme
générique observation/proposition/validation/application à une frontière du temps
simulé ; les domaines définissent leurs observations et actions ; le host gère
les fournisseurs, quotas et attentes réelles. Les règles V8, rapports, pneus et
stands restent dans Racing. Le petit démonstrateur Grid servira à éprouver la
partie commune avant de la figer. Rejouer les actions acceptées ne rappellera
jamais le modèle ; une nouvelle inférence restera une nouvelle expérience.

La suite immédiate du jeu est la présentation des conseils et de leurs preuves
([#241](https://github.com/loicbelec/pitgun-game/issues/241), avec le contexte de
comparaison [#240](https://github.com/loicbelec/pitgun-game/issues/240)). La
trajectoire BYOE complète demeure suivie dans
[#161](https://github.com/loicbelec/pitgun-game/issues/161).
