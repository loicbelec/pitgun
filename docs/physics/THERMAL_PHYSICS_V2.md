# Candidat thermique moteur et pneus V2

Statut : **implémenté et raccordé au solveur natif, paramètres exploratoires**.
Le champ facultatif `ResolvedSimulationRequestV3.thermal_physics_v2` active le
candidat et exige le noyau travail-énergie V1, le carburant par BSFC et des gommes
explicitement résolues. Tous les constructeurs de carrière continuent à fournir
`None`. Aucun catalogue, Worker/WASM, jeu local, staging, production, permission
Authority ou identité Verifier n'est remplacé. Une compilation du framework n'est
pas une admission du candidat dans le jeu.

Le nouveau marqueur est `pitgun.racing-thermal/v2`. L'ancien export de puissance et
l'ancien rééchantillonneur refusent cette sémantique. Les champs absents restent
omis de la sérialisation historique. Les preuves anciennes restent exécutables.

## Mécanique et énergie déjà corrigées

Le [noyau longitudinal V1](LONGITUDINAL_ENERGY_V1.md) conserve le principe de
Newton, les cartes de couple et les résistances. Il remplace une discrétisation
incohérente par le bilan d'un déplacement :

```
1/2 m (v1²-v0²) = Wroues - Wfreins - Wtraînée - Wroulement - m g Δz
Δt = 2 Δs / (v0+v1)
Wmoteur = Wroues + Wtransmission + Wembrayage
```

Ainsi la vitesse constante paie les résistances, le démarrage est à couple fini,
la durée de mouvement sert aussi aux flux, et les pertes de transmission ne
disparaissent pas. Cette correction répond à des erreurs du noyau précédent ;
l'hybridation bénéficie ensuite de ces ports énergétiques explicites. Elle ne
justifie pas de refaire toute la dynamique ni d'ajouter immédiatement une batterie.

## Moteur : carburant, chaleur, radiateur et thermostat

Au travail d'arbre mesuré sur le segment on applique le BSFC déjà résolu :

```
mf = BSFC Wmoteur / 3 600 000 + débit_ralenti Δt
Echimique = mf PCI
Epertes = Echimique - Wmoteur
Qmoteur = χ Epertes
Eéchappement_et_autres = (1-χ) Epertes
Ce dTe/dt = Qmoteur/Δt - H(Te,v)(Te-Tair)
H = Hpassif + ouverture(Te) (Hradiateur,arrêt + Kv v)
ouverture = clamp((Te-Tdébut)/(Tplein-Tdébut), 0, 1)
```

BSFC : kg/kWh ; PCI : J/kg ; capacités : J/K ; conductances : W/K ; `Kv` :
W/(K·m/s). Les températures sont en °C, leurs différences en K. Le profil refuse
une combinaison BSFC/PCI produisant plus de travail que d'énergie chimique. Le
reste des pertes est agrégé, pas présenté comme un échappement mesuré. Le même
bilan fonctionne au ralenti pendant le service, où l'ancien candidat ajoutait du
carburant sans sa chaleur chimique.

L'échange air est signé ; il peut réchauffer un moteur froid. Le thermostat évite
de traiter un radiateur fermé comme un échangeur pleinement exposé. C'est une
fermeture réduite, pas un modèle hydraulique de pompe ou de circuits eau/huile.
L'ancienne loi `alpha_heat × Wmoteur` est remplacée uniquement dans cette branche.

L'ouverture est tenue sur chaque sous-pas d'au plus 250 ms ; l'EDO linéaire de ce
sous-pas est intégrée exponentiellement. Ce n'est pas une solution exacte de toute
l'EDO non linéaire avec thermostat. Un test de raffinement temporel couvre cette
approximation. Le seuil/slope de derating proviennent des ressources de chaque
moteur ; le solveur réduit effectivement le couple avant de résoudre le mouvement.

Le résolveur d'expérience choisit le dimensionnement par **moteur installé**,
jamais par le seul numéro d'ère :

```
taille = taille_base + gain (cooling_effectif / 20)
Kv = Kv_famille × taille
CdA = CdA_hors_ancien_coût_cooling + nouveau_coût_cooling × (cooling_effectif/20)
```

Ce mapping reste dans le Simulator/probe et ses données, pas dans les équations
du Solver. Les valeurs sont des hypothèses de dimensionnement : elles ne sont
pas des mesures historiques de radiateurs. Les seuils 110°C n'ont pas été abaissés
pour fabriquer le derating précoce. L'opportunité de budget et la traînée pénalisent
le surdimensionnement. Aucun malus spécial n'est appliqué au point zéro ; aucun
minimum de points n'est décrété gagnant partout. Une masse de radiateur n'est pas
encore ajoutée. Les bonus HQ doivent être résolus avant cette frontière physique.

## Pneus : surface et carcasse

Deux états agrègent les quatre pneus. `tire_temp` devient explicitement la surface
qui gouverne le grip, `tire_carcass_temp` la structure à inertie plus lente. Les
capacités et conductances sont propres à chaque gomme résolue.

