//! Parameter Registry for canonical parameter definitions.
//!
//! This module provides the [`ParameterRegistry`] which serves as a canonical
//! dictionary of all telemetry parameters, inspired by ECUBridge's PGV format.
//!
//! # Architecture
//!
//! The registry acts as a single source of truth for parameter definitions:
//! - Parameter ID → Name, Unit, Type, Range, Conversion
//! - Source-specific IDs can be mapped to canonical IDs
//! - Access levels control data visibility
//! - Conversions transform raw values to engineering units
//!
//! # Example
//!
//! ```rust,ignore
//! use pitgun_contract::registry::{ParameterRegistry, Parameter, DataType};
//!
//! let registry = ParameterRegistry::load_from_yaml("registries/f1_generic.yaml")?;
//!
//! if let Some(param) = registry.get(1) {
//!     println!("{}: {} ({})", param.id, param.name, param.unit);
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

use crate::frame::{ParameterId, SampleValue};

/// A registry of parameter definitions.
///
/// The registry provides lookup and validation for telemetry parameters.
/// It can be loaded from YAML files for easy configuration.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ParameterRegistry {
    /// Version of the registry format.
    #[serde(default = "default_version")]
    pub version: String,
    /// Optional name/description of this registry.
    #[serde(default)]
    pub name: String,
    /// Optional description.
    #[serde(default)]
    pub description: String,
    /// List of parameter definitions.
    #[serde(default)]
    pub parameters: Vec<Parameter>,
    /// Internal lookup map (built on load).
    #[serde(skip)]
    index: HashMap<ParameterId, usize>,
    /// Name to ID lookup.
    #[serde(skip)]
    name_index: HashMap<String, ParameterId>,
}

fn default_version() -> String {
    "1.0".to_string()
}

