use serde::{Deserialize, Serialize};

use crate::models::{DriverConfig, TireConfig};

const DEFAULT_DRIVER_ID: &str = "default";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DriverEffects {
    pub tire_wear_multiplier: f64,
    pub lap_time_noise_std_ms: i32,
    pub peak_pace_bonus_ms: i32,
}

pub fn default_driver_id() -> &'static str {
    DEFAULT_DRIVER_ID
}

pub fn driver_effects(driver: &DriverConfig) -> DriverEffects {
    let a = driver.aggressiveness.clamp(0.0, 1.0);
    DriverEffects {
        tire_wear_multiplier: lerp(0.92, 1.18, a),
        lap_time_noise_std_ms: lerp(20.0, 80.0, a).round() as i32,
        peak_pace_bonus_ms: lerp(-20.0, -90.0, a).round() as i32,
    }
}

pub fn apply_driver_to_tire(tire: &TireConfig, effects: &DriverEffects) -> TireConfig {
    let mut adjusted = tire.clone();
    adjusted.wear_per_s *= effects.tire_wear_multiplier;
    adjusted
}

pub fn deterministic_lap_delta_ms(
    effects: &DriverEffects,
    driver_id: &str,
    seed: Option<u64>,
    lap_number: u16,
) -> i32 {
    let noise_ms = match seed {
        Some(seed) => deterministic_noise(seed, driver_id, lap_number, effects.lap_time_noise_std_ms),
        None => 0,
    };
    effects.peak_pace_bonus_ms + noise_ms
}

fn lerp(x0: f64, x1: f64, a: f64) -> f64 {
    x0 + (x1 - x0) * a
}

fn deterministic_noise(seed: u64, driver_id: &str, lap_number: u16, std_dev_ms: i32) -> i32 {
    if std_dev_ms <= 0 {
        return 0;
    }

    let mut state = seed ^ ((lap_number as u64) << 32);
    for byte in driver_id.as_bytes() {
        state = state
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(*byte as u64 + 0xBF58_476D_1CE4_E5B9);
        state ^= state >> 27;
    }

    // Approximate a bell-shaped distribution by summing three centered uniforms.
    let centered = centered_unit(state)
        + centered_unit(state.rotate_left(13))
        + centered_unit(state.rotate_left(29));
    let normalized = centered / 3.0;
    (normalized * std_dev_ms as f64).round() as i32
}

fn centered_unit(mut state: u64) -> f64 {
    state ^= state >> 30;
    state = state.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    state ^= state >> 27;
    state = state.wrapping_mul(0x94D0_49BB_1331_11EB);
    state ^= state >> 31;
    let unit = (state as f64) / (u64::MAX as f64);
    unit * 2.0 - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn driver_effects_match_aggressiveness_curve() {
        let effects = driver_effects(&DriverConfig {
            id: "d".to_string(),
            display_name: "Driver".to_string(),
            aggressiveness: 1.0,
        });

        assert!(effects.tire_wear_multiplier > 1.1);
        assert!(effects.peak_pace_bonus_ms < -80);
    }

    #[test]
    fn noise_is_seeded_and_stable() {
        let effects = DriverEffects {
            tire_wear_multiplier: 1.0,
            lap_time_noise_std_ms: 80,
            peak_pace_bonus_ms: 0,
        };

        let a = deterministic_lap_delta_ms(&effects, "driver-x", Some(42), 3);
        let b = deterministic_lap_delta_ms(&effects, "driver-x", Some(42), 3);
        let c = deterministic_lap_delta_ms(&effects, "driver-x", Some(43), 3);

        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
