# Raccordement versionné des canaux de puissance

Ce lot raccorde la projection native de `POWER_TELEMETRY_V1.md` aux flux locaux
FP/RACE, au contrôleur, à Authority et au replay Verifier. Il ne modifie pas les
équations de trajectoire, les temps, l'usure ou la consommation. Le modèle
physique composant0.11 et le catalogue1.8.1 restent inchangés.

## Origine du diagnostic

L'historique local retrouve `p_act / power_t`, son plafond1.2, puis
`p_act * throttle` dans2302d05ad447aeb1cf1b886b0e0c53f6cd5aa4ec (9février2026).
La formule a ensuite été conservée dans les extractions du moteur, notamment
2b62f3337aeb9d3fcd955425605e32476f53143c. Cela date son introduction dans l'export ;
ce constat ne démontre pas l'intention de son auteur. Elle n'est pas une mesure
de pédale. Les anciens canaux ne sont pas corrigés rétroactivement.

## Identité et admission

- Canaux5100–5106 : charge moteur équivalente, puissance moteur disponible,
  puissance moteur délivrée, puissance propulsive aux roues, perte transmission,
  résidu de régularisation basse vitesse, budget de propulsion. Toutes les
  puissances sont enW, la charge en%.
- Politique `pitgun.racing-power-channel-policy/1.0.0` :
  `sha256:55433421b519e7e93a9dafe9d8d560255d3ecccab7ba17e22acd7348341d86f2`.
- Politique de contrôle `pitgun.racing-service-control-policy/2.0.0` :
  `sha256:47bda4740951f45e861099019ad773bec79f657d084f7f3134d120875f23cb49`.
- Sélecteur local `pitgun.local-career-power-unit-thermal/v2`.
- Authority choisit la politique2 uniquement pour le digest WASM configuré par
  l'opérateur dans `PITGUN_RACING_POWER_TELEMETRY_WASM_DIGEST`, déjà admis dans sa
  liste des runtimes. Aucun paramètre de requête ne confère cette permission.
- Verifier reconnaît les deux identités exactes. Une politique inconnue ou un
  changement de politique dans l'enveloppe signée est refusé.

Le digest WASM et le bridge doivent être construits et épinglés ensemble depuis
un commit propre. Les anciens runtimes/bridges et la politique1 restent nécessaires
au replay des sessions antérieures. Ce travail local ne configure aucun serveur
hébergé et n'autorise aucune promotion.

## Export et coût

La projection est appliquée après chaque tour natif à ses dates d'export5Hz :
maintien causal commun, sans extrapoler au-delà du préfixe calculé. Un petit
cache des lignes exportées sert à la compatibilité de la réponse terminale ;
il ne conserve ni une deuxième trajectoire native ni un deuxième historique de
frames. Les intervalles de service ne reçoivent pas de puissance inventée.
Les bilans d'énergie restent ceux de la grille native, indépendants de l'export.

Les canaux historiques5008/5009/5014 sont absents du nouveau mode. Le moteur ne
connaît pas les commandes réelles de pédale et de frein : une propulsion nulle
ne suffit pas à conclure au freinage. Le front et l'ingénieur doivent conserver
cette distinction et ne pas appliquer leurs règles de pédale à la charge moteur.

## Qualification native

La passe complète Authority/Simulator/Solver/Verifier passe :30+112+40+16+6 tests,
avant les deux tests supplémentaires du maintien causal et du trou de service.
Le test signé compare les résultats physiques anciens/nouveaux, vérifie le
nouveau replay de référence et refuse une enveloppe altérée. Le replay de
référence natif n'est pas présenté comme une recette navigateur/WASM distribuée.
Les tests ciblés et la recette de l'artefact WASM sont documentés au raccordement
jeu. Clippy strict bibliothèques/binaires et rustfmt passent ; les cinq warnings
préexistants des anciennes cibles de test restent séparés.
