//! Opt-in native physical facts. No UI relevance thresholds or controller calls.
//! Consume committed native states only; never invoke from numerical iterations.
use crate::{RacingSessionEventV1, physical_time_us};
use pitgun_contract::{Digest, canonical_json_digest};
use pitgun_racing_solver::{
    EngineParams, EngineThermalParamsV3, ResolvedSimulationLapV3, derating_factor_v3,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeratingEventPolicyVersion {
    #[serde(rename = "pitgun.racing-derating-event-policy/v1")]
    V1,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeratingEventPolicyV1 {
    pub schema_version: DeratingEventPolicyVersion,
    /// Explicit audit bound, not a UI message quota. Overflow fails, never drops facts.
    pub max_events_per_competitor: u32,
}
impl Default for DeratingEventPolicyV1 {
    fn default() -> Self {
        Self {
            schema_version: DeratingEventPolicyVersion::V1,
            max_events_per_competitor: 8192,
        }
    }
}
impl DeratingEventPolicyV1 {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=65536).contains(&self.max_events_per_competitor) {
            return Err("physical events: invalid record bound".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RacingPhysicsEventVersion {
    #[serde(rename = "pitgun.racing-physics-event/v1")]
    V1,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeratingTransitionV1 {
    Started,
    Recovered,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DeratingObservationBasisV1 {
    /// State at a native point. The last point denotes end-of-driving capability,
    /// not the duplicated last applied segment factor in the solution arrays.
    NativePoint { index: u32 },
    /// First known committed post-service state; not an interpolated crossing time.
    ServiceBoundary,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineDeratingEventV1 {
    pub schema_version: RacingPhysicsEventVersion,
    pub policy_digest: Digest,
    pub competitor_id: String,
    pub sequence: u64,
    pub lap: u16,
    pub transition: DeratingTransitionV1,
    pub observation_basis: DeratingObservationBasisV1,
    /// Start sequence of this episode, also carried by its recovery.
    pub episode_start_sequence: u64,
    /// An active state at the very first observation is not a known prior crossing.
    pub initial_observation: bool,
    pub engine_temperature_millicelsius: i32,
    /// Rounded for transport only. State transitions use native factor < 1.0.
    /// Availability is not delivered shaft power.
    pub available_power_fraction_ppm: u32,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "payload",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum RacingPhysicsEventV1 {
    EngineDerating(EngineDeratingEventV1),
    TireWindow(crate::tire_events::TireWindowEventV1),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeratingDetectorStateVersion {
    #[serde(rename = "pitgun.racing-derating-detector-state/v1")]
    V1,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeratingDetectorCheckpointV1 {
    schema_version: DeratingDetectorStateVersion,
    policy: DeratingEventPolicyV1,
    competitor_id: String,
    next_sequence: u64,
    last_lap: u16,
    last_time_s: f64,
    pending_boundary_lap: Option<u16>,
    active_episode: Option<u64>,
    observed_any: bool,
}
#[derive(Debug, Clone)]
pub struct NativeDeratingEventDetector {
    state: DeratingDetectorCheckpointV1,
    policy_digest: Digest,
}
#[derive(Debug, Clone, PartialEq)]
pub struct NativePhysicsEventOccurrence {
    pub occurred_at_us: u64,
    pub event: RacingPhysicsEventV1,
}
impl NativePhysicsEventOccurrence {
    pub fn into_unreleased(self) -> (u64, RacingSessionEventV1) {
        (
            self.occurred_at_us,
            RacingSessionEventV1::Physical {
                event: self.event,
                timing: None,
            },
        )
    }
}

pub(crate) fn checked_time(time_s: f64) -> Result<u64, String> {
    if !time_s.is_finite() || time_s < 0.0 || time_s >= u64::MAX as f64 / 1e6 {
        return Err("physical events: invalid simulation time".into());
    }
    Ok(physical_time_us(time_s))
}

fn validate_guard(engine: &EngineParams, guard: EngineThermalParamsV3) -> Result<(), String> {
    guard.validate()?;
    if !engine.t_soft.is_finite() || !engine.beta_derate.is_finite() || engine.beta_derate < 0.0 {
        return Err("physical events: invalid engine thermal guard".into());
    }
    Ok(())
}

impl NativeDeratingEventDetector {
    pub fn new(competitor_id: String, policy: DeratingEventPolicyV1) -> Result<Self, String> {
        Self::restore(DeratingDetectorCheckpointV1 {
            schema_version: DeratingDetectorStateVersion::V1,
            policy,
            competitor_id,
            next_sequence: 0,
            last_lap: 0,
            last_time_s: 0.0,
            pending_boundary_lap: None,
            active_episode: None,
            observed_any: false,
        })
    }
    /// Detector continuity only; the host must separately persist pending released
    /// records and physical/control state. This is not an Authority capability.
    pub fn checkpoint(&self) -> DeratingDetectorCheckpointV1 {
        self.state.clone()
    }
    pub fn restore(state: DeratingDetectorCheckpointV1) -> Result<Self, String> {
        state.policy.validate()?;
        checked_time(state.last_time_s)?;
        if state.competitor_id.is_empty()
            || state.competitor_id.len() > 512
            || state.next_sequence > u64::from(state.policy.max_events_per_competitor)
            || state
                .active_episode
                .is_some_and(|seq| seq >= state.next_sequence)
            || state
                .pending_boundary_lap
                .is_some_and(|lap| lap == 0 || lap != state.last_lap)
            || (state.observed_any != (state.last_lap > 0))
            || (!state.observed_any
                && (state.next_sequence != 0
                    || state.last_time_s != 0.0
                    || state.active_episode.is_some()))
        {
            return Err("physical events: invalid detector checkpoint".into());
        }
        let policy_digest = canonical_json_digest(&state.policy).map_err(|e| e.to_string())?;
        Ok(Self {
            state,
            policy_digest,
        })
    }

    /// Observe exactly one accepted driving trace, before or after service.
    /// It remains open until observe_lap_boundary commits the end state. Calls
    /// validate atomically: invalid/stale input leaves detector continuity intact.
    pub fn observe_driving_lap(
        &mut self,
        lap: &ResolvedSimulationLapV3,
        engine: &EngineParams,
        guard: EngineThermalParamsV3,
    ) -> Result<Vec<NativePhysicsEventOccurrence>, String> {
        let mut next = self.clone();
        let s = &lap.solution;
        let n = s.t.len();
        if next.state.pending_boundary_lap.is_some()
            || next.state.last_lap.checked_add(1) != Some(lap.lap_index)
            || n < 2
            || s.temp.len() != n
            || s.engine_derating_factor.len() != n
            || s.engine_derating_factor
                .iter()
                .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
            || s.t[0] != 0.0
            || s.t.iter().any(|t| !t.is_finite() || *t < 0.0)
            || s.t.windows(2).any(|w| w[1] <= w[0])
            || s.lap_index.len() != n
            || s.lap_index.iter().any(|l| *l != lap.lap_index)
        {
            return Err("physical events: invalid or out-of-order native lap".into());
        }
        validate_guard(engine, guard)?;
        let offset = next.state.last_time_s;
        let crossing = offset + s.t[n - 1];
        checked_time(crossing)?;
        checked_time(lap.cumulative_time_s)?;
        if lap.cumulative_time_s + 1e-9 < crossing {
            return Err("physical events: lap ends before driving".into());
        }
        let mut events = Vec::new();
        for i in 0..n {
            let factor = derating_factor_v3(s.temp[i], engine, guard);
            // The terminal factor array repeats the preceding segment. Its
            // accepted temperature is nevertheless an observed endpoint state.
            if i < n - 1
                && (!s.engine_derating_factor[i].is_finite()
                    || (factor - s.engine_derating_factor[i]).abs() > 1e-12)
            {
                return Err(
                    "physical events: native derating disagrees with bound engine guard".into(),
                );
            }
            next.observe_point(
                lap.lap_index,
                DeratingObservationBasisV1::NativePoint {
                    index: u32::try_from(i).map_err(|_| "native point index overflow")?,
                },
                offset + s.t[i],
                s.temp[i],
                factor,
                &mut events,
            )?;
        }
        next.state.last_lap = lap.lap_index;
        next.state.last_time_s = crossing;
        next.state.pending_boundary_lap = Some(lap.lap_index);
        *self = next;
        Ok(events)
    }

    /// Commit post-service state once, including the final lap. This detects a
    /// change observed at the service endpoint, not its unknown interior instant.
    pub fn observe_lap_boundary(
        &mut self,
        lap: &ResolvedSimulationLapV3,
        engine: &EngineParams,
        guard: EngineThermalParamsV3,
    ) -> Result<Vec<NativePhysicsEventOccurrence>, String> {
        if self.state.pending_boundary_lap != Some(lap.lap_index) {
            return Err("physical events: missing or already committed driving boundary".into());
        }
        validate_guard(engine, guard)?;
        let mut next = self.clone();
        checked_time(lap.cumulative_time_s)?;
        if lap.cumulative_time_s + 1e-9 < next.state.last_time_s {
            return Err("physical events: service time moves backwards".into());
        }
        let mut events = Vec::new();
        next.observe_point(
            lap.lap_index,
            DeratingObservationBasisV1::ServiceBoundary,
            lap.cumulative_time_s,
            lap.state_after_lap.engine_temp,
            derating_factor_v3(lap.state_after_lap.engine_temp, engine, guard),
            &mut events,
        )?;
        next.state.last_time_s = lap.cumulative_time_s;
        next.state.pending_boundary_lap = None;
        *self = next;
        Ok(events)
    }

    fn observe_point(
        &mut self,
        lap: u16,
        basis: DeratingObservationBasisV1,
        time_s: f64,
        temperature: f64,
        factor: f64,
        events: &mut Vec<NativePhysicsEventOccurrence>,
    ) -> Result<(), String> {
        let time = checked_time(time_s)?;
        let temperature_milli = (temperature * 1000.0).round();
        if !temperature_milli.is_finite()
            || temperature_milli < i32::MIN as f64
            || temperature_milli > i32::MAX as f64
            || !factor.is_finite()
            || !(0.0..=1.0).contains(&factor)
        {
            return Err("physical events: invalid temperature or availability".into());
        }
        let initial = !self.state.observed_any;
        self.state.observed_any = true;
        let active = factor < 1.0;
        if active == self.state.active_episode.is_some() {
            return Ok(());
        }
        let seq = self.state.next_sequence;
        if seq >= u64::from(self.state.policy.max_events_per_competitor) {
            return Err("physical events: record budget exhausted".into());
        }
        let (transition, episode) = if active {
            (DeratingTransitionV1::Started, seq)
        } else {
            (
                DeratingTransitionV1::Recovered,
                self.state.active_episode.unwrap(),
            )
        };
        let event = EngineDeratingEventV1 {
            schema_version: RacingPhysicsEventVersion::V1,
            policy_digest: self.policy_digest,
            competitor_id: self.state.competitor_id.clone(),
            sequence: seq,
            lap,
            transition,
            observation_basis: basis,
            episode_start_sequence: episode,
            initial_observation: initial,
            engine_temperature_millicelsius: temperature_milli as i32,
            available_power_fraction_ppm: (factor * 1e6).round() as u32,
        };
        self.state.next_sequence += 1;
        self.state.active_episode = active.then_some(episode);
        events.push(NativePhysicsEventOccurrence {
            occurred_at_us: time,
            event: RacingPhysicsEventV1::EngineDerating(event),
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pitgun_racing_solver::{DriverControlParamsV3, SimulationSolution, VehicleState};
    fn engine() -> EngineParams {
        EngineParams {
            n_rpm: vec![0.0, 12000.0],
            trq: vec![0.1, 0.1],
            gear_ratios: vec![10.0],
            n_upshift: 11000.0,
            n_downshift: 3000.0,
            n_idle: 1000.0,
            n_max: 12000.0,
            t_amb: 35.0,
            t_init: 90.0,
            c_th: 100000.0,
            alpha_heat: 0.45,
            p_cool0: 0.0,
            k_cool: 45.0,
            t_soft: 100.0,
            beta_derate: 0.01,
            fuel_burn_kg_per_s: 0.02,
        }
    }
    fn lap(index: u16, offset: f64, temps: &[f64]) -> ResolvedSimulationLapV3 {
        let e = engine();
        let guard = EngineThermalParamsV3::default();
        let n = temps.len();
        let mut factors = temps
            .iter()
            .map(|t| derating_factor_v3(*t, &e, guard))
            .collect::<Vec<_>>();
        factors[n - 1] = factors[n - 2];
        ResolvedSimulationLapV3 {
            thermal_v2: None,
            longitudinal_energy_v1: None,
            lap_index: index,
            lap_time_s: (n - 1) as f64,
            cumulative_time_s: offset + (n - 1) as f64,
            cumulative_distance_m: f64::from(index) * 100.0,
            state_after_lap: VehicleState {
                engine_temp: temps[n - 1],
                ..Default::default()
            },
            end_speed_mps: Some(20.0),
            end_gear: Some(1),
            shift_time_remaining_s: 0.0,
            active_driver_control: DriverControlParamsV3::default(),
            correction_workload_multiplier: None,
            pit_stop_completed: false,
            solution: SimulationSolution {
                t: (0..n).map(|i| i as f64).collect(),
                temp: temps.to_vec(),
                engine_derating_factor: factors,
                lap_index: vec![index; n],
                ..Default::default()
            },
        }
    }
    fn observe(
        d: &mut NativeDeratingEventDetector,
        l: &ResolvedSimulationLapV3,
    ) -> Vec<NativePhysicsEventOccurrence> {
        let e = engine();
        let guard = EngineThermalParamsV3::default();
        let mut out = d.observe_driving_lap(l, &e, guard).unwrap();
        out.extend(d.observe_lap_boundary(l, &e, guard).unwrap());
        out
    }
    fn detector() -> NativeDeratingEventDetector {
        NativeDeratingEventDetector::new("player".into(), Default::default()).unwrap()
    }
    fn data(event: &NativePhysicsEventOccurrence) -> &EngineDeratingEventV1 {
        let RacingPhysicsEventV1::EngineDerating(e) = &event.event else {
            panic!("derating-only fixture")
        };
        e
    }

    #[test]
    fn native_edges_persist_across_laps_without_duplicate_starts() {
        let mut d = detector();
        let a = observe(&mut d, &lap(1, 0.0, &[90.0, 110.0, 105.0]));
        let b = observe(&mut d, &lap(2, 2.0, &[105.0, 99.0, 98.0]));
        assert_eq!(a.len(), 1);
        assert_eq!(b.len(), 1);
        assert_eq!(data(&a[0]).transition, DeratingTransitionV1::Started);
        assert_eq!(data(&b[0]).transition, DeratingTransitionV1::Recovered);
        assert_eq!(
            (a[0].occurred_at_us, b[0].occurred_at_us),
            (1_000_000, 3_000_000)
        );
        assert_eq!(data(&a[0]).available_power_fraction_ppm, 900000);
        assert_eq!(data(&a[0]).engine_temperature_millicelsius, 110000);
        assert_eq!(
            data(&a[0]).episode_start_sequence,
            data(&b[0]).episode_start_sequence
        );
    }
    #[test]
    fn terminal_capability_uses_temperature_not_the_duplicated_segment_factor() {
        let mut d = detector();
        let out = observe(&mut d, &lap(1, 0.0, &[110.0, 115.0, 90.0]));
        assert_eq!(out.len(), 2);
        assert!(data(&out[0]).initial_observation);
        assert_eq!(data(&out[1]).transition, DeratingTransitionV1::Recovered);
        assert_eq!(
            data(&out[1]).observation_basis,
            DeratingObservationBasisV1::NativePoint { index: 2 }
        );
        assert!(!data(&out[1]).initial_observation);
    }
    #[test]
    fn repeated_within_lap_occurrences_have_distinct_stable_ids_even_at_rounded_ties() {
        let mut l = lap(1, 0.0, &[90.0, 110.0, 90.0, 110.0, 90.0]);
        l.solution.t = vec![0.0, 1e-7, 2e-7, 3e-7, 4e-7];
        l.cumulative_time_s = 4e-7;
        let out = observe(&mut detector(), &l);
        assert_eq!(out.len(), 4);
        let mut ids = std::collections::BTreeSet::new();
        for (i, o) in out.into_iter().enumerate() {
            assert_eq!(data(&o).sequence, i as u64);
            let (time, event) = o.into_unreleased();
            assert_eq!(time, 0);
            let event = event.release_at_horizon(time, time).unwrap();
            let RacingSessionEventV1::Physical {
                timing: Some(timing),
                ..
            } = event
            else {
                panic!("physical fact expected")
            };
            assert!(ids.insert(timing.event_id.to_string()));
        }
    }
    #[test]
    fn checkpoint_round_trip_preserves_pending_boundary_and_episode() {
        let a = lap(1, 0.0, &[90.0, 110.0, 105.0]);
        let b = lap(2, 2.0, &[105.0, 110.0, 90.0]);
        let mut d = detector();
        d.observe_driving_lap(&a, &engine(), Default::default())
            .unwrap();
        let checkpoint = serde_json::to_vec(&d.checkpoint()).unwrap();
        let mut resumed =
            NativeDeratingEventDetector::restore(serde_json::from_slice(&checkpoint).unwrap())
                .unwrap();
        assert_eq!(
            d.observe_lap_boundary(&a, &engine(), Default::default())
                .unwrap(),
            resumed
                .observe_lap_boundary(&a, &engine(), Default::default())
                .unwrap()
        );
        assert_eq!(observe(&mut d, &b), observe(&mut resumed, &b));
        assert_eq!(d.checkpoint(), resumed.checkpoint());
    }
    #[test]
    fn service_recovery_is_reported_at_the_observed_committed_endpoint() {
        let mut l = lap(1, 0.0, &[110.0, 115.0]);
        l.pit_stop_completed = true;
        l.cumulative_time_s += 20.0;
        l.state_after_lap.engine_temp = 90.0;
        let mut d = detector();
        let out = observe(&mut d, &l);
        assert_eq!(out.len(), 2);
        assert_eq!(out[1].occurred_at_us, 21_000_000);
        assert_eq!(
            data(&out[1]).observation_basis,
            DeratingObservationBasisV1::ServiceBoundary
        );
        assert!(observe(&mut d, &lap(2, 21.0, &[90.0, 95.0])).is_empty());
        assert!(
            d.observe_lap_boundary(&l, &engine(), Default::default())
                .is_err()
        );
    }
    #[test]
    fn malformed_or_repeated_work_cannot_partially_mutate_detector_state() {
        let good = lap(1, 0.0, &[90.0, 110.0, 90.0]);
        for kind in 0..5 {
            let mut l = good.clone();
            match kind {
                0 => l.solution.temp[2] = f64::NAN,
                1 => l.solution.engine_derating_factor[1] = 0.3,
                2 => l.solution.t[2] = l.solution.t[1],
                3 => l.cumulative_time_s = 0.0,
                _ => l.solution.engine_derating_factor[2] = f64::NAN,
            }
            let mut d = detector();
            let before = d.checkpoint();
            assert!(
                d.observe_driving_lap(&l, &engine(), Default::default())
                    .is_err()
            );
            assert_eq!(before, d.checkpoint());
        }
        let mut d = detector();
        d.observe_driving_lap(&good, &engine(), Default::default())
            .unwrap();
        let before = d.checkpoint();
        assert!(
            d.observe_driving_lap(&good, &engine(), Default::default())
                .is_err()
        );
        assert_eq!(before, d.checkpoint());
    }
    #[test]
    fn explicit_record_budget_and_checkpoint_validation_fail_closed() {
        let mut d = NativeDeratingEventDetector::new(
            "player".into(),
            DeratingEventPolicyV1 {
                max_events_per_competitor: 1,
                ..Default::default()
            },
        )
        .unwrap();
        let before = d.checkpoint();
        assert!(
            d.observe_driving_lap(
                &lap(1, 0.0, &[90.0, 110.0, 90.0]),
                &engine(),
                Default::default()
            )
            .is_err()
        );
        assert_eq!(d.checkpoint(), before);
        let mut invalid = before.clone();
        invalid.active_episode = Some(99);
        assert!(NativeDeratingEventDetector::restore(invalid).is_err());
        let mut value = serde_json::to_value(before).unwrap();
        value["unrecognized"] = serde_json::json!(true);
        assert!(serde_json::from_value::<DeratingDetectorCheckpointV1>(value).is_err());
    }
    #[test]
    fn release_requires_an_observed_horizon_and_refuses_retiming() {
        let fact = observe(&mut detector(), &lap(1, 0.0, &[90.0, 110.0]));
        let (time, event) = fact[0].clone().into_unreleased();
        assert!(event.clone().release_at_horizon(time, time - 1).is_err());
        let early = event.clone().release_at_horizon(time, time).unwrap();
        let later = event.release_at_horizon(time, time + 100).unwrap();
        let id = |e: &RacingSessionEventV1| {
            let RacingSessionEventV1::Physical {
                timing: Some(t), ..
            } = e
            else {
                panic!("timed fact expected")
            };
            t.event_id
        };
        assert_eq!(id(&early), id(&later));
        assert!(early.release_at_horizon(time, time + 1000).is_err());
    }
    #[test]
    fn invalid_guard_is_rejected_even_when_nan_would_hide_a_threshold_crossing() {
        let mut e = engine();
        e.t_soft = f64::NAN;
        assert!(
            detector()
                .observe_driving_lap(&lap(1, 0.0, &[90.0, 110.0]), &e, Default::default())
                .is_err()
        );
    }
}