impl ParameterRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a registry with the given name.
    pub fn with_name(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Loads a registry from a YAML file.
    pub fn load_from_yaml<P: AsRef<Path>>(path: P) -> Result<Self, RegistryError> {
        let content = fs::read_to_string(path.as_ref()).map_err(RegistryError::Io)?;
        Self::from_yaml(&content)
    }

    /// Parses a registry from a YAML string.
    pub fn from_yaml(yaml: &str) -> Result<Self, RegistryError> {
        let mut registry: ParameterRegistry =
            serde_yaml::from_str(yaml).map_err(RegistryError::Yaml)?;
        registry.build_index();
        registry.validate()?;
        Ok(registry)
    }

    /// Serializes the registry to YAML.
    pub fn to_yaml(&self) -> Result<String, RegistryError> {
        serde_yaml::to_string(self).map_err(RegistryError::Yaml)
    }

    /// Saves the registry to a YAML file.
    pub fn save_to_yaml<P: AsRef<Path>>(&self, path: P) -> Result<(), RegistryError> {
        let yaml = self.to_yaml()?;
        fs::write(path, yaml).map_err(RegistryError::Io)
    }

    /// Builds the internal lookup indices.
    fn build_index(&mut self) {
        self.index.clear();
        self.name_index.clear();
        for (idx, param) in self.parameters.iter().enumerate() {
            self.index.insert(param.id, idx);
            self.name_index.insert(param.name.clone(), param.id);
            if let Some(ref canonical) = param.canonical_name {
                self.name_index.insert(canonical.clone(), param.id);
            }
        }
    }

    /// Validates the registry for consistency.
    pub fn validate(&self) -> Result<(), RegistryError> {
        let mut seen_ids = HashMap::new();
        let mut seen_names = HashMap::new();

        for param in &self.parameters {
            // Check for duplicate IDs
            if let Some(existing) = seen_ids.insert(param.id, &param.name) {
                return Err(RegistryError::DuplicateId {
                    id: param.id,
                    name1: existing.clone(),
                    name2: param.name.clone(),
                });
            }
            // Check for duplicate names
            if let Some(existing_id) = seen_names.insert(param.name.clone(), param.id) {
                return Err(RegistryError::DuplicateName {
                    name: param.name.clone(),
                    id1: existing_id,
                    id2: param.id,
                });
            }
            // Validate parameter itself
            param.validate()?;
        }
        Ok(())
    }

    /// Adds a parameter to the registry.
    pub fn add(&mut self, param: Parameter) -> Result<(), RegistryError> {
        if self.index.contains_key(&param.id) {
            return Err(RegistryError::DuplicateId {
                id: param.id,
                name1: self.get(param.id).unwrap().name.clone(),
                name2: param.name.clone(),
            });
        }
        param.validate()?;
        let idx = self.parameters.len();
        self.name_index.insert(param.name.clone(), param.id);
        if let Some(ref canonical) = param.canonical_name {
            self.name_index.insert(canonical.clone(), param.id);
        }
        self.index.insert(param.id, idx);
        self.parameters.push(param);
        Ok(())
    }

    /// Gets a parameter by ID.
    pub fn get(&self, id: ParameterId) -> Option<&Parameter> {
        self.index.get(&id).map(|&idx| &self.parameters[idx])
    }

    /// Gets a mutable parameter by ID.
    pub fn get_mut(&mut self, id: ParameterId) -> Option<&mut Parameter> {
        self.index
            .get(&id)
            .copied()
            .map(|idx| &mut self.parameters[idx])
    }

    /// Gets a parameter by name.
    pub fn get_by_name(&self, name: &str) -> Option<&Parameter> {
        self.name_index.get(name).and_then(|&id| self.get(id))
    }

    /// Returns true if the registry contains a parameter with the given ID.
    pub fn contains(&self, id: ParameterId) -> bool {
        self.index.contains_key(&id)
    }

    /// Returns true if the registry contains a parameter with the given name.
    pub fn contains_name(&self, name: &str) -> bool {
        self.name_index.contains_key(name)
    }

    /// Returns the number of parameters in the registry.
    pub fn len(&self) -> usize {
        self.parameters.len()
    }

    /// Returns true if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.parameters.is_empty()
    }

    /// Returns an iterator over all parameters.
    pub fn iter(&self) -> impl Iterator<Item = &Parameter> {
        self.parameters.iter()
    }

    /// Returns parameters filtered by access level.
    pub fn with_access_level(&self, level: AccessLevel) -> impl Iterator<Item = &Parameter> {
        self.parameters
            .iter()
            .filter(move |p| p.access_level == level)
    }

    /// Returns parameters that are publicly accessible.
    pub fn public_parameters(&self) -> impl Iterator<Item = &Parameter> {
        self.with_access_level(AccessLevel::Public)
    }

    /// Converts a raw value to engineering units for a parameter.
    pub fn convert(&self, id: ParameterId, raw: &SampleValue) -> Option<f64> {
        let param = self.get(id)?;
        let raw_f64 = raw.as_f64()?;
        Some(param.conversion.apply(raw_f64))
    }

    /// Validates a value against parameter range.
    pub fn validate_value(&self, id: ParameterId, value: f64) -> Option<ValidationResult> {
        let param = self.get(id)?;
        Some(param.validate_value(value))
    }
}

/// A single parameter definition.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Parameter {
    /// Unique parameter ID.
    pub id: ParameterId,
    /// Parameter name (source-specific).
    pub name: String,
    /// Canonical parameter name (cross-source standard).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_name: Option<String>,
    /// Human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Engineering unit (e.g., "rpm", "km/h", "°C").
    #[serde(default)]
    pub unit: String,
    /// Data type of the raw value.
    #[serde(default)]
    pub data_type: DataType,
    /// Valid range for the engineering value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<Range>,
    /// Conversion from raw to engineering units.
    #[serde(default)]
    pub conversion: Conversion,
    /// Access level for this parameter.
    #[serde(default)]
    pub access_level: AccessLevel,
    /// Optional group/category.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Optional tags for filtering.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Sample rate in Hz (if known).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_rate_hz: Option<f64>,
}

