# Energy transmission redline roundoff correction

Owner playtest caught a real physical lock in the I/II candidate: the energy integrator caps speed at redline, but converting the cap back to RPM can yield 10,999.999999999998 instead of the authored 11,000 upshift threshold. Exact comparison then keeps first gear forever. The telemetry reports the actual solver state; this is not a display-only defect.

The energy-only selector now accepts representational equality within `8 * f64::EPSILON * max(abs(rpm), upshift_rpm)` (about 2e-11 RPM here). It does not move the authored shift point to a different physical RPM. Adjacent selection, shift interruption, maximum gear and downshift behavior remain. The legacy non-energy selector is unchanged. Focused checks cover the observed round trip, refusal 0.001 RPM below threshold, active shift lock, all adjacent shifts and top gear.

One identical Monza I player lap (sealed reference setup, medium tire) changed from first gear / 163.1868 km/h / 153.1447 s to maximum third gear / 229.4534 km/h / 142.9712 s. These are a single-case diagnostic, not optimized shifts, car validation or renewed AI calibration. Retained before/after reports identify the exact original inputs through qualified_gear_audit.rs and the sealed native reference file.

Model 0.16.1 and catalog 1.10.1 distinguish the correction. Earlier model/catalog/artifact/source and the active owner preview remain retained. The profile explicitly marks inherited fixed-nine preparations as awaiting requalification. No prior outcome digest, timing reference or campaign report is relabeled as passing the corrected model. A native/WASM tolerance diagnostic of the old model is not an acceptance oracle for this model.

The previous 24,966-GP campaign remains evidence of its exact old executable, not validation of corrected transmission balancing. Requalify physics/setup/one-two-stop feasibility with a bounded matrix first, then recalibrate one era at a time. Do not immediately rerun the entire campaign, publish the catalog, promote staging or admit the candidate to a leaderboard cohort.

## Corrective checks

613 workspace tests pass (22 existing ignored), including the actual round-trip scalar regression and all legacy golden tests. Strict affected-library Clippy and formatting pass. The paired game has 775 passing tests plus lint/build. Isolated Chrome Monza-I FP1/FP2/FP3 and full 53-lap GP each expose actual sim.gear values 1/2/3, finish all ten cars and independently verify their proofs; result/HQ return passes without browser exceptions. This is not renewed all-era/all-doctrine/AI-competitiveness qualification. Provider/publication remain disabled; the old owner preview stays intact.

The compiled source is 3f1667cbb2bf79f13650b1b3a639a0e7cd11e290, runtime sha256:ea7ac619f54098ef79190bbc93ecd5ab5a50dd36753768e6a48ef0c5c82c3b88, model 0.16.1/catalog1.10.1. The existing calibration cache pins both binary digest and source digests and fails closed on changed native code. A corrected campaign must use a new namespace, not reuse old same-input hits.
