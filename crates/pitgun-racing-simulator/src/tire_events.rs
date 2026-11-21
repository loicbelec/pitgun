//! Observed tire surface-temperature bands, not inferred punctures or UI alerts.
use crate::{
    RacingSessionEventV1,
    physical_events::{
        DeratingObservationBasisV1, NativePhysicsEventOccurrence, RacingPhysicsEventV1,
    },
};
use pitgun_contract::{Digest, canonical_json_digest};
use pitgun_racing_solver::{ResolvedSimulationLapV3, TireParams};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TireWindowPolicyV1 {
    pub schema_version: TireWindowPolicyVersion,
    /// Diagnostic convention: optimal temperature +/- this multiple of sigma.
    /// This is not a new grip equation or an experimentally identified window.
    pub half_width_sigma_milli: u32,
    pub max_events_per_competitor: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TireWindowPolicyVersion {
    #[serde(rename = "pitgun.racing-tire-window-policy/v1")]
    V1,
}
impl Default for TireWindowPolicyV1 {
    fn default() -> Self {
        Self {
            schema_version: TireWindowPolicyVersion::V1,
            half_width_sigma_milli: 1000,
            max_events_per_competitor: 8192,
        }
    }
}
impl TireWindowPolicyV1 {
    fn validate(&self) -> Result<(), String> {
        if !(100..=3000).contains(&self.half_width_sigma_milli)
            || !(1..=65536).contains(&self.max_events_per_competitor)
        {
            return Err("tire events: invalid policy".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TireWindowEventVersion {
    #[serde(rename = "pitgun.racing-tire-window-event/v1")]
    V1,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TireThermalBandV1 {
    Cold,
    InWindow,
    Hot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TireWindowEventV1 {
    pub schema_version: TireWindowEventVersion,
    pub policy_digest: Digest,
    pub competitor_id: String,
    pub sequence: u64,
    pub lap: u16,
    pub stint: u16,
    pub tire_id: String,
    pub tire_parameters_digest: Digest,
    pub previous_band: Option<TireThermalBandV1>,
    pub band: TireThermalBandV1,
    pub initial_observation: bool,
    pub tire_replaced: bool,
    pub observation_basis: DeratingObservationBasisV1,
    pub surface_temperature_millicelsius: i32,
    pub window_low_millicelsius: i32,
    pub window_high_millicelsius: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TireEventContextV1 {
    pub tire_id: String,
    pub tire: TireParams,
}
impl TireEventContextV1 {
    fn validate(&self, policy: &TireWindowPolicyV1) -> Result<(f64, f64), String> {
        if self.tire_id.is_empty()
            || self.tire_id.len() > 512
            || !self.tire.temp_opt.is_finite()
            || !self.tire.temp_sigma.is_finite()
            || self.tire.temp_sigma <= 0.0
        {
            return Err("tire events: invalid compound context".into());
        }
        if ![
            self.tire.mu_scale,
            self.tire.wear_per_s,
            self.tire.wear_load_k,
            self.tire.wear_grip_k,
            self.tire.wear_min,
            self.tire.temp_min_k,
            self.tire.heat_k,
            self.tire.cool_k,
        ]
        .iter()
        .all(|v| v.is_finite() && *v >= 0.0)
        {
            return Err("tire events: non-finite compound context".into());
        }
        // Validate all serialized context values, including fields outside this band law.
        let width = self.tire.temp_sigma * f64::from(policy.half_width_sigma_milli) / 1000.0;
        let low = self.tire.temp_opt - width;
        let high = self.tire.temp_opt + width;
        milli(low)?;
        milli(high)?;
        Ok((low, high))
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TireDetectorStateVersion {
    #[serde(rename = "pitgun.racing-tire-window-detector-state/v1")]
    V1,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TireWindowCheckpointV1 {
    schema_version: TireDetectorStateVersion,
    policy: TireWindowPolicyV1,
    competitor_id: String,
    next_sequence: u64,
    last_lap: u16,
    last_time_s: f64,
    pending_lap: Option<u16>,
    context: Option<TireEventContextV1>,
    band: Option<TireThermalBandV1>,
    stint: u16,
}
#[derive(Debug, Clone)]
pub struct NativeTireWindowEventDetector {
    state: TireWindowCheckpointV1,
    policy_digest: Digest,
}

fn milli(t: f64) -> Result<i32, String> {
    let t = (t * 1000.0).round();
    if !t.is_finite() || t < i32::MIN as f64 || t > i32::MAX as f64 {
        return Err("tire events: invalid temperature".into());
    }
    Ok(t as i32)
}
impl NativeTireWindowEventDetector {
    pub fn new(competitor_id: String, policy: TireWindowPolicyV1) -> Result<Self, String> {
        Self::restore(TireWindowCheckpointV1 {
            schema_version: TireDetectorStateVersion::V1,
            policy,
            competitor_id,
            next_sequence: 0,
            last_lap: 0,
            last_time_s: 0.0,
            pending_lap: None,
            context: None,
            band: None,
            stint: 0,
        })
    }
    pub fn checkpoint(&self) -> TireWindowCheckpointV1 {
        self.state.clone()
    }
    pub fn restore(state: TireWindowCheckpointV1) -> Result<Self, String> {
        state.policy.validate()?;
        super::physical_events::checked_time(state.last_time_s)?;
        if state.competitor_id.is_empty()
            || state.competitor_id.len() > 512
            || state.next_sequence > u64::from(state.policy.max_events_per_competitor)
            || state.context.is_some() != state.band.is_some()
            || (state.last_lap == 0) != state.context.is_none()
            || state
                .pending_lap
                .is_some_and(|l| l == 0 || l != state.last_lap)
            || state.stint > state.last_lap
            || (state.last_lap == 0 && (state.next_sequence != 0 || state.last_time_s != 0.0))
            || (state.last_lap > 0 && state.next_sequence == 0)
        {
            return Err("tire events: invalid checkpoint".into());
        }
        if let Some(context) = &state.context {
            context.validate(&state.policy)?;
        }
        Ok(Self {
            policy_digest: canonical_json_digest(&state.policy).map_err(|e| e.to_string())?,
            state,
        })
    }
    pub fn observe_driving_lap(
        &mut self,
        lap: &ResolvedSimulationLapV3,
        context: &TireEventContextV1,
    ) -> Result<Vec<NativePhysicsEventOccurrence>, String> {
        let s = &lap.solution;
        let n = s.t.len();
        if self.state.pending_lap.is_some()
            || self.state.last_lap.checked_add(1) != Some(lap.lap_index)
            || n < 2
            || s.tire_temp.len() != n
            || s.lap_index.len() != n
            || s.lap_index.iter().any(|l| *l != lap.lap_index)
            || s.t[0] != 0.0
            || s.t.windows(2).any(|w| w[1] <= w[0])
            || self.state.context.as_ref().is_some_and(|c| c != context)
        {
            return Err("tire events: invalid driving trace or uncommitted tire change".into());
        }
        let mut next = self.clone();
        let mut events = Vec::new();
        for (i, (&time, &temp)) in s.t.iter().zip(&s.tire_temp).enumerate() {
            super::physical_events::checked_time(time)?;
            next.observe(
                lap.lap_index,
                context,
                temp,
                self.state.last_time_s + time,
                DeratingObservationBasisV1::NativePoint {
                    index: u32::try_from(i).map_err(|_| "tire events: point overflow")?,
                },
                false,
                &mut events,
            )?;
        }
        let end = self.state.last_time_s + s.t[n - 1];
        super::physical_events::checked_time(lap.cumulative_time_s)?;
        if lap.cumulative_time_s + 1e-9 < end {
            return Err("tire events: lap before driving end".into());
        }
        next.state.last_lap = lap.lap_index;
        next.state.last_time_s = end;
        next.state.pending_lap = Some(lap.lap_index);
        *self = next;
        Ok(events)
    }
    pub fn observe_lap_boundary(
        &mut self,
        lap: &ResolvedSimulationLapV3,
        after: &TireEventContextV1,
    ) -> Result<Vec<NativePhysicsEventOccurrence>, String> {
        if self.state.pending_lap != Some(lap.lap_index)
            || lap.cumulative_time_s < self.state.last_time_s
            || (!lap.pit_stop_completed && self.state.context.as_ref() != Some(after))
        {
            return Err("tire events: invalid service boundary".into());
        }
        let mut next = self.clone();
        if lap.pit_stop_completed {
            next.state.stint = next
                .state
                .stint
                .checked_add(1)
                .ok_or("tire events: stint overflow")?;
        }
        let mut events = Vec::new();
        next.observe(
            lap.lap_index,
            after,
            lap.state_after_lap.tire_temp,
            lap.cumulative_time_s,
            DeratingObservationBasisV1::ServiceBoundary,
            lap.pit_stop_completed,
            &mut events,
        )?;
        next.state.last_time_s = lap.cumulative_time_s;
        next.state.pending_lap = None;
        *self = next;
        Ok(events)
    }
    #[allow(clippy::too_many_arguments)]
    fn observe(
        &mut self,
        lap: u16,
        context: &TireEventContextV1,
        temperature: f64,
        time: f64,
        basis: DeratingObservationBasisV1,
        replaced: bool,
        events: &mut Vec<NativePhysicsEventOccurrence>,
    ) -> Result<(), String> {
        let occurred_at_us = super::physical_events::checked_time(time)?;
        let (low, high) = context.validate(&self.state.policy)?;
        let temp = milli(temperature)?;
        let band = if temperature < low {
            TireThermalBandV1::Cold
        } else if temperature > high {
            TireThermalBandV1::Hot
        } else {
            TireThermalBandV1::InWindow
        };
        let initial = self.state.context.is_none();
        if !initial && !replaced && self.state.band == Some(band) {
            return Ok(());
        }
        let sequence = self.state.next_sequence;
        if sequence >= u64::from(self.state.policy.max_events_per_competitor) {
            return Err("tire events: record budget exhausted".into());
        }
        events.push(NativePhysicsEventOccurrence {
            occurred_at_us,
            event: RacingPhysicsEventV1::TireWindow(TireWindowEventV1 {
                schema_version: TireWindowEventVersion::V1,
                policy_digest: self.policy_digest,
                competitor_id: self.state.competitor_id.clone(),
                sequence,
                lap,
                stint: self.state.stint,
                tire_id: context.tire_id.clone(),
                tire_parameters_digest: canonical_json_digest(&context.tire)
                    .map_err(|e| e.to_string())?,
                previous_band: if replaced { None } else { self.state.band },
                band,
                initial_observation: initial,
                tire_replaced: replaced,
                observation_basis: basis,
                surface_temperature_millicelsius: temp,
                window_low_millicelsius: milli(low)?,
                window_high_millicelsius: milli(high)?,
            }),
        });
        self.state.next_sequence += 1;
        self.state.context = Some(context.clone());
        self.state.band = Some(band);
        Ok(())
    }
}

/// Stable tie-breaking within a physical-event family; the surrounding run scopes IDs.
pub(crate) fn event_order(event: &RacingSessionEventV1) -> Option<(&str, u8, u64)> {
    match event {
        RacingSessionEventV1::Physical {
            event: RacingPhysicsEventV1::EngineDerating(e),
            ..
        } => Some((&e.competitor_id, 0, e.sequence)),
        RacingSessionEventV1::Physical {
            event: RacingPhysicsEventV1::TireWindow(e),
            ..
        } => Some((&e.competitor_id, 1, e.sequence)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pitgun_racing_solver::{DriverControlParamsV3, SimulationSolution, VehicleState};
    fn context(id: &str, opt: f64) -> TireEventContextV1 {
        TireEventContextV1 {
            tire_id: id.into(),
            tire: TireParams {
                mu_scale: 1.0,
                wear_per_s: 0.0,
                wear_load_k: 0.0,
                wear_grip_k: 0.0,
                wear_min: 0.5,
                heat_k: 0.0,
                cool_k: 0.0,
                temp_opt: opt,
                temp_sigma: 10.0,
                temp_min_k: 0.5,
            },
        }
    }
    fn lap(index: u16, offset: f64, temps: &[f64]) -> ResolvedSimulationLapV3 {
        ResolvedSimulationLapV3 {
            thermal_v2: None,
            longitudinal_energy_v1: None,
            lap_index: index,
            lap_time_s: (temps.len() - 1) as f64,
            cumulative_time_s: offset + (temps.len() - 1) as f64,
            cumulative_distance_m: 100.0 * f64::from(index),
            state_after_lap: VehicleState {
                fuel_mass: 10.0,
                tire_wear: 0.1,
                tire_temp: *temps.last().unwrap(),
                engine_temp: 90.0,
            },
            end_speed_mps: None,
            end_gear: None,
            shift_time_remaining_s: 0.0,
            active_driver_control: DriverControlParamsV3::default(),
            correction_workload_multiplier: None,
            pit_stop_completed: false,
            solution: SimulationSolution {
                t: (0..temps.len()).map(|i| i as f64).collect(),
                tire_temp: temps.to_vec(),
                lap_index: vec![index; temps.len()],
                ..Default::default()
            },
        }
    }
    fn event(e: &NativePhysicsEventOccurrence) -> &TireWindowEventV1 {
        let RacingPhysicsEventV1::TireWindow(e) = &e.event else {
            panic!("tire event expected")
        };
        e
    }
    #[test]
    fn native_crossings_include_both_directions_and_keep_inclusive_window_edges() {
        let mut d =
            NativeTireWindowEventDetector::new("player".into(), Default::default()).unwrap();
        let c = context("medium", 100.0);
        let l = lap(1, 0.0, &[80.0, 90.0, 110.0, 120.0, 100.0, 85.0]);
        let facts = d.observe_driving_lap(&l, &c).unwrap();
        assert_eq!(
            facts.iter().map(|e| event(e).band).collect::<Vec<_>>(),
            [
                TireThermalBandV1::Cold,
                TireThermalBandV1::InWindow,
                TireThermalBandV1::Hot,
                TireThermalBandV1::InWindow,
                TireThermalBandV1::Cold
            ]
        );
        assert!(event(&facts[0]).initial_observation);
        assert_eq!(facts[2].occurred_at_us, 3_000_000);
        assert!(d.observe_lap_boundary(&l, &c).unwrap().is_empty());
        assert!(
            d.observe_driving_lap(&lap(2, 5.0, &[85.0, 80.0]), &c)
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn a_same_compound_replacement_is_a_new_stint_not_recovery_of_old_tires() {
        let mut d =
            NativeTireWindowEventDetector::new("player".into(), Default::default()).unwrap();
        let c = context("medium", 100.0);
        let mut l = lap(1, 0.0, &[120.0, 125.0]);
        d.observe_driving_lap(&l, &c).unwrap();
        let serialized = serde_json::to_vec(&d.checkpoint()).unwrap();
        d = NativeTireWindowEventDetector::restore(serde_json::from_slice(&serialized).unwrap())
            .unwrap();
        l.pit_stop_completed = true;
        l.cumulative_time_s += 20.0;
        l.state_after_lap.tire_temp = 80.0;
        let facts = d.observe_lap_boundary(&l, &c).unwrap();
        let e = event(&facts[0]);
        assert_eq!(e.stint, 1);
        assert!(e.tire_replaced);
        assert!(!e.initial_observation);
        assert_eq!(e.previous_band, None);
        assert_eq!(facts[0].occurred_at_us, 21_000_000);
        assert!(
            d.observe_driving_lap(&lap(2, 21.0, &[80.0, 85.0]), &c)
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn replacement_uses_new_compound_window_and_records_same_band_again() {
        let mut d =
            NativeTireWindowEventDetector::new("player".into(), Default::default()).unwrap();
        let mut l = lap(1, 0.0, &[100.0, 100.0]);
        d.observe_driving_lap(&l, &context("medium", 100.0))
            .unwrap();
        l.pit_stop_completed = true;
        let facts = d.observe_lap_boundary(&l, &context("hard", 105.0)).unwrap();
        assert_eq!(event(&facts[0]).window_low_millicelsius, 95000);
        assert_eq!(event(&facts[0]).band, TireThermalBandV1::InWindow);
        assert_eq!(event(&facts[0]).tire_id, "hard");
    }
    #[test]
    fn malformed_traces_bounds_and_checkpoints_fail_atomically() {
        let c = context("medium", 100.0);
        for invalid in 0..4 {
            let mut d =
                NativeTireWindowEventDetector::new("player".into(), Default::default()).unwrap();
            let before = d.checkpoint();
            let mut l = lap(1, 0.0, &[80.0, 100.0, 120.0]);
            match invalid {
                0 => l.solution.tire_temp[2] = f64::NAN,
                1 => l.solution.t[1] = f64::NAN,
                2 => l.solution.t[2] = l.solution.t[1],
                _ => l.cumulative_time_s = 0.0,
            };
            assert!(d.observe_driving_lap(&l, &c).is_err());
            assert_eq!(d.checkpoint(), before);
        }
        let mut d = NativeTireWindowEventDetector::new(
            "player".into(),
            TireWindowPolicyV1 {
                max_events_per_competitor: 1,
                ..Default::default()
            },
        )
        .unwrap();
        let before = d.checkpoint();
        assert!(
            d.observe_driving_lap(&lap(1, 0.0, &[80.0, 100.0]), &c)
                .is_err()
        );
        assert_eq!(d.checkpoint(), before);
        let mut bad = before;
        bad.band = Some(TireThermalBandV1::Hot);
        assert!(NativeTireWindowEventDetector::restore(bad).is_err());
    }
    #[test]
    fn changed_context_without_service_and_repeated_boundaries_are_rejected() {
        let c = context("medium", 100.0);
        let mut d =
            NativeTireWindowEventDetector::new("player".into(), Default::default()).unwrap();
        let l = lap(1, 0.0, &[80.0, 85.0]);
        d.observe_driving_lap(&l, &c).unwrap();
        let before = d.checkpoint();
        assert!(d.observe_lap_boundary(&l, &context("hard", 105.0)).is_err());
        assert_eq!(before, d.checkpoint());
        d.observe_lap_boundary(&l, &c).unwrap();
        assert!(d.observe_lap_boundary(&l, &c).is_err());
        assert!(
            d.observe_driving_lap(&lap(2, 1.0, &[85.0, 90.0]), &context("hard", 105.0))
                .is_err()
        );
    }
}
