# Politique thermique pneus — qualification native

Le profil explicite prépare la surface à70°C. Initialiser aussi la carcasse à70°C
est une simplification d'état homogène, pas une mesure Pirelli/FIA. Le départ du
banc correspond au montage/mise en mouvement ; pas à une grille réelle après un
tour de formation simulé. Aucun maintien artificiel à70°C en roulage. Les pneus
neufs aux stands sont préparés indépendamment du scénario initial froid/chaud.

Le profil est une hypothèse **moderne slick**, sélectionnée explicitement, jamais
une règle automatique pour toutes les ères ou pour pluie/intermédiaires. Les cas
anciens du plan conservent exactement leurs conditions antérieures. Les entrées
35/90/110°C sont des diagnostics nommés, pas des températures de couvertures.

## Fenêtre et événements

La politique d'événements V2 définit la zone proche du pic par un facteur thermique
de grip>=0.95, indépendamment de l'usure et du multiplicateur propre à la gomme.
Le seuil95% est une convention Pitgun. Avec la loi actuelle :
`Topt ± sigma*sqrt(-ln(0.95))`. Un plancher>=seuil rend la bande finie indéfinie et
est rejeté. Les bornes sont calculées avant arrondi d'affichage en milli°C.
La politique V1 +/-sigma, ses digests et checkpoints gardent leurs anciennes
sémantiques. La nouvelle sélection est explicite et hachée dans les faits ; aucune
ancienne trace n'est réinterprétée. Seuls les états natifs acceptés alimentent les
faits ; les courbes ne déclenchent aucun événement ou appel de modèle.

## Sources et portée d'adoption

- [Pirelli, présentation2022](https://press.pirelli.com/2022-pirelli-motorsport-season-presentation/),
  passage Formula1 : annonce du préchauffage70°C et évolution des fenêtres avec
  la construction des pneus. Justifie une référence moderne, sans transférer ses
  propriétés aux pneus historiques.
- [Pirelli/FIA, Monza2026](https://www.fia.com/system/files/decision-document/2026_italian_grand_prix_-_competition_notes_-_pirelli_preview.pdf),
  document3 du2septembre2026, annexe p2, prescriptions de chauffe du train slick :
  limite externe70°C et durée maximale2h. Le modèle reprend la température de
  montage comme hypothèse, sans simuler couvertures/durée/pression et sans déduire
  une température de carcasse mesurée de cette prescription.
- [Mercedes, Temperature Talk](https://www.mercedesamgf1.com/news/insight-temperature-talk) :
  perte de grip de part et d'autre d'une fenêtre, relation aux conditions et au
  pilotage. Adoption qualitative ; aucune valeur publique de capacité/conductance
  ou de largeur des gommes de Pitgun n'est attribuée à Mercedes.

Sources consultées le22novembre2026. Les optimums90/95/100°C, largeurs38/42/48°C,
plancher0.65 et paramètres thermiques du précédent profil restent des hypothèses
exposées. Aucun recalage sur cet écran ne peut ensuite être nommé validation
indépendante. La littérature justifie les mécanismes ; ces sources ne constituent
pas une identification expérimentale du pneu de Pitgun.

## Exécution

Commiter code/plan/profil avant de mesurer. Compiler les exemples `tire_operating_probe`,
`thermal_driver_probe`, `longitudinal_energy_probe`, `native_derating_events_probe` :

```sh
cargo build --release --offline --locked -p pitgun-racing-simulator \
  --example tire_operating_probe --example thermal_driver_probe \
  --example longitudinal_energy_probe --example native_derating_events_probe
python3 experiments/tyre_operating_policy_v1/run_local.py \
  /CHEMIN/target/release/examples /CHEMIN/qualification-v1
```

Le plan compte23cas,2passages exacts et des régressions historiques séparées.
Les métriques utilisent tous les points natifs. L'occupation des bandes emploie
l'état du début de chaque segment pondéré par sa durée ; ce n'est pas une durée
de franchissement interpolée. Les courbes ne retiennent qu'un point natif chaque
1s pendant les300premières secondes puis10s, plus les extrémités de tours ; elles peuvent lisser visuellement de petites
oscillations. La fréquence du calcul n'est pas réduite. Les températures de
carcasse et surface, l'usure irréversible et le grip effectif restent distincts.

Pas de nouveau modèle publié, carrière/Worker/WASM/Authority ou staging/prod.
Le calcul de bilan existant et les paramètres sont conservés. La qualification
de préparation/événements ne vaut pas équilibre des stratégies ou précision
industrielle. L'anticipation de freinage conservatrice reste le contrôle :#461.
