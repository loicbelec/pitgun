# Candidat longitudinal et thermique V1

Cette branche introduit de **nouvelles équations natives**, sélectionnées uniquement
par `ResolvedSimulationRequestV3.longitudinal_energy`. Le marqueur sérialisé est
`pitgun.racing-longitudinal-energy/v1`. Les constructeurs de carrière continuent à
sélectionner `None` : le jeu, les anciennes preuves et les catalogues ne changent
pas. Ce candidat n'est pas un runtime admis par Authority ou Verifier.

## Problèmes ciblés

Le chemin historique peut attribuer une propulsion nulle à un segment parcouru à
sa vitesse limite alors qu'il faut compenser traînée et roulement. Sa durée utilisée
pour la puissance, la thermique et les changements de rapport peut différer de la
durée finale du déplacement. La régularisation `max(v, 1)` au démarrage ne représente
pas un mécanisme de transmission. Modifier seulement l'export des canaux ne corrige
pas ces calculs ; la projection de puissance du lot précédent reste historique.

Le candidat remplace ce noyau par un bilan travail-énergie et une horloge commune.
Il garde les paramètres existants pour isoler le changement d'équations. Il ne
prétend pas que les paramètres sont identifiés ou que la voiture est validée.

## Équations et unités

Pour un segment de longueur `ds`, de dénivelé `dz`, à masse `m` tenue constante,
avec vitesses d'entrée `v0` et de sortie `v1` en m/s :

```
vm = (v0 + v1) / 2
Dt = 2 ds / (v0 + v1)
Fd = 0.5 rho CdA vm²
Flift = 0.5 rho ClA vm²
N = max(m (g + vm² dSlope/ds) + Flift, 0)
Fr = Crr N
Fg = m g dz / ds
Frequired = m (v1² - v0²) / (2 ds) + Fd + Fr + Fg
```

`CdA` et `ClA` sont des surfaces équivalentes en m² ; les forces sont en N. La
quadrature des forces est au milieu du segment, le travail de pesanteur vaut
exactement `m g dz`. La géométrie 1D imposée et l'approximation de courbure verticale
restent celles du modèle réduit ; ce n'est pas une dynamique multicorps 3D.

Si `Frequired >= 0`, les roues fournissent `Frequired ds`. Sinon les freins dissipent
`-Frequired ds`, sans propulsion simultanée. Rouler à vitesse constante consomme
donc la puissance nécessaire aux charges résistantes ; on n'infère aucune pédale.
La force latérale `m vm² |kappa|` partage le cercle d'adhérence avec la force
longitudinale. La demande brute est contrôlée **avant** tout affichage borné. Une
trajectoire impossible est refusée, pas masquée par un indicateur plafonné à 100 %.

### Moteur, transmission et démarrage

La propulsion est bornée par le couple disponible, le rapport total, le rendement
de transmission et le rayon de roue : `Fmax = torque_Nm ratio eta / rwheel`, puis
par l'adhérence. Le format historique du catalogue exprime `trq` en kN·m ; la
conversion `× 1000` est explicite. Le couple tenu sur un segment ne dépasse pas le
minimum de la carte linéaire sur l'intervalle de régime traversé. Ses extrémités
sont incluses dans ce nouveau chemin, contrairement à l'ancien interpolateur.

Un embrayage idéal glisse sous le régime de ralenti. Son travail perdu est calculé
à partir de l'intégrale de l'écart de vitesse avec la vitesse de verrouillage,
pour une accélération constante sur le segment. Ainsi :

```
Wshaft = Wwheel + Wtransmission_loss + Wclutch_loss
Wtransmission_loss = Wwheel (1/eta - 1)
Wclutch_loss = Fdrive / eta * integral(max(vlock - v(t), 0), dt)
```

La force au démarrage est finie et le véhicule peut démarrer de zéro sans plancher
artificiel de vitesse. Un véhicule incapable de parcourir un segment est refusé.
L'embrayage n'a pas encore d'inertie, de température ou de commande. L'interruption
de couple au changement de rapport utilise la même durée `Dt` que le déplacement.

Les enveloppes de freinage et de propulsion utilisent les mêmes charges au milieu
du segment. Une recherche de racine avec intervalle encadré, de type Illinois,
garde une solution faisable ; elle est bornée à 48 essais et signale l'échec de
convergence. Les tolérances et essais analytiques sont dans le code et le plan.

### Thermique et carburant

Le moteur conserve une température agrégée :

```
C dT/dt = Q - H (T - Tamb)
H = H0 + Hv vm
Q Dt = alpha_heat Wshaft
```

