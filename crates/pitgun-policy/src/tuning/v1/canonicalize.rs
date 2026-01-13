use serde_json::{Map as JsonMap, Value as JsonValue};

use crate::tuning::v1::error::{PolicyError, invalid_field};
use crate::tuning::v1::expr::{EvalContext, eval_bool, parse_expression};
use crate::tuning::v1::schema::{FloatRange, ParameterSpecV1, TuningPolicyV1};
use crate::tuning::v1::{CanonicalTuningParameters, PlayerTuningRequest, TuningEvalContext};

pub(crate) fn canonicalize(
    policy: &TuningPolicyV1,
    ctx: &TuningEvalContext,
    req: &PlayerTuningRequest,
) -> Result<CanonicalTuningParameters, PolicyError> {
    let req_params = req
        .parameters
        .as_object()
        .ok_or_else(|| invalid_field("parameters", "must be an object"))?;

    for (subsystem, value) in req_params {
        let Some(specs) = policy.parameters.get(subsystem) else {
            return Err(invalid_field(
                format!("parameters.{subsystem}"),
                "unknown subsystem",
            ));
        };
        let sub_map = value.as_object().ok_or_else(|| {
            invalid_field(format!("parameters.{subsystem}"), "must be an object")
        })?;
        for key in sub_map.keys() {
            if !specs.contains_key(key) {
                return Err(invalid_field(
                    format!("parameters.{subsystem}.{key}"),
                    "unknown parameter",
                ));
            }
        }
    }

    let mut output = JsonMap::new();
    for (subsystem, params) in &policy.parameters {
        let req_sub = req_params
            .get(subsystem)
            .and_then(|value| value.as_object());
        let mut out_params = JsonMap::new();

        for (name, spec) in params {
            let path = format!("parameters.{subsystem}.{name}");
            let value = match req_sub.and_then(|map| map.get(name)) {
                Some(raw) => {
                    if let Some(expr) = spec.unlock_expr() {
                        let parsed = parse_expression(expr).map_err(|err| {
                            invalid_field(
                                format!("{path}.unlock"),
                                format!("failed to parse '{expr}': {err}"),
                            )
                        })?;
                        let eval_ctx = EvalContext {
                            era: ctx.era,
                            category_levels: &ctx.category_levels,
                            owned_upgrades: &ctx.owned_upgrades,
                            parameters: None,
                        };
                        let allowed = eval_bool(&parsed, &eval_ctx).map_err(|err| {
                            invalid_field(
                                format!("{path}.unlock"),
                                format!("unlock '{expr}' failed: {err}"),
                            )
                        })?;
                        if !allowed {
                            return Err(invalid_field(
                                path,
                                format!("unlock condition not met: {expr}"),
                            ));
                        }
                    }
                    canonicalize_value(spec, raw, &path)?
                }
                None => spec.default_value(&path)?,
            };
            out_params.insert(name.clone(), value);
        }
        output.insert(subsystem.clone(), JsonValue::Object(out_params));
    }

    Ok(CanonicalTuningParameters {
        parameters: JsonValue::Object(output),
    })
}

pub(crate) fn validate_constraints(
    policy: &TuningPolicyV1,
    ctx: &TuningEvalContext,
    canonical: &CanonicalTuningParameters,
) -> Result<(), PolicyError> {
    let Some(constraints) = &policy.derived_constraints else {
        return Ok(());
    };
    if !canonical.parameters.is_object() {
        return Err(invalid_field("parameters", "must be an object"));
    }

    let eval_ctx = EvalContext {
        era: ctx.era,
        category_levels: &ctx.category_levels,
        owned_upgrades: &ctx.owned_upgrades,
        parameters: Some(&canonical.parameters),
    };

    for constraint in constraints {
        let rule = constraint.rule.as_str();
        let parsed = parse_expression(rule).map_err(|err| {
            invalid_field(
                format!("derived_constraints.{}.rule", constraint.name),
                format!("failed to parse '{rule}': {err}"),
            )
        })?;
        let ok = eval_bool(&parsed, &eval_ctx).map_err(|err| {
            invalid_field(
                format!("derived_constraints.{}", constraint.name),
                format!("constraint '{rule}' failed: {err}"),
            )
        })?;
        if !ok {
            return Err(invalid_field(
                format!("derived_constraints.{}", constraint.name),
                constraint.error_msg.clone(),
            ));
        }
    }

    Ok(())
}

impl ParameterSpecV1 {
    fn unlock_expr(&self) -> Option<&str> {
        match self {
            ParameterSpecV1::Float { unlock, .. } => unlock.as_deref(),
            ParameterSpecV1::Enum { unlock, .. } => unlock.as_deref(),
        }
    }

    fn default_value(&self, path: &str) -> Result<JsonValue, PolicyError> {
        match self {
            ParameterSpecV1::Float { default, range, .. } => {
                let quantized = quantize_float(*default, range);
                json_number(quantized, path)
            }
            ParameterSpecV1::Enum { default, .. } => Ok(JsonValue::String(default.clone())),
        }
    }
}

fn canonicalize_value(
    spec: &ParameterSpecV1,
    raw: &JsonValue,
    path: &str,
) -> Result<JsonValue, PolicyError> {
    match spec {
        ParameterSpecV1::Float { range, .. } => {
            let value = raw.as_f64().ok_or_else(|| {
                invalid_field(path, "expected a number for float parameter")
            })?;
            if !value.is_finite() {
                return Err(invalid_field(path, "float must be finite"));
            }
            let quantized = quantize_float(value, range);
            json_number(quantized, path)
        }
        ParameterSpecV1::Enum { values, .. } => {
            let value = raw.as_str().ok_or_else(|| {
                invalid_field(path, "expected a string for enum parameter")
            })?;
            if !values.iter().any(|entry| entry == value) {
                return Err(invalid_field(
                    path,
                    format!("invalid enum value '{value}'"),
                ));
            }
            Ok(JsonValue::String(value.to_string()))
        }
    }
}

fn quantize_float(value: f64, range: &FloatRange) -> f64 {
    let clamped = value.clamp(range.min, range.max);
    if range.step == 0.0 {
        return clamped;
    }
    let steps = ((clamped - range.min) / range.step).round();
    let quantized = range.min + steps * range.step;
    quantized.clamp(range.min, range.max)
}

fn json_number(value: f64, path: &str) -> Result<JsonValue, PolicyError> {
    let number = serde_json::Number::from_f64(value)
        .ok_or_else(|| invalid_field(path, "number must be finite"))?;
    Ok(JsonValue::Number(number))
}
