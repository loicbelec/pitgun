# Plateau fixe de neuf IA — deuxième calibration native

**Campagne arrêtée sur instruction propriétaire avant sélection/réserve.** Le
jeu doit autoriser au plus deux arrêts volontaires. L'espace élargi révèle un
optimum échantillonné à 5–8 arrêts : ces résultats sont des diagnostics, pas une
politique à intégrer. Les 4 500 stratégies et 690 adaptations individuelles
terminées sont conservées ; aucun résultat réservé n'a été ouvert. Voir la
[décision d'arrêt](results/owner-stop-v2.json) et
l'[audit causal des pneus/stands](../pit_strategy_audit_v1/README.md).

La [première campagne](../fixed_opponents_v1/README.md) est conservée comme
résultat négatif : aucune des 27 calibrations ne permettait une victoire du
joueur informé en sortie d'ère I/II, malgré la bonne hiérarchie des favoris.
Aucun résultat réservé n'a été ouvert. Il n'y a pas de correction des chronos.

Cette version abaisse de **0,06 les trois capacités de chacun des neuf pilotes**,
en préservant leurs écarts relatifs et leurs préférences. Le joueur reste à
`.84/.84/.84`, Balanced ; la physique, les ressources véhicule, les budgets
candidats et les critères de sélection restent inchangés. Les trois niveaux
globaux des deux favoris sont de nouveau évalués sur l'entraînement uniquement.
Le plateau final est figé avant les essais réservés, sans adaptation à la
performance du joueur. Les identités/capacités sont constantes entre circuits.

La recherche de stratégies conserve les 222 plans précédents et ajoute des plans
à 4, 5, 6, 7, 8, 12 arrêts ainsi que la borne légale d'un arrêt par tour. Douze
familles de composés et trois répartitions temporelles sont dédupliquées. Cette
extension vérifie la borne haute de trois arrêts ; elle ne constitue pas une
optimisation exhaustive de toutes les séquences et de toutes les dates d'arrêt.

Les calculs V1 du pilote de référence et du joueur sont physiquement inchangés.
L'[importeur](import_controls.py) recalcule un GP témoin dans chacun des dix
contextes et dans les quatre étapes de carrière du joueur. Il exige l'égalité
exacte de toute la sortie, hormis les deux identités d'enveloppe qui changent :
`binary_digest` et `receipt.roster_digest`. Les sources, entrées, résultats et
enveloppes avant/après sont enregistrés. **Aucun résultat des neuf pilotes n'est
importé** : leurs capacités changent et ils sont tous recalculés. Les données
réutilisées ne sont pas comptées comme de nouveaux calculs.

Le [plan figé](plan-v2.json) et le runner reprennent la méthode V1 : GP complets,
budgets fixes par ère, vrais développements du joueur, sélection puis validation
réservée, répétitions fraîches, assemblage de dix voitures et invariance à l'ordre
des pilotes et aux réglages du joueur. Seules les ères I/II sont qualifiées ici.

```sh
cargo build --release --offline --locked -p pitgun-racing-simulator --example fixed_field_probe_v2
python3 experiments/fixed_opponents_v2/qualify.py target/release/examples/fixed_field_probe_v2
python3 experiments/fixed_opponents_v2/import_controls.py target/release/examples/fixed_field_probe_v2 /private/tmp/pitgun-fixed-field-v2-cache
python3 experiments/fixed_opponents_v2/run_local.py --preview
python3 experiments/fixed_opponents_v2/run_local.py target/release/examples/fixed_field_probe_v2 /private/tmp/pitgun-fixed-field-v2-cache
python3 experiments/fixed_opponents_v2/summarize.py
```

Adapter `CARGO_TARGET_DIR` au répertoire local de compilation. Ne jamais
recompiler le binaire ni modifier les sources épinglées pendant la campagne.
Le résultat reste un candidat natif, sans modification du jeu, de WASM,
d'Authority/Verifier, des catalogues admis, du staging ou de la production.