```
Cs dTs/dt = Pdéformation,surface - Gsb(Ts-Tb)
             - Hsa(v)(Ts-Tair) - Hsp(Ts-Tpiste)
Cb dTb/dt = Proulement + Pdéformation,carcasse
             + Gsb(Ts-Tb) - Hba(Tb-Tair)
```

Les échanges air/piste sont signés, le transfert interne se compense dans la somme.
La température de piste est une condition aux limites distincte, non une météo
dynamique ou une température inférée des voitures. Aucun flux de freins n'est
attribué artificiellement au pneu.

Le solveur 1 D ne résout pas les vitesses de glissement. Le candidat emploie une
**fermeture réduite de pertes par déformation**, et ne revendique pas le modèle de
glissement des articles :

```
Fx_base = m(v1²-v0²)/(2Δs) + Ftraînée + Froulement_base + Fpente
u = clamp(hypot(Fx_base,Fy)/Fdisponible, 0, 1)
Fdéformation = kdéformation N u²
Wdéformation = Fdéformation Δs
Wroulement_base = Crr N Δs
```

Cette résistance participe aux enveloppes de propulsion **et** de freinage et au
bilan mécanique. Une fraction déclarée de ces pertes chauffe les deux capacités ;
le reste rejoint directement l'environnement. Il n'y a pas de chaleur inventée
sans coût mécanique. Le champ existant `EnergyLedgerV1.rolling_work_j` agrège
roulement et cette nouvelle déformation en V2 ; `ThermalLedgerV2` les sépare. Le
marqueur V2 est indispensable pour interpréter cette différence.

La loi de grip gaussienne bornée existante est désormais évaluée sur la surface,
avec les fenêtres/planchers du nouveau profil. L'usure irréversible est nouvelle :

```
Δusure = (Wdéformation + ε Wroulement_base) / Efin_vie_gomme
         × [1 + gain_thermique ((Tmoyenne_surface-Topt)/σ)²]
```

Cette loi semi-empirique relie travail dissipé, gomme et température. Elle n'est pas
une loi de matériau identifiée : pas de graining, blistering, chimie d'abrasion,
pression, humidité ou répartition roue par roue. Le cumul reste borné à1 ; le
grip à1 dépend de la loi résiduelle de la gomme. Aucun affichage client ne gonfle
le pourcentage. L'échelle de durée compétitive et les meilleurs arrêts doivent
encore être validés ; une stratégie peut rationnellement s'arrêter avant 100%.

Les échanges utilisent un système implicite 2×2 à sous-pas≤250 ms, sans allocation
par sous-pas et sans écrêtage de température. Tests : équilibre, maximum passif,
relaxation analytique, bilan, raffinement temporel, échauffement/refroidissement,
usure chaude et piste réchauffant un pneu froid. Les bilans ne suffisent pas à
valider expérimentalement les paramètres.

## Continuité, contrôles et coût

La carcasse traverse les tours et apparaît dans chaque frontière incrémentale,
dans le résultat final et dans les courbes natives. Un remplacement remet surface,
carcasse et usure aux conditions de service explicitement déclarées ; il ne
réinitialise pas le moteur. Les bilans par tour comprennent le ralenti au stand.
Un pneu non résolu avant exécution est refusé avant l'acceptation d'une commande.
Ces états devront entrer dans le futur contrat de checkpoint/reprise admis.

Le V1 conserve ses quatre passes historiques de couplage pneu/vitesse. Le V2
itère jusqu’à une variation relative de grip≤10⁻³ et une borne de sollicitation
réelle du contact≤1.0002 ; il refuse le tour après 24 passes sans convergence. Les références sont sous-relaxées de moitié entre deux passes. Les
erreurs finales et le nombre de passes sont enregistrés. Le V1
anticipe le tour suivant à grip de fin de tour tenu constant : le pilote local a
montré que cette anticipation ne suffit pas avec des variations rapides de grip.
Le V2 utilise provisoirement le **minimum mathématique de grip** de la gomme pour
cette fermeture de freinage. Cela évite une remise à l'échelle instantanée de
vitesse ou la réécriture du préfixe, mais peut rendre le passage de ligne trop
conservateur. Les écarts V1/V2 ne sont donc pas une ablation thermique pure. Il
reste à qualifier un horizon couplé moins conservateur et la précision spatiale avant admission compétitive.

Les transitions dynamiques de conduite et l'ancien proxy supplémentaire de chaleur
de correction ne sont pas admis. Les profils de conduite fixes restent possibles.
L'agent conserve son interface générique observations/événements/commandes ; les
règles de moteur, pneu ou tour restent dans le domaine Racing.

La grille physique et la fréquence des observations ne changent pas. Réduire un
export de 5 à2 Hz ne simplifie aucune de ces équations. Le coût natif, puis WASM et
mémoire du Worker restent des mesures distinctes. L'historique complet utilisé
dans ce banc n'est pas un nouveau format de replay du jeu.

## Références et suite

