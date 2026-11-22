# Performance et durée de vie des pneus — candidat natif

Suite au [diagnostic pneus/stands](../pit_strategy_audit_v1/RESULTS_V1.md) et à
[#472](https://github.com/loicbelec/pitgun/issues/472), avant la reprise du plateau
fixe de [#457](https://github.com/loicbelec/pitgun/issues/457).

**Statut : hypothèses locales à comparer, aucun profil choisi ni admis.** Le jeu,
ses sauvegardes, les catalogues publiés et les services déployés restent inchangés.
Les anciennes campagnes sont conservées dans leur branche/source figée.

## Loi explicite, sans nouvelle variable d'état

L'usure normalisée `w` conserve son accumulation de travail dissipé, sa dépendance
thermique et son budget matière en joules. Seule sa conversion en adhérence peut
choisir une nouvelle loi, portée par `TireParams.wear_grip_curve` :

`G_w(w) = max(G_min, 1 - a w - b w^p)`.

Les coefficients sont finis, positifs ou nuls, `a+b<=1` et `p` entier dans 2..16.
La loi est monotone décroissante sur 0..1 : le majorant d'usure du forecast fournit
toujours un minorant d'adhérence. Le solveur, les métriques du banc et les enveloppes de freinage
utilisent le même helper. Aucun cap de freinage ni tolérance n'est relâché.
Les puissances entières utilisent une séquence explicite de produits.

Sans cette option, l'expression historique `max(G_min,1-k w)` reste active.
Le champ absent n'est pas sérialisé : les anciennes entrées conservent leur
représentation et leurs digests. Les ressources et l'admission d'un nouveau
modèle demeurent distinctes de cette compatibilité mathématique.

Les trois [profils](profile-candidates-v1.json) partagent `a=.02`, `b=.18` et ne
diffèrent que par `p=2/4/8`. À 100 % du budget d'usure, la perte directe vaut 20 % ;
à 20 %, elle reste faible. Ce sont des paramètres de modèle réduit à qualifier,
pas des coefficients identifiés sur un pneu réel. Le budget n'est pas une mesure
de gomme en millimètres ni une probabilité de crevaison.

## Cohérence des paramètres

La fenêtre thermique est élargie (sigma ×3,1). Le gain d'usure thermique est
multiplié par 3,1², afin de conserver le rapport `gain/sigma²` et donc la pénalité
d'usure en fonction de la température. Les nouvelles trajectoires peuvent changer
le travail et les températures effectivement atteints ; les bilans sont recalculés.

Les écarts de grip entre composés autour de 1 sont multipliés par 0,25. Les montages
surface/carcasse à 70 °C sont une hypothèse homogène de préparation du jeu, soumise
aux essais, sans affirmation de fidélité historique pour toutes les ères. Les
capacités, échanges thermiques, travail de déformation, budgets d'usure 16/24/34 MJ,
refroidissement moteur, carburant initial et pénalité réelle de stand 22 s sont
conservés. Aucun décalage de chrono n'est ajouté après résolution.

La forme progressive s'appuie qualitativement sur [West et Limebeer, manuscrit
2022](https://repository.up.ac.za/server/api/core/bitstreams/a08bc69d-ba39-4611-b73d-905d3f494118/content).
Le [cahier FIA 2025, annexe F1 A2/A4](https://legal.fia.com/web/appeloffre.nsf/0F43EC70A0FACADFC1258991006043A9/%24FILE/ITT%20F1%20TYRES%202025-2027%20%28including%20F2%20and%20F3%29_UPDATE%2014.04.2023.pdf?openelement=)
fournit un repère de conception daté, sans constituer une calibration industrielle.
Les valeurs et hypothèses propres à Pitgun restent explicites dans le JSON.

Le [compilateur partagé](../../crates/pitgun-racing-simulator/src/tire_performance.rs)
résout les trois composés au départ, même ceux absents du plan initial. Les
futures commandes de stand pourront ainsi reprendre les mêmes paramètres, sans
retomber sur un pneu historique. Son application initiale est atomique et ne
réinitialise ni usure ni carburant ; elle ne doit pas être réappliquée à une
reprise de checkpoint. Ce compilateur ne constitue pas une admission de modèle
ni une frontière de confiance Authority.

## Campagne et protections

Le [plan](plan-v1.json) compare trois profils sur Monza I, Zandvoort I, Montréal II
et Mexico II. Chacun utilise 114 plans à un/deux arrêts et 84 diagnostics à 3..12
arrêts. Les plans hors limite ne peuvent pas devenir des plans de plateau.
Une sélection globale prédéclarée privilégie progressive4, puis 8, puis 2 parmi les
profils qui satisfont les critères ; aucun paramètre ne varie par circuit.

L'absence d'avantage des diagnostics 3+ doit résulter du modèle. Tous les échecs
sont conservés. Un profil qui échoue est REFINE, sans correction des chronos.
Manage/Balanced/Attack et cinq tours sans arrêt isolent ensuite des comportements
supplémentaires ; ils ne prouvent pas une qualification exhaustive du risque.
Le +1 cooling hors budget sert à isoler un effet physique, pas à comparer deux
joueurs à budgets identiques.

Le kernel est partagé avec le futur fitting : cache privé épinglé au binaire,
sources et ressources ; fichiers écrits atomiquement ; répétitions exactes ;
décompte séparant calcul frais et réutilisation. Les 100 contrôles source doivent
être reproduits avant cette campagne. Les seeds réservés du futur plateau restent
fermés. La recette native n'est pas une admission WASM/Authority/Verifier.

```sh
cargo build --release --offline --locked -p pitgun-racing-simulator --example tire_performance_probe
python3 experiments/tire_performance_v1/qualify.py target/release/examples/tire_performance_probe
python3 experiments/tire_performance_v1/qualify_extended.py target/release/examples/tire_performance_probe /private/tmp/pitgun-tire-performance-cache
python3 experiments/tire_performance_v1/run_screen.py --preview
python3 experiments/tire_performance_v1/run_screen.py target/release/examples/tire_performance_probe /private/tmp/pitgun-tire-performance-cache
```

Adapter `CARGO_TARGET_DIR` au cache local. Ne pas recompiler le binaire, modifier
les sources physiques, le kernel ou les profils pendant les calculs. L'ancien
comportement doit être reproduit depuis sa source figée ; le nouveau modèle doit
porter sa propre identité lors d'une future admission.
