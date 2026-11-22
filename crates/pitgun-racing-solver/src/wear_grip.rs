//! Explicit opt-in performance loss from the normalized accumulated wear state.
//! The curve does not change dissipated work, heat, or wear accumulation.
use crate::TireParams;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "model", deny_unknown_fields)]
pub enum TireWearGripCurveV1 {
    #[serde(rename = "progressive_v1")]
    Progressive {
        linear_loss: f64,
        late_life_loss: f64,
        late_life_power: u8,
    },
}

impl TireWearGripCurveV1 {
    pub fn validate(&self) -> Result<(), String> {
        match *self {
            Self::Progressive {
                linear_loss,
                late_life_loss,
                late_life_power,
            } => {
                if !linear_loss.is_finite()
                    || !late_life_loss.is_finite()
                    || linear_loss < 0.0
                    || late_life_loss < 0.0
                    || linear_loss + late_life_loss > 1.0
                    || !(2..=16).contains(&late_life_power)
                {
                    return Err("wear grip V1: finite nonnegative losses with sum<=1 and power2..16 required".into());
                }
            }
        }
        Ok(())
    }

    /// Validated coefficients and wear in [0,1] make this monotone nonincreasing.
    /// Endpoint evaluation therefore remains valid for reachable-wear bounds.
    fn factor(&self, wear: f64) -> f64 {
        match *self {
            Self::Progressive {
                linear_loss,
                late_life_loss,
                late_life_power,
            } => {
                // Explicit products keep the arithmetic sequence independent of
                // platform pow/libm implementations. Common candidates are cheap.
                let power = match late_life_power {
                    2 => wear * wear,
                    4 => {
                        let square = wear * wear;
                        square * square
                    }
                    8 => {
                        let square = wear * wear;
                        let fourth = square * square;
                        fourth * fourth
                    }
                    _ => (1..late_life_power).fold(wear, |product, _| product * wear),
                };
                1.0 - linear_loss * wear - late_life_loss * power
            }
        }
    }
}

/// Requires the ordinary validated TireParams and normalized state in [0,1].
/// An absent curve retains the historical expression and wire representation.
pub fn tire_wear_grip_factor(wear: f64, tire: &TireParams) -> f64 {
    let factor = match &tire.wear_grip_curve {
        Some(curve) => curve.factor(wear),
        None => 1.0 - tire.wear_grip_k * wear,
    };
    factor.max(tire.wear_min)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate() -> TireWearGripCurveV1 {
        TireWearGripCurveV1::Progressive {
            linear_loss: 0.04,
            late_life_loss: 0.16,
            late_life_power: 4,
        }
    }

    #[test]
    fn legacy_catalog_bytes_keep_their_serialized_input_identity() {
        let source: serde_json::Value = serde_json::from_str(include_str!(
            "../../../catalogs/racing/v1.8.1/simulation/tires/soft.json"
        ))
        .unwrap();
        let mut tire: TireParams = serde_json::from_value(source.clone()).unwrap();
        assert!(tire.wear_grip_curve.is_none());
        assert_eq!(serde_json::to_value(&tire).unwrap(), source);
        assert_eq!(tire_wear_grip_factor(0.2, &tire), 0.88);
        tire.wear_grip_curve = Some(candidate());
        assert!(tire_wear_grip_factor(0.2, &tire) > 0.99);
        let revised = serde_json::to_value(&tire).unwrap();
        assert_eq!(revised["wear_grip_curve"]["model"], "progressive_v1");
        assert_ne!(revised, source);
        assert_eq!(serde_json::from_value::<TireParams>(revised).unwrap(), tire);
    }

    #[test]
    fn operational_life_has_small_early_loss_and_late_decline() {
        let curve = candidate();
        curve.validate().unwrap();
        assert_eq!(curve.factor(0.0), 1.0);
        assert!(curve.factor(0.2) > 0.99);
        assert!(curve.factor(0.9) < 0.9);
        assert!((curve.factor(1.0) - 0.8).abs() < 1e-14);
        let early_drop = curve.factor(0.0) - curve.factor(0.25);
        let late_drop = curve.factor(0.75) - curve.factor(1.0);
        assert!(late_drop > 5.0 * early_drop);
    }

    #[test]
    fn validated_family_preserves_monotone_endpoint_bounds() {
        for power in 2..=16 {
            let curve = TireWearGripCurveV1::Progressive {
                linear_loss: 0.08,
                late_life_loss: 0.12,
                late_life_power: power,
            };
            curve.validate().unwrap();
            let mut previous = curve.factor(0.0);
            for i in 1..=1000 {
                let value = curve.factor(f64::from(i) / 1000.0);
                assert!(value <= previous && value >= 0.8 - 1e-14);
                previous = value;
            }
        }
    }

    #[test]
    fn invalid_or_unknown_curves_are_rejected() {
        for (a, b, p) in [
            (f64::NAN, 0.1, 4),
            (0.1, f64::INFINITY, 4),
            (-0.01, 0.1, 4),
            (0.1, -0.01, 4),
            (0.5, 0.6, 4),
            (0.04, 0.16, 1),
            (0.04, 0.16, 17),
        ] {
            assert!(
                TireWearGripCurveV1::Progressive {
                    linear_loss: a,
                    late_life_loss: b,
                    late_life_power: p,
                }
                .validate()
                .is_err()
            );
        }
        assert!(serde_json::from_str::<TireWearGripCurveV1>(r#"{"model":"unknown"}"#).is_err());
        assert!(serde_json::from_str::<TireWearGripCurveV1>(r#"{"model":"progressive_v1","linear_loss":0.04,"late_life_loss":0.16,"late_life_power":4,"extra":1}"#).is_err());
    }
}
