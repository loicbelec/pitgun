use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DrivingStyle {
    Conservative,
    Balanced,
    Aggressive,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EngineMode {
    Economy,
    Balanced,
    Push,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionKind {
    Fp1,
    Fp2,
    Fp3,
    Race,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompetitorProfile {
    pub id: String,
    pub display_name: String,
    pub style: DrivingStyle,
    pub engine_mode: EngineMode,
    pub tire_id: String,
    pub downforce_bias: f64,
    pub gear_ratio_bias: f64,
    pub pace_variance_ms: f64,
}

impl Default for CompetitorProfile {
    fn default() -> Self {
        Self {
            id: "balanced".to_string(),
            display_name: "Balanced".to_string(),
            style: DrivingStyle::Balanced,
            engine_mode: EngineMode::Balanced,
            tire_id: "medium".to_string(),
            downforce_bias: 0.0,
            gear_ratio_bias: 0.0,
            pace_variance_ms: 30.0,
        }
    }
}

impl CompetitorProfile {
    pub fn for_session(&self, session: SessionKind) -> Self {
        let mut adjusted = self.clone();
        match session {
            SessionKind::Fp1 => {
                adjusted.engine_mode = EngineMode::Economy;
                adjusted.style = DrivingStyle::Conservative;
                adjusted.pace_variance_ms *= 1.2;
            }
            SessionKind::Fp2 => {
                adjusted.engine_mode = EngineMode::Balanced;
                adjusted.style = DrivingStyle::Balanced;
            }
            SessionKind::Fp3 => {
                adjusted.engine_mode = EngineMode::Push;
                adjusted.style = DrivingStyle::Aggressive;
                adjusted.pace_variance_ms *= 1.1;
            }
            SessionKind::Race => {}
        }
        adjusted
    }

    pub fn tire_wear_multiplier(&self) -> f64 {
        match self.style {
            DrivingStyle::Conservative => 0.92,
            DrivingStyle::Balanced => 1.0,
            DrivingStyle::Aggressive => 1.14,
        }
    }

    pub fn power_multiplier(&self) -> f64 {
        let mode = match self.engine_mode {
            EngineMode::Economy => 0.95,
            EngineMode::Balanced => 1.0,
            EngineMode::Push => 1.05,
        };

        let style = match self.style {
            DrivingStyle::Conservative => 0.985,
            DrivingStyle::Balanced => 1.0,
            DrivingStyle::Aggressive => 1.015,
        };

        mode * style
    }

    pub fn heat_multiplier(&self) -> f64 {
        match self.engine_mode {
            EngineMode::Economy => 0.92,
            EngineMode::Balanced => 1.0,
            EngineMode::Push => 1.10,
        }
    }

    pub fn fuel_multiplier(&self) -> f64 {
        match self.engine_mode {
            EngineMode::Economy => 0.94,
            EngineMode::Balanced => 1.0,
            EngineMode::Push => 1.08,
        }
    }
}

pub fn builtin_profiles() -> Vec<CompetitorProfile> {
    vec![
        CompetitorProfile {
            id: "balanced".to_string(),
            display_name: "Balanced".to_string(),
            ..CompetitorProfile::default()
        },
        CompetitorProfile {
            id: "aggressive".to_string(),
            display_name: "Aggressive".to_string(),
            style: DrivingStyle::Aggressive,
            engine_mode: EngineMode::Push,
            tire_id: "soft".to_string(),
            downforce_bias: 0.05,
            gear_ratio_bias: -0.04,
            pace_variance_ms: 38.0,
        },
        CompetitorProfile {
            id: "conservative".to_string(),
            display_name: "Conservative".to_string(),
            style: DrivingStyle::Conservative,
            engine_mode: EngineMode::Economy,
            tire_id: "hard".to_string(),
            downforce_bias: -0.03,
            gear_ratio_bias: 0.04,
            pace_variance_ms: 22.0,
        },
    ]
}