Les sections 2.1–2.1.1 de [Choi et al., Energies 2020](https://www.mdpi.com/1996-1073/13/20/5301)
documentent la séparation énergie carburant/travail/chaleur et les circuits
thermiques. Les sections 1–2 de [Schabauer et al., IAVSD2023, publié 2024](https://link.springer.com/chapter/10.1007/978-3-031-66968-2_29)
documentent capacités par couche, déformation/glissement, échanges piste/air. Nous
retenons les principes et choisissons ici une réduction à deux états ; nous ne
reprenons ni leur modèle complet ni leurs paramètres expérimentaux. La nouvelle
usure et la fermeture `k N u²` sont des hypothèses propres à Pitgun. La référence
NASCAR antérieure reste distincte : sa profondeur de lecture n'est pas requalifiée
par ce travail.

Le [banc local](../../experiments/thermal_physics_v2/README.md) sépare l'écran de
calibration, les anciens résultats gelés, la répétabilité et la future validation
réservée. Vient ensuite la qualification du couplage, des distances complètes et
des stratégies, puis les contrats de télémétrie/état, Worker/WASM et Authority/Verifier.

L'hybridation ajoutera des ports **explicites** : couple électrique à la
transmission, partage freinage friction/récupération, énergie et rendement batterie,
puissance et capacité maximales, chaleur des pertes. Le bilan actuel prépare ce
raccordement ; il ne fournit encore aucune récupération, batterie ni démonstration
de réduction des émissions. La chaîne de simulation reproductible est une valeur
industrielle potentielle ; une précision industrielle exigerait des données de
mesure et des validations supplémentaires.

## Orientation carburant validée après le premier écran

Le propriétaire valide une **préparation automatique du carburant dans le jeu**.
La masse initiale reste un paramètre physique explicite, versionné et enregistré
pour la reproduction et les expériences du framework. Aucun curseur en kilogrammes
n'est ajouté au Pit Wall dans ce lot. Un futur contrôle de consommation devra
correspondre à un compromis effectif de rythme/énergie, avec l'hybridation.

La préparation devra estimer le besoin depuis la distance, la configuration et le
plan initial, puis ajouter une réserve déclarée, sans recalculer tout le Grand Prix
avant le départ. Charge initiale, débit/énergie admissible, capacité et réserve sont
des contraintes distinctes, propres au profil de véhicule/règles. Le plein de
100 kg du banc n'est ni une obligation F1 universelle ni une capacité admissible
définitive. Un besoin incompatible avec les limites doit rester explicite ; pas
de réservoir augmenté silencieusement, de consommation annulée ou de ravitaillement
implicite pendant la course.

Prochain lot natif : qualifier l'anticipation aux frontières de tour et le coût
isolé avant de poursuivre le choix des coefficients ; définir et éprouver cette
préparation de carburant ; comparer les gommes/plans d'arrêt sur plusieurs circuits
et distances complètes prises en charge. L'épuisement d'un concurrent devra avoir
une issue déterministe propre sans faire échouer toute la session (#445).

Les cas de validation doivent être réservés avant les ajustements. Le gain modeste
de cooling au début est accepté : ne pas le renforcer pour grossir les écarts.
L'hybridation (#246) reste le module physique suivant ; les cas exigeant une
capacité électrique ne sont pas déclarés validés par un simple supplément de
carburant. L'admission Worker/WASM et le raccordement au jeu viennent après la
qualification du profil physique et de ses états/export/replay.

## Préparation et fenêtre opérationnelle —22novembre2026

Le lot [tyre_operating_policy_v1](../../experiments/tyre_operating_policy_v1/README.md)
sépare désormais la préparation des pneus, la dynamique thermique et la lecture
des états. Une politique native explicite prépare les trains modernes slicks à70°C
au montage et aux remplacements. Les prescriptions Pirelli/FIA justifient la
référence de préchauffage externe ; la carcasse initialement homogène à70°C et
l'absence de trajet couvertures→grille sont des simplifications déclarées. Aucune
déduction automatique pour les premières ères, la pluie ou les intermédiaires.

L'événement thermique V2 définit sa fenêtre à partir du facteur thermique réel
`exp(-((T-Topt)/sigma)^2)` et du seuil proche du pic95%, convention Pitgun. Les
paramètres de gommes du candidat restent exposés et non identifiés. Le facteur
thermique est réversible ; le produit avec l'usure et le grip propre à la gomme
est une autre grandeur. La température ne subit pas de clipping à la fenêtre.
Un pneu usé dans cette bande n'est pas à95% de son grip neuf. La politique V1
diagnostique +/-sigma et ses checkpoints/journaux ne changent pas rétroactivement.

Le nouveau banc distingue surface/carcasse/usure/grip, états initiaux froids et
chauds, temps natif dans les bandes et reset du train neuf aux stands. Les
coefficients de chaleur/grip/usure, l'équilibre des stratégies et l'anticipation
aux passages de ligne ne sont pas validés par la seule cohérence de cette lecture.