Pour `Q`, `H` et `Tamb` tenus constants sur le segment, l'intégration exponentielle
est exacte. `expm1` évite l'annulation numérique lorsque `H Dt / C` est petit ; la
limite `H = 0` retrouve `DeltaT = Q Dt / C`. L'énergie échangée avec l'air est
signée : l'air peut réchauffer un moteur plus froid. Cette loi est la réduction à
capacité thermique concentrée ; sa validité comme température d'un moteur complet
et les valeurs de `C/H/alpha_heat` restent à identifier. Voir les [références consultées](REFERENCES_ENERGY_V1.json).

Le carburant utilise la loi BSFC existante appliquée au travail moteur, plus le
ralenti multiplié par la durée commune. Il n'y a pas encore de carte de rendement,
de bilan chimique complet, de refroidissement de l'huile/eau séparé ou de batterie.
L'influence physique du refroidissement peut être mesurée sans prétendre avoir
équilibré l'allocation des points `cooling` dans le HQ.

## Continuité des tours et arrêts

Le candidat ne rabote plus instantanément la vitesse au début d'un tour. Il anticipe
le freinage du prochain tour avec l'état de pneu prédit en fin du tour courant,
tenu constant, et les enveloppes de conduite déjà fixées. Ce n'est pas de la
future télémétrie ni une observation fournie à un contrôleur. La fermeture périodique
est bornée à 16 passages ; la limite terminale reste conservative pendant les
quatre passes de couplage existantes. Cette anticipation approximative et les
quatre passes fixes doivent être réévaluées avec le futur modèle de pneus.

La masse de carburant reste mise à jour à la frontière du tour. Le bilan distingue
l'énergie cinétique emportée avec cette masse de celle de la voiture restante.
L'arrêt au stand abstrait conserve sa remise à zéro de vitesse : son énergie est
comptabilisée dans `unresolved_service_velocity_reset_j`, **pas** attribuée à un
freinage ou à une chaleur inventés. Le candidat avance aussi le refroidissement
stationnaire et le carburant de ralenti pendant le temps de service. La chaleur
chimique au ralenti n'est pas encore modélisée. La trajectoire d'entrée/sortie du
stand et une conservation complète de tous les flux restent des travaux distincts.

## Audit et frontières de compatibilité

Le résultat expose un `EnergyLedgerV1` agrégé et par tour : travaux moteur/roues,
traînée, roulement, pesanteur, freinage, pertes de transmission/embrayage, chaleur
produite/échangée, temps de roulage/service, masse emportée et remise à zéro au
stand. Les résidus de bilan sont contrôlés à l'échelle des segments et de la course.
Le bilan mécanique de roulage et le bilan des frontières sont séparés afin de ne
pas faire passer une discontinuité de vitesse pour de la dissipation physique.

`resolve_career_physics_inputs_for_audit` résout une configuration de carrière V9
sans calculer de tour. Le banc compare les deux noyaux sur ces mêmes entrées
physiques. Ce résolveur n'accorde aucun droit d'exécution ni accès à un modèle LLM.
L'identité candidate du banc dérive des sources ; elle reste **non enregistrée**.

Les nouveaux champs `None` sont omis de la sérialisation historique. L'exporteur
historique et la projection `PowerTelemetryV1` refusent explicitement les solutions
marquées avec le nouveau modèle. Il faut un futur contrat d'export, une identité
WASM et une admission signée cohérents avant de l'afficher dans le Pit Wall.
Les changements de conduite programmés sont refusés dans ce premier candidat ;
seules les enveloppes fixées et les stratégies d'arrêt sont qualifiées ici.

## Preuves, coût et suite

Le [banc local](../../experiments/longitudinal_energy/README.md) fige les requêtes,
les sources, les binaires, le catalogue et les critères **avant** la qualification.
Il vérifie les sorties canoniques historiques, la répétabilité, les bilans et des
sensibilités. Les solutions analytiques de lancement, pente, freinage et thermique
apportent un oracle distinct des seuls bilans reconstruits depuis le calcul.
Aucune de ces preuves ne remplace une comparaison à des données expérimentales.

Le noyau contraint demande davantage de calcul que l'ancien. Les mesures séparent
le cœur du solveur, le processus complet et son RSS ; elles ne concluent pas au coût
WASM/navigateur ou VPS. Ce coût devra être arbitré avant admission dans le jeu.

Les prochains lots restent : échange thermique et compromis refroidissement/aéro,
pneus thermiques/usure/adhérence, aéro cohérente, puis flux moteur-batterie et
récupération d'énergie. Les paramètres devront être identifiés sur des scénarios
locaux explicites, avec des cas de validation séparés. L'interface générique du
framework et le futur domaine réseau électrique ne reçoivent pas de règles de
course dans cette branche.
