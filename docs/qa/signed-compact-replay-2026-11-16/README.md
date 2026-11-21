# Replay compact signé et runtimes conservés — 16 novembre 2026

Suite locale du [prototype compact](../compact-replay-2026-11-16/README.md).
Direction d'architecture : [ADR0003](../../adr/0003-distributed-execution-and-independent-replay.md).

## Livré localement

Le Verifier peut conserver jusqu'à huit couples catalogue/runtime supplémentaires
dans un manifeste opérateur borné. Chaque couple garde son adaptateur, ses pins
WASM/bridge, son budget et son identité de vérificateur. Aucun chemin ou URL
exécutable ne vient des requêtes. Tous partagent le sémaphore existant.

L'image peut embarquer l'adaptateur compact sans l'activer par défaut. Authority
continue de n'émettre que pour son runtime configuré. La recette lance deux
Authority **jetables**, chacune avec une admission distincte, et un seul Verifier
avec les deux routes. Elle ne retague aucune ancienne signature.

## Qualification

[Rapport public sans preuve/signature/nonce](report.json).

- 27 tests Authority,16 tests bibliothèque Verifier et6 tests du binaire passent.
- Clippy `--all-targets -D warnings`, formatage et syntaxe Node passent.
- 19 cas de recette réelle HTTP/WASM passent :6 résultats VERIFIED,7 historiques
  falsifiés/rechaînés refusés, signature hexadécimale altérée refusée, runtime
  inconnu refusé, artefact serveur corrompu PENDING puis rétabli VERIFIED,
  démarrage refusé pour mauvais pin conservé et doublon de couple.
- Nouveau runtime : Keep/Pit/Timeout à Monza, Keep à Silverstone et Monaco.
  Ancien runtime07 : Keep à Monza. Sept tours, une voiture,5Hz,catalogue1.8.1.
  Les arrêts attendus sont `[3,5]`, contre `[2]` pour le changement de stratégie.
- Les empreintes de vérificateur diffèrent entre chemins : runtime et adaptateur
  sont liés à l'identité du verdict. Les signatures sont vérifiées avant replay ;
  observations, reçus et propositions sont régénérés et comparés. Aucune inférence
  ni clé de modèle n'est nécessaire à la recette.

Hôte macOSarm64,Node23.11.0 ; services Rust release, clé HMAC temporaire supprimée
en sortie, ports loopback éphémères, processus arrêtés. Les33 cas hors ligne
Node22 du prototype restent une qualification distincte. La recette signée ne
mesure pas un gain VPS et ne remplace pas une carrière complète/publication ou
une recette hébergée. Les mesures GP53 restent celles du prototype précédent.

WASM candidat `cb24d7f0bf474cd88eb3e05e828690438972a7f40d7875e6ac3bc2bb3f103451`,
bridge `8148432845163d31f23fb1c2fe15defe1f49763d9dc51f18f06122a66e4f773c`.
Ancien runtime07/bridge1de conservés sans modification. Aucun changement de
physique,Hz,entrée métier ou format de preuve publique.

## Reproduction et limites

Construire les binaires Authority/Verifier release et l'exemple
`racing_service_control_probe`, puis exécuter
`scripts/qualify-signed-service-replay.mjs` avec les quatre chemins d'artefacts
et un nouveau répertoire de sortie privé ; voir le [runbook Verifier](../../../services/pitgun-verifier/README.md).
Les preuves détaillées restent dans
`.maintenance/signed-compact-final-2026-11-16.local`, hors dépôt/publication.

Ce raccordement n'implémente ni file durable ni exécuteur distant. L'ADR définit
ces limites, la confiance requise et la séparation exploration/certification.
Le jeu conserve sa politique actuelle de preuves et ses Workers. Le futur
exécuteur hors VPS ne devra recevoir ni clé HMAC Authority ni accès en écriture
à la base ; déplacer tout le Verifier actuel déplacerait cette clé partagée.

Staging,production,les services locaux persistants et les carrières utilisateur
sont inchangés. Restent avant activation : qualification élargie des distances et
plateaux, packaging/admission de release avec anciennes versions conservées,
puis recette staging habituelle. L'optimisation du Worker navigateur est un
incrément distinct : son digest de résultat complet sert toujours à la reprise.