impl Parameter {
    /// Creates a new parameter with minimal required fields.
    pub fn new(id: ParameterId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            canonical_name: None,
            description: None,
            unit: String::new(),
            data_type: DataType::F32,
            range: None,
            conversion: Conversion::default(),
            access_level: AccessLevel::default(),
            group: None,
            tags: Vec::new(),
            sample_rate_hz: None,
        }
    }

    /// Builder method to set the canonical name.
    pub fn with_canonical_name(mut self, name: impl Into<String>) -> Self {
        self.canonical_name = Some(name.into());
        self
    }

    /// Builder method to set the unit.
    pub fn with_unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = unit.into();
        self
    }

    /// Builder method to set the data type.
    pub fn with_data_type(mut self, data_type: DataType) -> Self {
        self.data_type = data_type;
        self
    }

    /// Builder method to set the range.
    pub fn with_range(mut self, min: f64, max: f64) -> Self {
        self.range = Some(Range { min, max });
        self
    }

    /// Builder method to set the conversion.
    pub fn with_conversion(mut self, conversion: Conversion) -> Self {
        self.conversion = conversion;
        self
    }

    /// Builder method to set linear conversion.
    pub fn with_linear_conversion(mut self, scale: f64, offset: f64) -> Self {
        self.conversion = Conversion::Linear { scale, offset };
        self
    }

    /// Builder method to set the access level.
    pub fn with_access_level(mut self, level: AccessLevel) -> Self {
        self.access_level = level;
        self
    }

    /// Builder method to set the group.
    pub fn with_group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }

    /// Builder method to add a tag.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Validates the parameter definition.
    pub fn validate(&self) -> Result<(), RegistryError> {
        if self.name.is_empty() {
            return Err(RegistryError::InvalidParameter {
                id: self.id,
                reason: "name cannot be empty".into(),
            });
        }
        if let Some(ref range) = self.range
            && range.min > range.max
        {
            return Err(RegistryError::InvalidParameter {
                id: self.id,
                reason: format!("invalid range: min ({}) > max ({})", range.min, range.max),
            });
        }
        Ok(())
    }

    /// Validates a value against this parameter's range.
    pub fn validate_value(&self, value: f64) -> ValidationResult {
        match &self.range {
            Some(range) => {
                if value < range.min {
                    ValidationResult::BelowMin(range.min)
                } else if value > range.max {
                    ValidationResult::AboveMax(range.max)
                } else {
                    ValidationResult::Valid
                }
            }
            None => ValidationResult::Valid,
        }
    }

    /// Converts a raw value to engineering units.
    pub fn convert(&self, raw: f64) -> f64 {
        self.conversion.apply(raw)
    }

    /// Converts an engineering value back to raw.
    pub fn unconvert(&self, eng: f64) -> f64 {
        self.conversion.unapply(eng)
    }
}

/// Data type of a parameter's raw value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum DataType {
    /// Boolean.
    Bool,
    /// Unsigned 8-bit.
    U8,
    /// Unsigned 16-bit.
    U16,
    /// Unsigned 32-bit.
    U32,
    /// Unsigned 64-bit.
    U64,
    /// Signed 8-bit.
    I8,
    /// Signed 16-bit.
    I16,
    /// Signed 32-bit.
    I32,
    /// Signed 64-bit.
    I64,
    /// 32-bit float.
    #[default]
    F32,
    /// 64-bit float.
    F64,
    /// Variable-length bytes.
    Bytes,
    /// String.
    String,
}

impl DataType {
    /// Returns the size in bytes (None for variable-length types).
    pub fn size_bytes(&self) -> Option<usize> {
        match self {
            Self::Bool | Self::U8 | Self::I8 => Some(1),
            Self::U16 | Self::I16 => Some(2),
            Self::U32 | Self::I32 | Self::F32 => Some(4),
            Self::U64 | Self::I64 | Self::F64 => Some(8),
            Self::Bytes | Self::String => None,
        }
    }

    /// Returns true if this is a floating-point type.
    pub fn is_float(&self) -> bool {
        matches!(self, Self::F32 | Self::F64)
    }

    /// Returns true if this is an integer type.
    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            Self::U8
                | Self::U16
                | Self::U32
                | Self::U64
                | Self::I8
                | Self::I16
                | Self::I32
                | Self::I64
        )
    }

    /// Returns true if this is a signed type.
    pub fn is_signed(&self) -> bool {
        matches!(
            self,
            Self::I8 | Self::I16 | Self::I32 | Self::I64 | Self::F32 | Self::F64
        )
    }
}

