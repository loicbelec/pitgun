//! Local experimental decision boundary. Not a published workload, Authority
//! permission, benchmark identity or stable cross-domain wire contract.
//!
//! The host supplies pinned inputs and persists the journal. This module has
//! no clock, provider, network, renderer or access to a competitor's future.

#[cfg(test)]
use crate::start_incremental_race_with_catalog_and_v3_fuel_contract_candidate;
use std::collections::BTreeMap;

use pitgun_contract::{Digest, canonical_json_digest};
use serde::{Deserialize, Serialize};

use crate::{
    EmbeddedCatalog, IncrementalExecutionStreamEventV1, IncrementalRacingSession, PitStop,
    RaceOutput, RacingCatalogSnapshot, RacingDriverInstructionTimelineV1,
    RacingSessionStreamBatchV1, RunRaceRequest, TireParams,
    racing_model_v3_fuel_contract_candidate_identity, start_fuel_contract_control_mode,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrototypeVersion {
    #[serde(rename = "pitgun.racing-control-prototype/v1")]
    V1,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalControlConfig {
    pub execution_id: String,
    /// Hash of the exact runtime artifact; supplied by the local runner, not
    /// remotely attested. Replay requires this same runtime, not just a label.
    pub runtime_digest: Digest,
    /// One-based count of completed laps. Strictly increasing, before finish.
    pub decision_laps: Vec<u16>,
    pub allowed_tire_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_windows: Option<ServiceWindowPolicy>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceWindowVersion {
    #[serde(rename = "pitgun.racing-service-window/v1")]
    V1,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceWindowPolicy {
    pub schema_version: ServiceWindowVersion,
    pub closing_margin_us: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceWindow {
    pub schema_version: ServiceWindowVersion,
    pub opened_at_us: u64,
    pub deadline_at_us: u64,
    pub service_at_us: u64,
    pub service_lap: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerObservation {
    pub version: PrototypeVersion,
    pub domain: String,
    pub run_digest: Digest,
    pub decision_sequence: u64,
    pub completed_laps: u16,
    pub total_laps: u16,
    pub player_id: String,
    pub last_lap_time_ms: u64,
    pub cumulative_time_ms: u64,
    pub fuel_mass_kg: f64,
    pub tire_wear_fraction: f64,
    pub tire_temperature_c: f64,
    pub engine_temperature_c: f64,
    pub next_lap_tire_id: String,
    pub remaining_planned_pit_laps: Vec<u16>,
    pub allowed_tire_ids: Vec<String>,
    pub can_pit_next_lap: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<Box<ServiceWindow>>,
}

impl PlayerObservation {
    pub fn digest(&self) -> Result<Digest, String> {
        canonical_json_digest(self).map_err(|error| error.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    KeepPlan,
    /// Service at the end of completed_laps + 1; this tire replaces the
    /// remaining stint plan from the following lap through to the finish.
    PitNextLap {
        tire_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProposalSource {
    ReferenceController,
    Manual,
    /// The host selected the fallback; it must not impersonate a controller.
    Fallback {
        reason: FallbackReason,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FallbackReason {
    Timeout,
    InvalidResponse,
    ProviderUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    pub version: PrototypeVersion,
    pub run_digest: Digest,
    pub decision_sequence: u64,
    pub completed_laps: u16,
    pub observation_digest: Digest,
    pub idempotency_key: String,
    pub source: ProposalSource,
    pub action: Action,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_at_us: Option<u64>,
}

impl Proposal {
    pub fn for_observation(
        observation: &PlayerObservation,
        idempotency_key: String,
        source: ProposalSource,
        action: Action,
    ) -> Result<Self, String> {
        Ok(Self {
            version: PrototypeVersion::V1,
            run_digest: observation.run_digest,
            decision_sequence: observation.decision_sequence,
            completed_laps: observation.completed_laps,
            observation_digest: observation.digest()?,
            idempotency_key,
            source,
            action,
            decision_at_us: observation.window.as_ref().map(|w| w.opened_at_us),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppliedDecision {
    pub proposal: Proposal,
    /// None for keep; otherwise one-based end-of-lap service boundary.
    pub pit_after_lap: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum JournalEntry {
    Observation(PlayerObservation),
    Proposal(Proposal),
    Rejected { reason: String },
    Applied(AppliedDecision),
    Duplicate { idempotency_key: String },
}

pub enum ControlStep {
    Batch(RacingSessionStreamBatchV1),
    /// Repeated pulls return the same frozen observation without computing.
    AwaitingDecision(PlayerObservation),
}

pub struct LocalControlledRacingSession {
    session: IncrementalRacingSession,
    run_digest: Digest,
    config: LocalControlConfig,
    tires: BTreeMap<String, TireParams>,
    next_decision: usize,
    pending: Option<PlayerObservation>,
    opening: Option<(PlayerObservation, u64)>,
    service_resolved: bool,
    accepted: BTreeMap<String, AppliedDecision>,
    journal: Vec<JournalEntry>,
}

impl LocalControlledRacingSession {
    /// Pins a fuel-contract candidate, catalogue, initial timeline, runtime
    /// digest and delegated capabilities before computing anything.
    pub fn start(
        request: RunRaceRequest,
        snapshot: &RacingCatalogSnapshot,
        timeline: RacingDriverInstructionTimelineV1,
        config: LocalControlConfig,
    ) -> Result<Self, String> {
        if config.execution_id.trim().is_empty() || config.execution_id.len() > 128 {
            return Err("execution_id must contain 1..128 bytes".to_string());
        }
        let laps = request.input.race.laps;
        if config
            .decision_laps
            .iter()
            .any(|lap| *lap == 0 || *lap >= laps)
            || config
                .decision_laps
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(
                "decision laps must increase strictly between start and finish".to_string(),
            );
        }
        if request
            .input
            .race
            .competitors
            .iter()
            .filter(|c| c.is_player)
            .count()
            != 1
        {
            return Err("local control requires exactly one explicit player".to_string());
        }
        let catalog = EmbeddedCatalog::from_snapshot(snapshot)?;
        let mut tires = BTreeMap::new();
        for id in &config.allowed_tire_ids {
            let tire = catalog
                .tires
                .get(id)
                .ok_or_else(|| format!("unknown delegated tire {id}"))?;
            if tires.insert(id.clone(), tire.clone()).is_some() {
                return Err("delegated tires must be unique".to_string());
            }
        }
        let run_digest = canonical_json_digest(&(
            PrototypeVersion::V1,
            racing_model_v3_fuel_contract_candidate_identity(),
            snapshot.release_identity(),
            &request,
            &timeline,
            &config,
        ))
        .map_err(|error| error.to_string())?;
        if config
            .service_windows
            .as_ref()
            .is_some_and(|p| p.closing_margin_us > 30_000_000)
        {
            return Err("service closing margin exceeds 30 seconds".into());
        }
        let mut session = start_fuel_contract_control_mode(
            request,
            snapshot,
            timeline,
            config.service_windows.is_some(),
        )?;
        session.align_player_horizon = true;
        Ok(Self {
            session,
            run_digest,
            config,
            tires,
            next_decision: 0,
            pending: None,
            opening: None,
            service_resolved: false,
            accepted: BTreeMap::new(),
            journal: Vec::new(),
        })
    }

    pub fn journal(&self) -> &[JournalEntry] {
        &self.journal
    }

    pub fn advance(&mut self) -> Result<ControlStep, String> {
        if let Some(observation) = &self.pending {
            return Ok(ControlStep::AwaitingDecision(observation.clone()));
        }
        if self.config.service_windows.is_some() {
            return self.advance_window();
        }
        let player = self
            .session
            .competitors
            .iter()
            .find(|c| c.is_player)
            .ok_or_else(|| "missing player".to_string())?;
        if let Some(lap) = &player.latest_lap
            && self.config.decision_laps.get(self.next_decision) == Some(&lap.lap_index)
        {
            let observation = self.completed_observation()?;
            self.journal
                .push(JournalEntry::Observation(observation.clone()));
            self.pending = Some(observation.clone());
            return Ok(ControlStep::AwaitingDecision(observation));
        }
        self.session.advance().map(ControlStep::Batch)
    }

    fn completed_observation(&self) -> Result<PlayerObservation, String> {
        let player = self
            .session
            .competitors
            .iter()
            .find(|c| c.is_player)
            .ok_or("missing player")?;
        let lap = player
            .latest_lap
            .as_ref()
            .ok_or("missing completed player lap")?;
        let completed = lap.lap_index;
        Ok(PlayerObservation {
            version: PrototypeVersion::V1,
            domain: "racing".to_string(),
            run_digest: self.run_digest,
            decision_sequence: self.next_decision as u64,
            completed_laps: completed,
            total_laps: self.session.laps,
            player_id: player.competitor_id.clone(),
            last_lap_time_ms: (lap.lap_time_s * 1000.0).round() as u64,
            cumulative_time_ms: (lap.cumulative_time_s * 1000.0).round() as u64,
            fuel_mass_kg: lap.state_after_lap.fuel_mass,
            tire_wear_fraction: lap.state_after_lap.tire_wear,
            tire_temperature_c: lap.state_after_lap.tire_temp,
            engine_temperature_c: lap.state_after_lap.engine_temp,
            next_lap_tire_id: player.stint_plan.tire_by_lap[completed as usize].clone(),
            remaining_planned_pit_laps: player
                .stint_plan
                .pit_laps
                .iter()
                .copied()
                .filter(|lap| *lap > completed)
                .collect(),
            allowed_tire_ids: self.tires.keys().cloned().collect(),
            can_pit_next_lap: completed + 1 < self.session.laps && !self.tires.is_empty(),
            window: None,
        })
    }
    pub fn has_window_prefix(&self) -> bool {
        self.opening.is_some() && self.session.service_pending
    }
    fn advance_window(&mut self) -> Result<ControlStep, String> {
        if self.session.service_pending {
            if let Some((mut observation, opened_at_us)) = self.opening.take() {
                let player = self
                    .session
                    .competitors
                    .iter()
                    .find(|c| c.is_player)
                    .unwrap();
                let lap = player.latest_lap.as_ref().unwrap();
                let service_at_us = crate::physical_time_us(lap.cumulative_time_s);
                let deadline_at_us = service_at_us.saturating_sub(
                    self.config
                        .service_windows
                        .as_ref()
                        .unwrap()
                        .closing_margin_us,
                );
                if deadline_at_us <= opened_at_us {
                    return Err("service window has no reaction interval".into());
                }
                observation.window = Some(Box::new(ServiceWindow {
                    schema_version: ServiceWindowVersion::V1,
                    opened_at_us,
                    deadline_at_us,
                    service_at_us,
                    service_lap: lap.lap_index,
                }));
                self.journal
                    .push(JournalEntry::Observation(observation.clone()));
                self.pending = Some(observation.clone());
                return Ok(ControlStep::AwaitingDecision(observation));
            }
            if !self.service_resolved {
                self.session
                    .competitors
                    .iter_mut()
                    .find(|c| c.is_player)
                    .unwrap()
                    .solver
                    .resolve_service_boundary(None)?;
            }
            self.service_resolved = false;
            return self.session.advance().map(ControlStep::Batch);
        }
        let player = self
            .session
            .competitors
            .iter()
            .find(|c| c.is_player)
            .unwrap();
        if let Some(lap) = &player.latest_lap
            && self.config.decision_laps.get(self.next_decision) == Some(&lap.lap_index)
        {
            self.opening = Some((
                self.completed_observation()?,
                crate::physical_time_us(lap.cumulative_time_s),
            ));
        }
        self.session.advance().map(ControlStep::Batch)
    }

    /// Validation failure leaves the boundary open. An identical retry returns
    /// its original receipt, even after advancing; it never applies twice.
    pub fn propose(&mut self, proposal: Proposal) -> Result<AppliedDecision, String> {
        self.journal.push(JournalEntry::Proposal(proposal.clone()));
        let result = self.validate_and_apply(&proposal);
        if let Err(reason) = &result {
            self.journal.push(JournalEntry::Rejected {
                reason: reason.clone(),
            });
        }
        result
    }

    fn validate_and_apply(&mut self, proposal: &Proposal) -> Result<AppliedDecision, String> {
        if proposal.idempotency_key.trim().is_empty() || proposal.idempotency_key.len() > 128 {
            return Err("idempotency key must contain 1..128 bytes".to_string());
        }
        if let Some(accepted) = self.accepted.get(&proposal.idempotency_key) {
            if accepted.proposal != *proposal {
                return Err("idempotency key conflict".to_string());
            }
            self.journal.push(JournalEntry::Duplicate {
                idempotency_key: proposal.idempotency_key.clone(),
            });
            return Ok(accepted.clone());
        }
        let observation = self
            .pending
            .as_ref()
            .ok_or_else(|| "no open decision boundary".to_string())?;
        if proposal.run_digest != self.run_digest
            || proposal.decision_sequence != observation.decision_sequence
            || proposal.completed_laps != observation.completed_laps
            || proposal.observation_digest != observation.digest()?
        {
            return Err("proposal does not bind the open observation".to_string());
        }
        match (&observation.window, proposal.decision_at_us) {
            (Some(window), Some(at)) => {
                let expiry = matches!(
                    proposal.source,
                    ProposalSource::Fallback {
                        reason: FallbackReason::Timeout
                    }
                );
                if at < window.opened_at_us
                    || (expiry && at != window.deadline_at_us)
                    || (!expiry && at >= window.deadline_at_us)
                {
                    return Err("proposal is outside its logical service window".into());
                }
            }
            (None, None) => {}
            _ => return Err("proposal decision clock does not match the window mode".into()),
        }
        if matches!(proposal.source, ProposalSource::Fallback { .. })
            && proposal.action != Action::KeepPlan
        {
            return Err("the prototype fallback may only keep the plan".to_string());
        }
        let pit_after_lap = match &proposal.action {
            Action::KeepPlan => {
                if observation.window.is_some() {
                    self.session
                        .competitors
                        .iter_mut()
                        .find(|c| c.is_player)
                        .unwrap()
                        .solver
                        .resolve_service_boundary(None)?;
                }
                None
            }
            Action::PitNextLap { tire_id } => {
                if !observation.can_pit_next_lap {
                    return Err("pit is unavailable at this boundary".to_string());
                }
                let tire = self
                    .tires
                    .get(tire_id)
                    .ok_or_else(|| "tire is not delegated".to_string())?
                    .clone();
                let player = self
                    .session
                    .competitors
                    .iter_mut()
                    .find(|c| c.is_player)
                    .ok_or_else(|| "missing player".to_string())?;
                let pit_lap = if let Some(window) = &observation.window {
                    if player.latest_lap.as_ref().map(|p| p.lap_index) != Some(window.service_lap) {
                        return Err("service frontier mismatch".into());
                    }
                    player.solver.resolve_service_boundary(Some(tire.clone()))?
                } else {
                    player.solver.request_next_lap_pit(tire.clone())?
                };
                // Keep metadata and final lap-time service accounting aligned
                // with the actual Solver transition, including replaced stints.
                player
                    .stint_plan
                    .pit_laps
                    .retain(|lap| *lap <= observation.completed_laps);
                player.stint_plan.pit_laps.push(pit_lap);
                for assignment in &mut player.stint_plan.tire_by_lap[pit_lap as usize..] {
                    *assignment = tire_id.clone();
                }
                player
                    .request
                    .pit_plan
                    .stops
                    .retain(|stop| stop.lap <= observation.completed_laps);
                player
                    .request
                    .pit_plan
                    .stops
                    .push(PitStop { lap: pit_lap, tire });
                self.session.player_pit_laps = player.stint_plan.pit_laps.clone();
                Some(pit_lap)
            }
        };
        let applied = AppliedDecision {
            proposal: proposal.clone(),
            pit_after_lap,
        };
        self.accepted
            .insert(proposal.idempotency_key.clone(), applied.clone());
        self.journal.push(JournalEntry::Applied(applied.clone()));
        self.service_resolved = observation.window.is_some();
        self.pending = None;
        self.next_decision += 1;
        Ok(applied)
    }
}

/// Small deterministic reference controller exercising the same proposal path.
/// It is not an optimal racing strategy. All other windows keep the current plan.
pub fn reference_proposal(
    observation: &PlayerObservation,
    pit_after_completed_laps: u16,
    tire_id: &str,
) -> Result<Proposal, String> {
    let action = if observation.completed_laps == pit_after_completed_laps {
        Action::PitNextLap {
            tire_id: tire_id.to_string(),
        }
    } else {
        Action::KeepPlan
    };
    Proposal::for_observation(
        observation,
        format!("reference-{}", observation.decision_sequence),
        ProposalSource::ReferenceController,
        action,
    )
}

/// Offline reconstruction from applied decisions only. It regenerates each
/// observation and validates the recorded binding; no controller is called.
pub fn replay(
    request: RunRaceRequest,
    snapshot: &RacingCatalogSnapshot,
    timeline: RacingDriverInstructionTimelineV1,
    config: LocalControlConfig,
    applied: &[AppliedDecision],
) -> Result<RaceOutput, String> {
    let mut session = LocalControlledRacingSession::start(request, snapshot, timeline, config)?;
    let mut decisions = applied.iter();
    loop {
        match session.advance()? {
            ControlStep::AwaitingDecision(_) => {
                let recorded = decisions
                    .next()
                    .ok_or_else(|| "missing replay decision".to_string())?;
                if session.propose(recorded.proposal.clone())? != *recorded {
                    return Err("replay application differs from recorded receipt".to_string());
                }
            }
            ControlStep::Batch(batch) => {
                for record in batch.records() {
                    if let IncrementalExecutionStreamEventV1::Complete(output) = record.event() {
                        if decisions.next().is_some() {
                            return Err("extra replay decision".to_string());
                        }
                        return Ok(output.clone());
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
