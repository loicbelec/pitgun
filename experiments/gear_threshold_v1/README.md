# Energy transmission redline roundoff correction

Owner playtest caught a real physical lock in the I/II candidate: the energy integrator caps speed at redline, but converting the cap back to RPM can yield 10,999.999999999998 instead of the authored 11,000 upshift threshold. Exact comparison then keeps first gear forever. The telemetry reports the actual solver state; this is not a display-only defect.

The energy-only selector now accepts representational equality within `8 * f64::EPSILON * max(abs(rpm), upshift_rpm)` (about 2e-11 RPM here). It does not move the authored shift point to a different physical RPM. Adjacent selection, shift interruption, maximum gear and downshift behavior remain. The legacy non-energy selector is unchanged. Focused checks cover the observed round trip, refusal 0.001 RPM below threshold, active shift lock, all adjacent shifts and top gear.

One identical Monza I player lap (sealed reference setup, medium tire) changed from first gear / 163.1868 km/h / 153.1447 s to maximum third gear / 229.4534 km/h / 142.9712 s. These are a single-case diagnostic, not optimized shifts, car validation or renewed AI calibration. Retained before/after reports identify the exact original inputs through qualified_gear_audit.rs and the sealed native reference file.

Model 0.16.1 and catalog 1.10.1 distinguish the correction. Earlier model/catalog/artifact/source and the active owner preview remain retained. The profile explicitly marks inherited fixed-nine preparations as awaiting requalification. No prior outcome digest, timing reference or campaign report is relabeled as passing the corrected model. A native/WASM tolerance diagnostic of the old model is not an acceptance oracle for this model.

The previous 24,966-GP campaign remains evidence of its exact old executable, not validation of corrected transmission balancing. Requalify physics/setup/one-two-stop feasibility with a bounded matrix first, then recalibrate one era at a time. Do not immediately rerun the entire campaign, publish the catalog, promote staging or admit the candidate to a leaderboard cohort.