/// Valid range for a parameter value.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Range {
    /// Minimum valid value.
    pub min: f64,
    /// Maximum valid value.
    pub max: f64,
}

impl Range {
    /// Creates a new range.
    pub fn new(min: f64, max: f64) -> Self {
        Self { min, max }
    }

    /// Checks if a value is within range.
    pub fn contains(&self, value: f64) -> bool {
        value >= self.min && value <= self.max
    }

    /// Clamps a value to this range.
    pub fn clamp(&self, value: f64) -> f64 {
        value.clamp(self.min, self.max)
    }

    /// Returns the span of the range.
    pub fn span(&self) -> f64 {
        self.max - self.min
    }
}

/// Conversion from raw value to engineering units.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Conversion {
    /// No conversion (raw = engineering).
    None,
    /// Linear: eng = raw * scale + offset.
    Linear {
        scale: f64,
        #[serde(default)]
        offset: f64,
    },
    /// Polynomial: eng = sum(coefficients[i] * raw^i).
    Polynomial { coefficients: Vec<f64> },
    /// Table lookup with interpolation.
    Table { input: Vec<f64>, output: Vec<f64> },
    /// Bit extraction: extract bits from an integer.
    BitField {
        start_bit: u8,
        num_bits: u8,
        #[serde(default)]
        scale: f64,
        #[serde(default)]
        offset: f64,
    },
}

impl Default for Conversion {
    fn default() -> Self {
        Self::None
    }
}

impl Conversion {
    /// Creates a linear conversion.
    pub fn linear(scale: f64, offset: f64) -> Self {
        Self::Linear { scale, offset }
    }

    /// Creates a scale-only linear conversion.
    pub fn scale(scale: f64) -> Self {
        Self::Linear { scale, offset: 0.0 }
    }

    /// Applies the conversion to a raw value.
    pub fn apply(&self, raw: f64) -> f64 {
        match self {
            Self::None => raw,
            Self::Linear { scale, offset } => raw * scale + offset,
            Self::Polynomial { coefficients } => {
                let mut result = 0.0;
                let mut power = 1.0;
                for coef in coefficients {
                    result += coef * power;
                    power *= raw;
                }
                result
            }
            Self::Table { input, output } => {
                if input.is_empty() || output.is_empty() {
                    return raw;
                }
                // Find interpolation segment
                for i in 0..input.len().saturating_sub(1) {
                    if raw >= input[i] && raw <= input[i + 1] {
                        let t = (raw - input[i]) / (input[i + 1] - input[i]);
                        return output[i] + t * (output[i + 1] - output[i]);
                    }
                }
                // Out of range: clamp to nearest
                if raw < input[0] {
                    output[0]
                } else {
                    *output.last().unwrap_or(&raw)
                }
            }
            Self::BitField {
                start_bit,
                num_bits,
                scale,
                offset,
            } => {
                let raw_int = raw as u64;
                let mask = (1u64 << num_bits) - 1;
                let extracted = (raw_int >> start_bit) & mask;
                (extracted as f64) * scale + offset
            }
        }
    }

    /// Reverses the conversion (engineering → raw).
    pub fn unapply(&self, eng: f64) -> f64 {
        match self {
            Self::None => eng,
            Self::Linear { scale, offset } => {
                if *scale == 0.0 {
                    eng
                } else {
                    (eng - offset) / scale
                }
            }
            Self::Polynomial { .. } => {
                // Polynomial inverse is complex; return input as approximation
                eng
            }
            Self::Table { input, output } => {
                // Reverse lookup
                if input.is_empty() || output.is_empty() {
                    return eng;
                }
                for i in 0..output.len().saturating_sub(1) {
                    if eng >= output[i] && eng <= output[i + 1] {
                        let t = (eng - output[i]) / (output[i + 1] - output[i]);
                        return input[i] + t * (input[i + 1] - input[i]);
                    }
                }
                if eng < output[0] {
                    input[0]
                } else {
                    *input.last().unwrap_or(&eng)
                }
            }
            Self::BitField { scale, offset, .. } => {
                if *scale == 0.0 {
                    eng
                } else {
                    (eng - offset) / scale
                }
            }
        }
    }
}

