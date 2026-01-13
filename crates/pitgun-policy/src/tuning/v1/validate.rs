use std::collections::BTreeSet;

use crate::tuning::v1::error::{PolicyError, invalid_field};
use crate::tuning::v1::expr::{ExprMode, parse_expression, validate_expression};
use crate::tuning::v1::schema::{ParameterSpecV1, TuningPolicyV1};
use crate::tuning::v1::TUNING_POLICY_V1_VERSION;

pub(crate) fn validate_static(policy: &TuningPolicyV1) -> Result<(), PolicyError> {
    if policy.version != TUNING_POLICY_V1_VERSION {
        return Err(PolicyError::UnsupportedVersion(policy.version.clone()));
    }
    if policy.parameters.is_empty() {
        return Err(PolicyError::MissingParameters);
    }

    let mut param_paths = BTreeSet::new();

    for (subsystem, params) in &policy.parameters {
        if params.is_empty() {
            return Err(invalid_field(
                format!("parameters.{subsystem}"),
                "must not be empty",
            ));
        }

        for (name, spec) in params {
            let path = format!("parameters.{subsystem}.{name}");
            param_paths.insert(path.clone());
            match spec {
                ParameterSpecV1::Float {
                    range,
                    default,
                    unlock,
                    ..
                } => {
                    if !range.min.is_finite() || !range.max.is_finite() || !range.step.is_finite()
                    {
                        return Err(invalid_field(
                            format!("{path}.range"),
                            "min/max/step must be finite",
                        ));
                    }
                    if range.min > range.max {
                        return Err(invalid_field(
                            format!("{path}.range"),
                            "min must be <= max",
                        ));
                    }
                    if range.step <= 0.0 {
                        return Err(invalid_field(
                            format!("{path}.range.step"),
                            "step must be > 0",
                        ));
                    }
                    if !default.is_finite() {
                        return Err(invalid_field(
                            format!("{path}.default"),
                            "default must be finite",
                        ));
                    }
                    if *default < range.min || *default > range.max {
                        return Err(invalid_field(
                            format!("{path}.default"),
                            "default must be within range",
                        ));
                    }
                    if let Some(expr) = unlock.as_deref() {
                        let parsed = parse_expression(expr).map_err(|err| {
                            invalid_field(
                                format!("{path}.unlock"),
                                format!("failed to parse '{expr}': {err}"),
                            )
                        })?;
                        validate_expression(&parsed, ExprMode::Unlock, &param_paths)
                            .map_err(|err| invalid_field(format!("{path}.unlock"), err))?;
                    }
                }
                ParameterSpecV1::Enum {
                    values,
                    default,
                    unlock,
                    ..
                } => {
                    if values.is_empty() {
                        return Err(invalid_field(
                            format!("{path}.values"),
                            "values must not be empty",
                        ));
                    }
                    let mut seen = BTreeSet::new();
                    for value in values {
                        if !seen.insert(value) {
                            return Err(invalid_field(
                                format!("{path}.values"),
                                format!("duplicate value '{value}'"),
                            ));
                        }
                    }
                    if !values.contains(default) {
                        return Err(invalid_field(
                            format!("{path}.default"),
                            "default must be in values",
                        ));
                    }
                    if let Some(expr) = unlock.as_deref() {
                        let parsed = parse_expression(expr).map_err(|err| {
                            invalid_field(
                                format!("{path}.unlock"),
                                format!("failed to parse '{expr}': {err}"),
                            )
                        })?;
                        validate_expression(&parsed, ExprMode::Unlock, &param_paths)
                            .map_err(|err| invalid_field(format!("{path}.unlock"), err))?;
                    }
                }
            }
        }
    }

    if let Some(constraints) = &policy.derived_constraints {
        let mut seen = BTreeSet::new();
        for constraint in constraints {
            if constraint.name.trim().is_empty() {
                return Err(invalid_field(
                    "derived_constraints.name",
                    "name must not be empty",
                ));
            }
            if !seen.insert(constraint.name.clone()) {
                return Err(invalid_field(
                    format!("derived_constraints.{}", constraint.name),
                    "duplicate constraint name",
                ));
            }
            let rule = constraint.rule.as_str();
            let parsed = parse_expression(rule).map_err(|err| {
                invalid_field(
                    format!("derived_constraints.{}.rule", constraint.name),
                    format!("failed to parse '{rule}': {err}"),
                )
            })?;
            validate_expression(&parsed, ExprMode::Constraint, &param_paths).map_err(|err| {
                invalid_field(format!("derived_constraints.{}.rule", constraint.name), err)
            })?;
        }
    }

    Ok(())
}
