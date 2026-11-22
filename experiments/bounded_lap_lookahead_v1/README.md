# Anticipation bornée aux frontières de tour — candidat natif

Le mode explicite `reachable_wear_v1` complète le wrapper diagnostique haché
`pitgun.lap-boundary-diagnostic/v1`. Aucune construction ordinaire ni runtime
signé ne le sélectionne. Les anciens résultats/contrats restent inchangés.
Le profil thermique et les coefficients physiques ne sont pas recalibrés ici.

## Construction et argument de sûreté sur les lois implémentées

1. La vitesse d'un point physique est bornée par le maximum configuré et le plus
   grand régime maximal converti en vitesse parmi les rapports. La charge normale
   par segment est donc majorée par
   `m_max*(g+v_max²*max(k_vertical,0)) + rho*ClA*v_max²/2`.
   Masse maximale = masse au début du tour courant, supérieure à celle du suivant
   dans le modèle sans ravitaillement.
2. Avec la déformation réduite `k*N*u²`, où le code borne `u` à1, les travaux sont
   majorés par `Wdef<=k*Nmax*ds`, `Wroll<=Crr*Nmax*ds`. Le coefficient effectif
   inclut déjà le coût des corrections du pilote. Ces majorants ne sont jamais
   injectés dans les bilans physiques.
3. Le système thermique à deux capacités est passif avec sources positives :
   température minimale>=min(surface,carcasse,air,route) ; température maximale
   <=max(de ces quatre températures)+chaleur cumulée maximale/min(Cs,Cb).
   Cela reste vrai pour des durées longues : pas d'hypothèse de temps futur cachée.
4. La loi d'usure énergétique/thermique est majorée segment par segment avec ces
   températures et travaux. Son cumul, borné à1, donne un profil d'usure atteignable
   maximal depuis l'état réellement calculé à la fin du tour courant. Pour le grip,
   conserver le plancher thermique positif ; ne pas tenir la température finale
   constante sur le prochain tour. Les profils à plancher<1e-6 sont hors portée.
5. Construire une enveloppe de freinage du tour suivant avec ce profil d'usure,
   le plancher thermique et les contrôles/bruits du prochain tour déjà épinglés.
   Ne compter aucune assistance de freinage issue de la déformation : elle est
   passive mais dépend du grip, et n'est pas une réserve constante.
6. Le fond du prochain horizon est ancré par une enveloppe périodique conservatrice
   à usure1 et utilisation minimale permise par le contrôle fixe/bruit. Cet ancrage
   n'est pas une hypothèse périodique sur l'état physique des pneus. Il protège
   aussi le tour suivant sans dérouler toute la course.

Pourquoi la masse maximale est conservative pour le freinage : pour une vitesse
fixée, `N=max(m*A+B,0)`, `B>=0`, et `F/m` est proportionnel à
`(m*A+B)^(1-lambda)/m`, avec `0<=lambda<=0.3`. Dans la zone de contact positif,
sa dérivée a le signe de `-lambda*m*A-B`, négatif même si `A<0` car alors
`|A|*m<B`. Force latérale par masse et pente sont indépendantes de la masse ;
traînée/roulement et frein mécanique par masse ne deviennent pas plus faibles
quand le carburant diminue. Les racines numériques et la tolérance mécanique
restent celles du Solver. Tests manufacturés et traces réelles vérifient ces
propriétés sur le domaine exploré ; aucune preuve générale d'optimalité du
solveur thermique non linéaire n'est revendiquée.

## Validation avant publication du tour

Commencer avec l'ancrage conservateur, converger le tour courant, puis calculer
le certificat depuis son vrai état final. Autoriser une relaxation de la borne,
recalculer uniquement le travail non commis, et recalculer le certificat depuis
le nouvel état. Accepter seulement si la vitesse finale réellement obtenue est
dans la borne fraîche. Les éventuels resserrements sont bornés ; après quatre
mises à jour, un recalcul conservateur explicitement enregistré peut être essayé.
Toute impossibilité ou non-convergence reste une erreur, jamais un trim de vitesse
ou un relâchement des tolérances. Les compteurs concernent les tentatives, pas le
journal des faits de course. Aucun préfixe publié n'est réécrit.

Le prochain tour vérifie en plus que ses deux températures/usures natives restent
dans l'enclosure annoncée. La fermeture périodique locale ancienne est retirée
uniquement dans ce mode : l'état d'arrivée peut différer de celui de départ.
Les pneus inchangés gardent leurs états ; un arrêt effectivement appliqué remet
à zéro le certificat du train précédent. La remise à zéro abstraite de vitesse
au service, déjà comptée dans les bilans, reste une limite du modèle de stands.

## Qualification locale et portée

`plan-v1.json` fige26scénarios×2modes×2passages. Les huit conditions réservées
n'ont pas été exécutées sous ce nouvel algorithme avant le plan. Elles sont
séparées des cinq pilotes exposés. Tout échec est conservé et ne devient pas
une réussite par ajustement silencieux/requalification du même jeu de validation.

```sh
cargo build --offline --locked --release -p pitgun-racing-simulator \
  --example lap_boundary_probe --example tire_operating_probe \
  --example thermal_driver_probe --example longitudinal_energy_probe \
  --example native_derating_events_probe
python3 experiments/bounded_lap_lookahead_v1/run_local.py screen \
  /CHEMIN/target/release/examples /CHEMIN/qualification
# Après la fin des tests/autres campagnes natives :
python3 experiments/bounded_lap_lookahead_v1/run_local.py cost \
  /CHEMIN/target/release/examples /CHEMIN/qualification
```

Le banc réexécute les23cas de préparation et les régressions historiques du lot
précédent, sans remplacer ses archives. Les coûts portent sur les appels natifs
advance du même binaire : les trajectoires des deux modes diffèrent, ce n'est pas
une optimisation à résultats identiques ni une mesure navigateur/WASM/VPS/RSS.

Le plancher thermique rend encore le certificat prudent. Une vitesse différente
au dernier tour peut être légitime car aucune contrainte de tour suivant n'y
subsiste ; elle ne prouve ni l'absence ni la présence d'un freinage artificiel.
Quantifier les trajectoires et le travail de freinage, conserver les limites
avant tout verdict de qualité ou calibration des adversaires. Pas d'activation
jeu/Worker/WASM, de nouvelle cadence ou de modification d'une preuve existante.

Résultats :[RESULTS_V1.md](RESULTS_V1.md). Le [banc statique](index.html) lit les
mesures natives archivées, sans exécuter le candidat dans le navigateur. La
reconstruction des figures utilise Matplotlib et `plot_report.py`. Les extraits
conservent un point natif tous les5indices sur les500derniers mètres des trois
premiers/deux derniers tours et des tours de service ; métriques et contrôles
conservent tous les points natifs de tous les tours.
