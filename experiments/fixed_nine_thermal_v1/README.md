# Budget-aware fixed nine on the selected thermal candidate

Issue #490 under #457. Native offline calibration; the game preview is not changed by this directory. The frozen protocol is [ACCEPTANCE.md](ACCEPTANCE.md); candidate inputs and retained archive hashes are in [plan-v1.json](plan-v1.json). Physics and the nine existing driver resources stay unchanged.

Build `fixed_nine_budget_probe` with the shared release target and two build jobs. Run `run.py --phase training --binary PATH --cache PRIVATE_CACHE/training --output results/training-v1.json.gz`. A passing training selection enables `--phase validation --selection results/selection-v1.json --training results/training-v1.json.gz` with a distinct validation cache/output. The runner is serial, resumable and rejects changed executable/source/policy identities. Do not tune from reserved results.

`policy.py` provides deterministic rational interpolation and exact integer point projection from a pre-weekend available effective cap. `report.py` audits counts/digests/receipts and renders results. Tests: `python3 -m unittest discover -s experiments/fixed_nine_thermal_v1 -v` and the Rust example tests in release mode. No deployed catalog/workload selects this research policy.