/// Access level for parameter visibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessLevel {
    /// Fully public, no restrictions.
    #[default]
    Public,
    /// Protected, requires authentication.
    Protected,
    /// Private, internal use only.
    Private,
    /// Confidential, highly restricted.
    Confidential,
}

impl AccessLevel {
    /// Returns the numeric level (higher = more restricted).
    pub fn level(&self) -> u8 {
        match self {
            Self::Public => 0,
            Self::Protected => 1,
            Self::Private => 2,
            Self::Confidential => 3,
        }
    }

    /// Returns true if this level can access data at the given level.
    pub fn can_access(&self, required: AccessLevel) -> bool {
        self.level() >= required.level()
    }
}

/// Result of parameter value validation.
#[derive(Clone, Debug, PartialEq)]
pub enum ValidationResult {
    /// Value is within valid range.
    Valid,
    /// Value is below minimum.
    BelowMin(f64),
    /// Value is above maximum.
    AboveMax(f64),
}

impl ValidationResult {
    /// Returns true if the value is valid.
    pub fn is_valid(&self) -> bool {
        matches!(self, Self::Valid)
    }
}

/// Errors that can occur in registry operations.
#[derive(Debug)]
pub enum RegistryError {
    /// I/O error.
    Io(io::Error),
    /// YAML parsing error.
    Yaml(serde_yaml::Error),
    /// Duplicate parameter ID.
    DuplicateId {
        id: ParameterId,
        name1: String,
        name2: String,
    },
    /// Duplicate parameter name.
    DuplicateName {
        name: String,
        id1: ParameterId,
        id2: ParameterId,
    },
    /// Invalid parameter definition.
    InvalidParameter { id: ParameterId, reason: String },
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Yaml(e) => write!(f, "YAML error: {e}"),
            Self::DuplicateId { id, name1, name2 } => {
                write!(f, "duplicate parameter ID {id}: '{name1}' and '{name2}'")
            }
            Self::DuplicateName { name, id1, id2 } => {
                write!(f, "duplicate parameter name '{name}': IDs {id1} and {id2}")
            }
            Self::InvalidParameter { id, reason } => {
                write!(f, "invalid parameter {id}: {reason}")
            }
        }
    }
}

impl std::error::Error for RegistryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Yaml(e) => Some(e),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_yaml() -> &'static str {
        r#"
version: "1.0"
name: "Test Registry"
parameters:
  - id: 1
    name: engine_speed
    canonical_name: engine_rpm
    unit: rpm
    data_type: U16
    range:
      min: 0
      max: 20000
    conversion:
      type: linear
      scale: 1.0
      offset: 0.0
    access_level: public
    group: powertrain
    
  - id: 2
    name: throttle_position
    unit: "%"
    data_type: U8
    range:
      min: 0
      max: 100
    conversion:
      type: linear
      scale: 0.392157
      offset: 0.0
    access_level: public
