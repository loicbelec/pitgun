# Configurations de carrière : résolution et premier écran natif

Source du résolveur/probe/protocole figée avant calcul : **aa48919**.
Livraison [brouillon #467](https://github.com/loicbelec/pitgun/pull/467), empilée sur #465.
Catalogue immuable v1.8.1. Coefficients thermiques V2 et pilotes V2 inchangés.
[Protocole](plan-v1.json), [méthode](README.md),
[synthèse machine](results/summary-v1.json),
[identités compilées et sources](results/provenance-v1.json).

## Ce qui est vérifié

- 54 préparations issues des vrais réducteurs, 540 voitures, deux résolutions
  strictement identiques. Les trois routes de doctrine sont ici des alias :
  **18 requêtes physiques distinctes**, pas 54 essais indépendants.
- Moteur réellement installé : 198 résolutions `v8_1960`, 342 `v8_1970`. À
  l'entrée de l'ère II, l'ancien V8 du joueur et les nouveaux V8 adverses sont
  distingués malgré leur véhicule de base commun. À la sortie, le remplacement
  acquis par le joueur est bien utilisé. Ne pas extrapoler ces nombres aux
  voitures moteur modernes non exécutées dans cette campagne.
- 18 GP officiels, joueur seul, deux anticipations de frontière et deux passes
  exactes : **36 cas / 72 exécutions**, tous terminés. Carburant hérité 100 kg,
  convention de pneus héritée 90 °C, relais inchangés. Pas d'auto-préparation ni
  d'augmentation de réservoir pour faire réussir un cas.
- **500 réglages de cinq tours**, trois étapes de carrière, Monza/Zandvoort,
  budgets discrétionnaires 8/11/16 plus bonus HQ fixes. Fractions moteur/châssis
  échantillonnées, cinq rapports de boîte dont les deux bornes, trois réglages
  d'appui lorsque déverrouillés, allocations avec transferts vers cooling/aéro.
  Un passage de cet écran exploratoire ; les meilleurs échantillons par catégorie
  sont ensuite confirmés sur GP, sans substitution si un GP échoue.
- Douze réglages sélectionnés sur cinq tours, deux anticipations sur leur GP
  complet et deux passes exactes : **24 cas / 48 exécutions**, tous terminés.
  Au total 560 cas distincts de simulation, 620 exécutions, aucun échec supprimé.

Les neuf adversaires sont **résolus**, pas exécutés ou recalibrés. Un test modifie
uniquement le réglage du joueur et compare l'intégralité des objets adversaires :
ils restent exacts, bruit et entrées compris. Les anciens ancrages moteur,
mécanique et thermique sans remplacement sont conservés. Les mappings/pilotes
manquants et les modes visant un concurrent inconnu sont refusés.

## Continuité et bilans

Sur tous les tours acceptés des 560 cas, le saut entre l'état après le tour/service
et la première valeur du tour suivant vaut **exactement zéro** pour vitesse,
température moteur, surface, carcasse et usure. Le banc ne prétend pas mesurer un
saut de carburant point à point : il n'existe pas de trace dédiée dans cette sortie.

| Contrôle | Maximum observé |
| --- | ---: |
| Erreur relative du couplage d'adhérence | 0,000999857, sous la tolérance 0,001 |
| Borne d'utilisation du contact | 0,994315 |
| Passes de couplage par segment | 15 sur 24 autorisées |
| Résidu bilan thermique moteur | 2,29 × 10⁻⁸ J |
| Résidu bilan thermique pneus | 5,99 × 10⁻⁹ J |
| Résidu bilan énergétique carburant | 0 J |
| Résidu bilan véhicule | 3,64 × 10⁻¹² J |
| Recalculs conservateurs de repli | 0 |
| Mises à jour de borne non encore publiées | 1 maximum |

Ces valeurs qualifient la cohérence numérique du domaine exécuté, pas une
identification expérimentale des paramètres. Les compteurs de tentatives restent
distincts du journal des faits et des observations Engineer. Aucune vitesse
acceptée n'est rabotée, aucun préfixe n'est réécrit, aucune tolérance élargie.

## Réglages : la durée de course change la conclusion

Chaque ligne compare les meilleurs **échantillons sur cinq tours** de deux
catégories, puis leurs GP complets à relais identiques. Les deux catégories
optimisent leurs curseurs dans la grille autorisée. « Concentré » signifie
uniquement une dépense discrétionnaire moteur/châssis ; les bonus HQ d'aéro et de
cooling restent présents. « Diversifié » inclut ici un transfert vers cooling.
Il ne s'agit pas d'une ablation isolant le refroidissement : d'autres points et
parfois les curseurs diffèrent entre les deux réglages sélectionnés.

**Écart = temps concentré − temps diversifié. Positif : diversifié plus rapide.**

| Étape | Circuit | Écart sur 5 tours | Écart sur GP complet |
| --- | --- | ---: | ---: |
| Entrée ère I, 8 points | Monza | +2,84 s | +61,06 s |
| Entrée ère I, 8 points | Zandvoort | +3,54 s | +136,71 s |
| Sortie ère I, 11 points + HQ | Monza | +2,64 s | +57,79 s |
| Sortie ère I, 11 points + HQ | Zandvoort | +3,32 s | +131,48 s |
| Sortie ère II, 16 points + HQ | Monza | +0,14 s | **−4,70 s** |
| Sortie ère II, 16 points + HQ | Zandvoort | +0,57 s | **−19,58 s** |

Les deux méthodes de frontière donnent le **même ordre pour les six paires sur
GP**. Les écarts changent cependant, jusqu'à environ 1,86 s entre les différences
de temps de ces paires : elles ne sont pas numériquement équivalentes. Cette
vérification ne prouve pas que tous les classements de réglages/stratégies sont
invariants.

En ère I, les dépenses concentrées ne dominent donc pas systématiquement cet
échantillon. En fin d'ère II, elles restent compétitives et gagnent ces deux
confirmations ; **le raccourci moteur/châssis ne peut pas être déclaré éliminé**.
Les bonus HQ apportent déjà 1 point de cooling à ces allocations concentrées.
Un meilleur réglage sur cinq tours peut perdre sur GP : ne pas calibrer les
adversaires seulement à partir du classement court.

**Clarification du propriétaire après cette campagne :** ces victoires de fin
d'ère II ne constituent pas un défaut en soi. Les bonus HQ et le moteur installé
changent les besoins. La cible est une importance réelle et contextuelle de
chaque axe disponible, sans répartition obligatoire ni obligation de faire perdre
les dépenses concentrées. Voir les [critères détaillés](ACCEPTANCE_CRITERIA.md).

Les valeurs de derating et températures par tour sont conservées. Sur les
références héritées de l'ère I, le minimum de fraction de puissance varie selon
le circuit jusqu'à environ 0,915 à Zandvoort ; les références de sortie d'ère II
avec 5 points effectifs de cooling ne subissent pas de derating. Cela ne garantit
ni le besoin d'une même allocation partout ni une pénalité artificielle de
cooling nul. Le cas préalablement qualifié de faible gain précoce reste inchangé,
aucun coefficient n'ayant été retouché ici.

## Portée, stockage et contrôles

Les graines/conditions sont fixes et ces cas sont désormais **exposés**. Restent
réservés les autres graines/conditions et les réponses aux réglages des autres
circuits ; leurs simples références GP ne sont pas des holdouts de pace.
Pas de balayage entier, d'optimum continu, de recherche pneus/stands, de jeu
contre un champ exécuté, de saison complète ou d'admission physique signée.
Pas de mesure de temps/RSS : quatre processus natifs concurrents, aucun chiffre
de performance navigateur/WASM/VPS déduit de cette campagne.

Les trois gros reçus sont archivés sans perte : environ 22,1 Mo de JSON deviennent
2,86 Mo de gzip. Les digests d'origine portent sur les octets décompressés ; les
digests compressés figurent séparément dans `packaging-v1.json`. Ni télémétrie
point par point complète ni sauvegarde personnelle ne sont conservées.

Validation locale : quatre nouveaux tests de résolution ; **598 tests passent,
0 échec, 22 ignorés préexistants**, espace de travail et doctests inclus.
Compilation de toutes les cibles, format/diff et Clippy strict du périmètre CI
passent. Le Clippy élargi à tous les anciens
tests du Solver rencontre quatre `field_reassign_with_default` préexistants dans
`tests/physical_kernel.rs`, inchangé par ce lot ; ce n'est pas un échec de course.
Le contrôle strict correspondant au périmètre CI a été exécuté séparément.

## Prochaine étape

1. Pré-enregistrer une recherche **sur GP complet** autour des allocations
   concentrées et de transferts voisins, puis stratégies/services à contrôles
   identiques. Ne pas confondre horizon court et optimum de course.
2. Réserver graines/conditions avant tout ajustement. Décider sur ces résultats
   s'il faut améliorer une loi physique, sa résolution en points ou la politique
   d'adversaire ; aucune modification uniquement pour imposer tous les axes.
3. Figer ensuite les profils de neuf adversaires et les éprouver sur des saisons,
   avec les deux méthodes/conditions nécessaires. #457 reste préalable à toute
   conclusion d'équilibrage. #461 reste ouvert pour la portée plus générale.
4. Qualifier séparément Worker/WASM, observations/événements et admission
   Authority/Verifier avant une intégration dans le jeu ou un déploiement.
