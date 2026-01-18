use pitgun_contract::game::v1::GamePlayerTuningV1;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CompetitorArchetype {
    AeroHeavy,
    EngineHeavy,
    ChassisGrip,
    CoolingSafe,
    Balanced,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CompetitorProfile {
    pub competitor_id: u32,
    pub display_name: String,
    pub archetype: CompetitorArchetype,
    pub tuning: GamePlayerTuningV1,
}

pub fn generate_competitors(
    seed: u64,
    track_id: &str,
    total_budget_t: i32,
    count: u32,
) -> Vec<CompetitorProfile> {
    if count == 0 {
        return Vec::new();
    }

    let track_hash = hash_track_id(track_id);
    let mut competitors = Vec::with_capacity(count as usize);

    for competitor_id in 0..count {
        let mut rng = SplitMix64::new(
            seed ^ track_hash ^ (competitor_id as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15),
        );
        let archetype = ARCHETYPES[competitor_id as usize % ARCHETYPES.len()];
        let weights = archetype_weights(archetype);
        let points = allocate_points(total_budget_t, weights, &mut rng);
        let tuning = build_tuning(points, track_id, archetype, &mut rng);

        competitors.push(CompetitorProfile {
            competitor_id,
            display_name: format!("Team {}", competitor_id + 1),
            archetype,
            tuning,
        });
    }

    competitors
}

const ARCHETYPES: [CompetitorArchetype; 5] = [
    CompetitorArchetype::AeroHeavy,
    CompetitorArchetype::EngineHeavy,
    CompetitorArchetype::ChassisGrip,
    CompetitorArchetype::CoolingSafe,
    CompetitorArchetype::Balanced,
];

fn archetype_weights(archetype: CompetitorArchetype) -> [f32; 4] {
    match archetype {
        CompetitorArchetype::AeroHeavy => [0.42, 0.20, 0.20, 0.18],
        CompetitorArchetype::EngineHeavy => [0.18, 0.20, 0.44, 0.18],
        CompetitorArchetype::ChassisGrip => [0.20, 0.40, 0.20, 0.20],
        CompetitorArchetype::CoolingSafe => [0.18, 0.20, 0.20, 0.42],
        CompetitorArchetype::Balanced => [0.25, 0.25, 0.25, 0.25],
    }
}

fn archetype_slider_bias(archetype: CompetitorArchetype) -> (f32, f32) {
    match archetype {
        CompetitorArchetype::AeroHeavy => (0.18, -0.05),
        CompetitorArchetype::EngineHeavy => (-0.10, 0.12),
        CompetitorArchetype::ChassisGrip => (0.10, -0.04),
        CompetitorArchetype::CoolingSafe => (-0.05, -0.02),
        CompetitorArchetype::Balanced => (0.0, 0.0),
    }
}

struct TrackProfile {
    downforce_base: f32,
    gear_ratio_base: f32,
}

fn track_profile(track_id: &str) -> TrackProfile {
    if track_id == "demo-oval" {
        TrackProfile {
            downforce_base: 0.35,
            gear_ratio_base: 0.6,
        }
    } else {
        TrackProfile {
            downforce_base: 0.5,
            gear_ratio_base: 0.5,
        }
    }
}

fn build_tuning(
    points: [i32; 4],
    track_id: &str,
    archetype: CompetitorArchetype,
    rng: &mut SplitMix64,
) -> GamePlayerTuningV1 {
    let profile = track_profile(track_id);
    let (df_bias, gr_bias) = archetype_slider_bias(archetype);

    let downforce_slider = jitter_slider(profile.downforce_base + df_bias, rng, 0.08);
    let gear_ratio_slider = jitter_slider(profile.gear_ratio_base + gr_bias, rng, 0.08);

    GamePlayerTuningV1 {
        aero_points: points[0],
        chassis_points: points[1],
        engine_points: points[2],
        cooling_points: points[3],
        downforce_slider,
        gear_ratio_slider,
    }
}

fn jitter_slider(base: f32, rng: &mut SplitMix64, amplitude: f32) -> f32 {
    let noise = rng.gen_range_f32(-amplitude, amplitude);
    (base + noise).clamp(0.0, 1.0)
}

fn allocate_points(total: i32, weights: [f32; 4], rng: &mut SplitMix64) -> [i32; 4] {
    if total <= 0 {
        return [0, 0, 0, 0];
    }

    let mut adjusted = weights;
    for weight in &mut adjusted {
        let noise = rng.gen_range_f32(-0.08, 0.08);
        *weight = (*weight + noise).max(0.05);
    }

    let sum: f32 = adjusted.iter().sum();
    let mut raw = [0.0; 4];
    let mut points = [0; 4];
    let mut fractions = [0.0; 4];
    let mut used = 0;

    for i in 0..4 {
        raw[i] = (adjusted[i] / sum) * total as f32;
        points[i] = raw[i].floor() as i32;
        fractions[i] = raw[i] - points[i] as f32;
        used += points[i];
    }

    let mut remaining = total - used;
    if remaining > 0 {
        let mut order = [0usize, 1, 2, 3];
        order.sort_by(|&a, &b| {
            fractions[b]
                .partial_cmp(&fractions[a])
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut idx = 0;
        while remaining > 0 {
            points[order[idx % 4]] += 1;
            remaining -= 1;
            idx += 1;
        }
    }

    points
}

struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        let mut z = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        self.state = z;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn next_f32(&mut self) -> f32 {
        let raw = self.next_u64() >> 40;
        (raw as f32) / (1u64 << 24) as f32
    }

    fn gen_range_f32(&mut self, min: f32, max: f32) -> f32 {
        min + (max - min) * self.next_f32()
    }
}

fn hash_track_id(track_id: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in track_id.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn competitor_generation_is_deterministic() {
        let a = generate_competitors(42, "demo-oval", 40, 9);
        let b = generate_competitors(42, "demo-oval", 40, 9);
        assert_eq!(a, b);
    }

    #[test]
    fn competitor_budget_matches_total() {
        let competitors = generate_competitors(7, "demo-oval", 32, 5);
        for competitor in competitors {
            let tuning = competitor.tuning;
            let sum = tuning.aero_points
                + tuning.chassis_points
                + tuning.engine_points
                + tuning.cooling_points;
            assert_eq!(sum, 32);
        }
    }
}
