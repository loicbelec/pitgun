//! Explicit power-channel policy. Legacy frames are never reinterpreted.
use crate::{
    FuelMassParamsV3, MechanicalParamsV3, ResolvedSimulationLapV3, VehicleParams, gateway_sample,
};
use pitgun_contract::{ArtifactIdentity, TelemetryFrame, canonical_json_digest};
use pitgun_racing_solver::power_telemetry::{PowerPointV1, project_lap_power_v1};

pub const POWER_CHANNELS: [(u16, &str, &str); 7] = [
    (5100, "sim.engine_load_pct", "%"),
    (5101, "sim.engine_available_w", "W"),
    (5102, "sim.engine_delivered_w", "W"),
    (5103, "sim.wheel_propulsion_w", "W"),
    (5104, "sim.driveline_loss_w", "W"),
    (5105, "sim.launch_regularization_w", "W"),
    (5106, "sim.drive_budget_w", "W"),
];

pub fn power_telemetry_policy_identity() -> ArtifactIdentity {
    let policy = serde_json::json!({
        "schema":"pitgun.racing-power-channel-policy/v1", "channels":POWER_CHANNELS,
        "source":"native-lap-power-telemetry/v1", "sampling":"causal-hold-at-native-export-times",
        "terminal":"causal-hold-of-emitted-power-rows-within-driving-intervals",
        "absent_commands":[5008,5009],"replaced_legacy_power":5014,
    });
    ArtifactIdentity {
        id: "pitgun.racing-power-channel-policy".parse().unwrap(),
        version: "1.0.0".parse().unwrap(),
        digest: canonical_json_digest(&policy).expect("finite static power policy"),
    }
}

/// Compact exported rows, not a second native history or full frame archive.
pub(crate) struct PowerLapCache {
    start_us: i64,
    end_us: i64,
    rows: Vec<(i64, PowerPointV1)>,
}
impl PowerLapCache {
    pub(crate) fn from_lap(
        lap: &ResolvedSimulationLapV3,
        vehicle: &VehicleParams,
        mechanical: MechanicalParamsV3,
        fuel: Option<FuelMassParamsV3>,
        times: &[f64],
        offset: f64,
    ) -> Result<Self, String> {
        let projection = project_lap_power_v1(&lap.solution, vehicle, mechanical, fuel)?;
        let us = |t: f64| ((t + offset) * 1e6).round() as i64;
        let start_us = us(*lap.solution.t.first().ok_or("missing power lap start")?);
        let end_us = us(*lap.solution.t.last().ok_or("missing power lap end")?);
        let rows = projection
            .sample_at_times(times)?
            .into_iter()
            .map(|p| (us(p.time_s), p))
            .collect();
        Ok(Self {
            start_us,
            end_us,
            rows,
        })
    }
    fn at(&self, time: i64) -> Option<&PowerPointV1> {
        if time < self.start_us || time >= self.end_us {
            return None;
        }
        let index = self.rows.partition_point(|(t, _)| *t <= time);
        index.checked_sub(1).map(|i| &self.rows[i].1)
    }
}

pub(crate) fn apply_power_frames(frames: &mut [TelemetryFrame], caches: &[PowerLapCache]) {
    let policy = power_telemetry_policy_identity();
    for frame in frames {
        frame
            .samples
            .retain(|s| ![5008, 5009, 5014].contains(&s.parameter_id));
        frame
            .metadata
            .insert("power_telemetry_policy".into(), policy.digest.to_string());
        frame.metadata.insert(
            "power_telemetry_version".into(),
            "pitgun.racing-power-channel-policy/v1".into(),
        );
        let index = caches.partition_point(|c| c.start_us <= frame.timestamp_us);
        let point = index
            .checked_sub(1)
            .and_then(|i| caches[i].at(frame.timestamp_us));
        if let Some(p) = point {
            let values = [
                p.equivalent_engine_load_pct,
                p.engine_available_w,
                p.engine_delivered_w,
                p.wheel_propulsion_w,
                p.driveline_loss_w,
                p.launch_regularization_w,
                p.drive_budget_w,
            ];
            frame.samples.extend(
                POWER_CHANNELS
                    .iter()
                    .zip(values)
                    .map(|((id, _, _), v)| gateway_sample(*id, v)),
            );
            frame.samples.sort_by_key(|s| s.parameter_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn terminal_sampling_does_not_leak_power_into_a_pit_gap_or_a_future_lap() {
        let p = PowerPointV1 {
            time_s: 0.0,
            engine_available_w: 100.0,
            engine_delivered_w: 80.0,
            drive_budget_w: 72.0,
            wheel_propulsion_w: 72.0,
            driveline_loss_w: 8.0,
            launch_regularization_w: 0.0,
            equivalent_engine_load_pct: 80.0,
            shift_power_fraction: 1.0,
        };
        let cache = PowerLapCache {
            start_us: 10,
            end_us: 30,
            rows: vec![(10, p.clone()), (20, p)],
        };
        assert!(cache.at(9).is_none());
        assert!(cache.at(10).is_some());
        assert!(cache.at(29).is_some());
        assert!(cache.at(30).is_none());
        assert!(cache.at(50).is_none());
    }
}
