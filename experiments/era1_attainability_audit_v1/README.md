# Era-I attainable player preparation audit

Current delivery: freeze and launch **one bounded batch** of the approved era-I audit. [Protocol](ACCEPTANCE.md), [plan](plan-v1.json). No local-game, physical-model, driver-trait, catalog or deployed-service changes.

- Three regular circuits x caps8/10/12 x three preparation families x six proposals =162logical comparisons.
- Fifteen exact id-only aliases removed:147unique player GPs +6fresh repeats =153maximum fresh solves, below the approved168ceiling.
- Eighty-one frozen rival GPs reused with exact typed-case equality, native resolve-only physical-input bridges, matching executable/policy/source artifact identity and explicit source-case/result references. Reuse is not counted as new compute.
- Caps8/12 are actual entry/exit fixtures; cap10 is explicitly controlled. Heavy80is defined on discretionary points after HQ floors, and may legitimately win. No forced all-axis spending or universal cooling preference.
- Default20new GP or300seconds per invocation, checked between cases. The program returns progress without a final audit verdict until the full matrix is finished. Run the same command later to resume; do not automatically chain batches.

Command from the committed research worktree:

```sh
python3 experiments/era1_attainability_audit_v1/run.py \
  --binary /ABSOLUTE/RETAINED/fixed_nine_budget_probe \
  --cache /PRIVATE/SHARED_PARENT/era1-attainability-v1 \
  --output experiments/era1_attainability_audit_v1/results/era1-v1.json.gz
```

Use the existing V1/pilot native-worker parent so the lock is shared. Preserve the executable hash from plan-v1.json and all frozen sources. The runner exports an era-I-specific schema and field provenance when complete; partial progress stays in its private cache. A changed program/source/matrix requires a new namespace, never relabeling old evidence. Tests: `python3 -m unittest discover -s experiments/era1_attainability_audit_v1 -v`; shared journal/batch tests remain in `experiments/fixed_nine_refinement_v2`.

Next after complete development review: era-II same-opportunity named-driver preparation, genuinely new reserved evaluation after a complete policy freeze, native/WASM/recovery/performance and versioned local admission. Later-eraIII–V scope remains #492. The owner-facing preview is unchanged by this offline audit.
