use pitgun_contract::game::v1::GamePlayerTuningV1;

pub const MIN_POINTS: i32 = 0;
pub const MAX_POINTS: i32 = 20;
pub const MIN_SLIDER: f32 = 0.0;
pub const MAX_SLIDER: f32 = 1.0;

#[derive(Clone, Copy, Debug)]
pub struct TuningPolicyV1 {
    pub min_points: i32,
    pub max_points: i32,
    pub clamp_sliders: bool,
}

impl Default for TuningPolicyV1 {
    fn default() -> Self {
        Self {
            min_points: MIN_POINTS,
            max_points: MAX_POINTS,
            clamp_sliders: true,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum PolicyError {
    PointsOutOfRange {
        field: &'static str,
        value: i32,
        min: i32,
        max: i32,
    },
    NonFiniteSlider {
        field: &'static str,
    },
    SliderOutOfRange {
        field: &'static str,
        value: f32,
    },
}

impl std::fmt::Display for PolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PolicyError::PointsOutOfRange {
                field,
                value,
                min,
                max,
            } => write!(f, "{field} must be in range [{min},{max}] (got {value})"),
            PolicyError::NonFiniteSlider { field } => {
                write!(f, "{field} must be finite")
            }
            PolicyError::SliderOutOfRange { field, value } => {
                write!(f, "{field} must be in range [0,1] (got {value})")
            }
        }
    }
}

impl std::error::Error for PolicyError {}

impl TuningPolicyV1 {
    pub fn normalize(&self, input: GamePlayerTuningV1) -> Result<GamePlayerTuningV1, PolicyError> {
        validate_points("aero_points", input.aero_points, self)?;
        validate_points("chassis_points", input.chassis_points, self)?;
        validate_points("engine_points", input.engine_points, self)?;
        validate_points("cooling_points", input.cooling_points, self)?;

        let downforce = normalize_slider("downforce_slider", input.downforce_slider, self)?;
        let gear_ratio = normalize_slider("gear_ratio_slider", input.gear_ratio_slider, self)?;

        Ok(GamePlayerTuningV1 {
            downforce_slider: downforce,
            gear_ratio_slider: gear_ratio,
            ..input
        })
    }
}

pub fn normalize_tuning_v1(input: GamePlayerTuningV1) -> Result<GamePlayerTuningV1, PolicyError> {
    TuningPolicyV1::default().normalize(input)
}

fn validate_points(
    field: &'static str,
    value: i32,
    policy: &TuningPolicyV1,
) -> Result<(), PolicyError> {
    if value < policy.min_points || value > policy.max_points {
        return Err(PolicyError::PointsOutOfRange {
            field,
            value,
            min: policy.min_points,
            max: policy.max_points,
        });
    }
    Ok(())
}

fn normalize_slider(
    field: &'static str,
    value: f32,
    policy: &TuningPolicyV1,
) -> Result<f32, PolicyError> {
    if !value.is_finite() {
        return Err(PolicyError::NonFiniteSlider { field });
    }
    if policy.clamp_sliders {
        return Ok(value.clamp(MIN_SLIDER, MAX_SLIDER));
    }
    if !(MIN_SLIDER..=MAX_SLIDER).contains(&value) {
        return Err(PolicyError::SliderOutOfRange { field, value });
    }
    Ok(value)
}
