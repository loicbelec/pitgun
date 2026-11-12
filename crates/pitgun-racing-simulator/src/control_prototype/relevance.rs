//! Racing-only event selection. Consumes released facts, never solver look-ahead.
use super::*;
use crate::RacingSessionEventV1;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RivalPitPolicyVersion {
    #[serde(rename = "pitgun.racing-rival-pit-policy/v1")]
    V1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RivalPitPolicy {
    pub schema_version: RivalPitPolicyVersion,
    pub max_pre_stop_gap_us: u64,
    pub max_event_age_us: u64,
    pub planned_stop_within_laps: u16,
    pub cooldown_laps: u16,
    pub max_windows: u16,
}
impl RivalPitPolicy {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=30_000_000).contains(&self.max_pre_stop_gap_us)
            || !(1..=300_000_000).contains(&self.max_event_age_us)
            || !(2..=8).contains(&self.planned_stop_within_laps)
            || !(2..=16).contains(&self.cooldown_laps)
            || !(1..=16).contains(&self.max_windows)
        {
            return Err("rival-pit policy thresholds exceed local bounds".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RivalPitEvidence {
    pub event_id: Digest,
    pub competitor_id: String,
    pub lap: u16,
    pub occurred_at_us: u64,
    /// Absolute difference between observed same-lap crossings before this stop.
    /// This is a checkpoint gap, not a live on-track or predicted undercut gap.
    pub pre_stop_gap_us: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RivalPitEpisode {
    pub schema_version: RivalPitPolicyVersion,
    pub policy_digest: Digest,
    pub episode_id: Digest,
    pub observed_at_us: u64,
    pub next_planned_pit_lap: u16,
    pub evidence: Vec<RivalPitEvidence>,
}

#[derive(Default)]
pub(super) struct RivalPitFilter {
    crossings: BTreeMap<(String, u16), u64>,
    seen: BTreeSet<String>,
    stops: Vec<(String, u16, crate::RacingEventTimingV1)>,
    last_window_lap: Option<u16>,
    windows: u16,
}
impl RivalPitFilter {
    pub fn ingest(&mut self, event: &RacingSessionEventV1) {
        match event {
            RacingSessionEventV1::LapCompleted {
                competitor_id,
                lap,
                timing: Some(t),
            } => {
                self.crossings
                    .entry((competitor_id.clone(), *lap))
                    .or_insert(t.occurred_at_us);
            }
            RacingSessionEventV1::PitStopCompleted {
                competitor_id,
                lap,
                timing: Some(t),
            } if self.seen.insert(t.event_id.to_string()) => {
                self.stops.push((competitor_id.clone(), *lap, t.clone()));
            }
            _ => {}
        }
    }
    pub fn review(
        &mut self,
        policy: &RivalPitPolicy,
        o: &PlayerObservation,
        horizon: u64,
    ) -> Result<Option<RivalPitEpisode>, String> {
        // Each observed stop gets one assessment. Silence/cooldown never queues a
        // stale alert for later; a pending observation is never mutated.
        let stops = std::mem::take(&mut self.stops);
        let Some(next_stop) = o.remaining_planned_pit_laps.first().copied() else {
            return Ok(None);
        };
        if !o.can_pit_next_lap
            || next_stop <= o.completed_laps + 1
            || next_stop > o.completed_laps + policy.planned_stop_within_laps
            || self.windows >= policy.max_windows
            || self
                .last_window_lap
                .is_some_and(|lap| o.completed_laps < lap + policy.cooldown_laps)
        {
            return Ok(None);
        }
        let mut evidence = Vec::new();
        for (id, lap, t) in stops {
            if id == o.player_id
                || t.occurred_at_us > horizon
                || t.released_at_us > horizon
                || horizon - t.occurred_at_us > policy.max_event_age_us
            {
                continue;
            }
            let Some(player_crossing) = self.crossings.get(&(o.player_id.clone(), lap)) else {
                continue;
            };
            let Some(rival_crossing) = self.crossings.get(&(id.clone(), lap)) else {
                continue;
            };
            if *player_crossing > horizon || *rival_crossing > t.occurred_at_us {
                continue;
            }
            let gap = player_crossing.abs_diff(*rival_crossing);
            if gap <= policy.max_pre_stop_gap_us {
                evidence.push(RivalPitEvidence {
                    event_id: t.event_id,
                    competitor_id: id,
                    lap,
                    occurred_at_us: t.occurred_at_us,
                    pre_stop_gap_us: gap,
                });
            }
        }
        evidence.sort_by(|a, b| {
            a.pre_stop_gap_us
                .cmp(&b.pre_stop_gap_us)
                .then(a.occurred_at_us.cmp(&b.occurred_at_us))
                .then(a.event_id.to_string().cmp(&b.event_id.to_string()))
        });
        if evidence.is_empty() {
            return Ok(None);
        }
        let policy_digest = canonical_json_digest(policy).map_err(|e| e.to_string())?;
        let episode_id =
            canonical_json_digest(&(o.run_digest, policy_digest, horizon, next_stop, &evidence))
                .map_err(|e| e.to_string())?;
        self.last_window_lap = Some(o.completed_laps);
        self.windows += 1;
        Ok(Some(RivalPitEpisode {
            schema_version: policy.schema_version,
            policy_digest,
            episode_id,
            observed_at_us: horizon,
            next_planned_pit_lap: next_stop,
            evidence,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn policy() -> RivalPitPolicy {
        RivalPitPolicy {
            schema_version: RivalPitPolicyVersion::V1,
            max_pre_stop_gap_us: 8_000_000,
            max_event_age_us: 120_000_000,
            planned_stop_within_laps: 2,
            cooldown_laps: 2,
            max_windows: 3,
        }
    }
    fn observation(lap: u16) -> PlayerObservation {
        PlayerObservation {
            version: PrototypeVersion::V1,
            domain: "racing".into(),
            run_digest: Digest::from_bytes(b"run"),
            decision_sequence: 0,
            completed_laps: lap,
            total_laps: 20,
            player_id: "player".into(),
            last_lap_time_ms: 100_000,
            cumulative_time_ms: u64::from(lap) * 100_000,
            fuel_mass_kg: 100.,
            tire_wear_fraction: 0.1,
            tire_temperature_c: 80.,
            engine_temperature_c: 90.,
            next_lap_tire_id: "soft".into(),
            remaining_planned_pit_laps: vec![lap + 2],
            allowed_tire_ids: vec!["hard".into()],
            can_pit_next_lap: true,
            window: None,
            trigger: None,
        }
    }
    fn stop(filter: &mut RivalPitFilter, id: &str, lap: u16, gap: u64, at: u64) {
        for (id, at) in [
            ("player", u64::from(lap) * 100_000_000),
            (id, u64::from(lap) * 100_000_000 - gap),
        ] {
            filter.ingest(
                &RacingSessionEventV1::LapCompleted {
                    competitor_id: id.into(),
                    lap,
                    timing: None,
                }
                .with_timing(at, at)
                .unwrap(),
            );
        }
        filter.ingest(
            &RacingSessionEventV1::PitStopCompleted {
                competitor_id: id.into(),
                lap,
                timing: None,
            }
            .with_timing(at, at)
            .unwrap(),
        );
    }
    #[test]
    fn far_future_old_own_or_unactionable_stops_are_silent() {
        let p = policy();
        let o = observation(2);
        for (id, gap, at) in [
            ("far", 8_000_001, 120_000_000),
            ("future", 0, 210_000_000),
            ("old", 0, 70_000_000),
            ("player", 0, 120_000_000),
        ] {
            let mut f = RivalPitFilter::default();
            stop(&mut f, id, 1, gap, at);
            assert!(f.review(&p, &o, 200_000_000).unwrap().is_none(), "{id}");
        }
        for mode in 0..4 {
            let mut f = RivalPitFilter::default();
            stop(&mut f, "rival", 1, 0, 120_000_000);
            let mut o = o.clone();
            match mode {
                0 => o.can_pit_next_lap = false,
                1 => o.remaining_planned_pit_laps.clear(),
                2 => o.remaining_planned_pit_laps = vec![3],
                _ => o.remaining_planned_pit_laps = vec![8],
            };
            assert!(f.review(&p, &o, 200_000_000).unwrap().is_none());
            assert!(
                f.review(&p, &observation(3), 300_000_000)
                    .unwrap()
                    .is_none(),
                "discard suppressed evidence"
            );
        }
    }
    #[test]
    fn stable_grouping_deduplicates_and_ignores_input_order() {
        let mut episodes = Vec::new();
        for ids in [["a", "b", "a"], ["b", "a", "b"]] {
            let mut f = RivalPitFilter::default();
            for id in ids {
                stop(&mut f, id, 1, 0, 120_000_000);
            }
            let e = f
                .review(&policy(), &observation(2), 200_000_000)
                .unwrap()
                .unwrap();
            assert_eq!(e.evidence.len(), 2);
            episodes.push(e);
        }
        assert_eq!(episodes[0], episodes[1]);
    }
    #[test]
    fn cooldown_discards_in_flight_events_and_budget_caps_new_episodes() {
        let mut f = RivalPitFilter::default();
        let mut p = policy();
        p.max_windows = 2;
        stop(&mut f, "a", 1, 0, 120_000_000);
        assert!(
            f.review(&p, &observation(2), 200_000_000)
                .unwrap()
                .is_some()
        );
        stop(&mut f, "b", 2, 0, 220_000_000);
        assert!(
            f.review(&p, &observation(3), 300_000_000)
                .unwrap()
                .is_none()
        );
        assert!(
            f.review(&p, &observation(4), 400_000_000)
                .unwrap()
                .is_none()
        );
        stop(&mut f, "c", 4, 0, 420_000_000);
        assert!(
            f.review(&p, &observation(5), 500_000_000)
                .unwrap()
                .is_some()
        );
        stop(&mut f, "d", 6, 0, 620_000_000);
        assert!(
            f.review(&p, &observation(7), 700_000_000)
                .unwrap()
                .is_none()
        );
    }
    #[test]
    fn missing_same_lap_crossings_never_invent_a_proximity_gap() {
        let mut f = RivalPitFilter::default();
        f.ingest(
            &RacingSessionEventV1::PitStopCompleted {
                competitor_id: "rival".into(),
                lap: 1,
                timing: None,
            }
            .with_timing(120_000_000, 200_000_000)
            .unwrap(),
        );
        assert!(
            f.review(&policy(), &observation(2), 200_000_000)
                .unwrap()
                .is_none()
        );
    }
}