"#
    }

    #[test]
    fn load_from_yaml() {
        let registry = ParameterRegistry::from_yaml(sample_yaml()).unwrap();
        assert_eq!(registry.len(), 2);
        assert_eq!(registry.name, "Test Registry");
    }

    #[test]
    fn get_by_id() {
        let registry = ParameterRegistry::from_yaml(sample_yaml()).unwrap();
        let param = registry.get(1).unwrap();
        assert_eq!(param.name, "engine_speed");
        assert_eq!(param.unit, "rpm");
    }

    #[test]
    fn get_by_name() {
        let registry = ParameterRegistry::from_yaml(sample_yaml()).unwrap();

        // By name
        let param = registry.get_by_name("engine_speed").unwrap();
        assert_eq!(param.id, 1);

        // By canonical name
        let param = registry.get_by_name("engine_rpm").unwrap();
        assert_eq!(param.id, 1);
    }

    #[test]
    fn linear_conversion() {
        let conv = Conversion::linear(0.392157, 0.0);
        let result = conv.apply(255.0);
        assert!((result - 100.0).abs() < 0.01);
    }

    #[test]
    fn polynomial_conversion() {
        // y = 1 + 2x + 3x²
        let conv = Conversion::Polynomial {
            coefficients: vec![1.0, 2.0, 3.0],
        };
        // x=2: 1 + 4 + 12 = 17
        assert!((conv.apply(2.0) - 17.0).abs() < 0.001);
    }

    #[test]
    fn table_conversion() {
        let conv = Conversion::Table {
            input: vec![0.0, 50.0, 100.0],
            output: vec![0.0, 25.0, 100.0],
        };
        // Linear interpolation at 25 → 12.5
        assert!((conv.apply(25.0) - 12.5).abs() < 0.001);
        // At boundaries
        assert!((conv.apply(0.0) - 0.0).abs() < 0.001);
        assert!((conv.apply(100.0) - 100.0).abs() < 0.001);
    }

    #[test]
    fn conversion_roundtrip() {
        let conv = Conversion::linear(2.5, 10.0);
        let raw = 42.0;
        let eng = conv.apply(raw);
        let back = conv.unapply(eng);
        assert!((raw - back).abs() < 0.001);
    }

    #[test]
    fn range_validation() {
        let param = Parameter::new(1, "test").with_range(0.0, 100.0);

        assert!(param.validate_value(50.0).is_valid());
        assert!(!param.validate_value(-1.0).is_valid());
        assert!(!param.validate_value(101.0).is_valid());
    }

    #[test]
    fn access_levels() {
        assert!(AccessLevel::Confidential.can_access(AccessLevel::Public));
        assert!(!AccessLevel::Public.can_access(AccessLevel::Confidential));
    }

    #[test]
    fn add_parameter() {
        let mut registry = ParameterRegistry::new();
        registry
            .add(Parameter::new(1, "speed").with_unit("km/h"))
            .unwrap();
        registry
            .add(Parameter::new(2, "rpm").with_unit("rpm"))
            .unwrap();

        assert_eq!(registry.len(), 2);
        assert!(registry.contains(1));
        assert!(registry.contains_name("speed"));
    }

    #[test]
    fn duplicate_id_error() {
        let mut registry = ParameterRegistry::new();
        registry.add(Parameter::new(1, "speed")).unwrap();
        let result = registry.add(Parameter::new(1, "velocity"));
        assert!(result.is_err());
    }

    #[test]
    fn parameter_builder() {
        let param = Parameter::new(42, "engine_temp")
            .with_canonical_name("coolant_temperature")
            .with_unit("°C")
            .with_data_type(DataType::I16)
            .with_range(-40.0, 150.0)
            .with_linear_conversion(0.1, -40.0)
            .with_access_level(AccessLevel::Protected)
            .with_group("thermal")
            .with_tag("engine")
            .with_tag("temperature");

        assert_eq!(param.id, 42);
        assert_eq!(param.name, "engine_temp");
        assert_eq!(
            param.canonical_name,
            Some("coolant_temperature".to_string())
        );
        assert_eq!(param.tags.len(), 2);
    }

    #[test]
    fn data_type_properties() {
        assert!(DataType::F32.is_float());
        assert!(!DataType::U16.is_float());
        assert!(DataType::I32.is_integer());
        assert!(DataType::I16.is_signed());
        assert!(!DataType::U32.is_signed());
        assert_eq!(DataType::U32.size_bytes(), Some(4));
        assert_eq!(DataType::Bytes.size_bytes(), None);
    }

    #[test]
    fn registry_to_yaml() {
        let mut registry = ParameterRegistry::with_name("My Registry");
        registry
            .add(
                Parameter::new(1, "speed")
                    .with_unit("km/h")
                    .with_range(0.0, 400.0),
            )
            .unwrap();

        let yaml = registry.to_yaml().unwrap();
        assert!(yaml.contains("speed"));
        assert!(yaml.contains("km/h"));
    }

    #[test]
    fn bitfield_conversion() {
        // Extract bits 4-7 (4 bits), scale by 10
        let conv = Conversion::BitField {
            start_bit: 4,
            num_bits: 4,
            scale: 10.0,
            offset: 0.0,
        };
        // 0xF0 (240) has bits 4-7 = 0xF = 15
        assert!((conv.apply(0xF0 as f64) - 150.0).abs() < 0.001);
    }
}
