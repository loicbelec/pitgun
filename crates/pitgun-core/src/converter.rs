//! Converter Service
//!
//! Provides value conversion for telemetry parameters using different
//! conversion methods: linear, polynomial, and table-based interpolation.
//!
//! # Example
//!
//! ```rust,ignore
//! use pitgun_core::converter::{ConverterService, ConversionMethod};
//!
//! let mut converter = ConverterService::new();
//!
//! // Linear: y = 0.1 * x - 40 (temperature sensor)
//! converter.register(42, ConversionMethod::Linear { scale: 0.1, offset: -40.0 });
//!
//! // Polynomial: y = a0 + a1*x + a2*x^2
//! converter.register(43, ConversionMethod::Polynomial {
//!     coefficients: vec![0.0, 0.001, 0.00001],
//! });
//!
//! // Table lookup with interpolation
//! converter.register(44, ConversionMethod::Table {
//!     breakpoints: vec![0.0, 100.0, 200.0, 255.0],
//!     values: vec![0.0, 1.0, 4.0, 10.0],
//! });
//!
//! let converted = converter.convert(42, 1200.0)?; // Returns 80.0
//! ```

use pitgun_contract::{ParameterRegistry, TelemetryFrame, TelemetrySample};
use std::collections::HashMap;

/// Conversion method for a parameter
#[derive(Clone, Debug)]
pub enum ConversionMethod {
    /// No conversion (pass-through)
    Identity,

    /// Linear conversion: y = scale * x + offset
    Linear { scale: f64, offset: f64 },

    /// Polynomial conversion: y = sum(coefficients[i] * x^i)
    Polynomial { coefficients: Vec<f64> },

    /// Table-based conversion with linear interpolation
    Table {
        /// Input breakpoints (must be sorted ascending)
        breakpoints: Vec<f64>,
        /// Output values corresponding to breakpoints
        values: Vec<f64>,
    },

    /// Rational polynomial: y = (a0 + a1*x + a2*x^2) / (b0 + b1*x + b2*x^2)
    Rational {
        numerator: Vec<f64>,
        denominator: Vec<f64>,
    },

    /// Logarithmic: y = scale * ln(x + offset)
    Logarithmic { scale: f64, offset: f64 },

    /// Custom conversion via closure (not serializable)
    #[allow(clippy::type_complexity)]
    Custom(fn(f64) -> f64),
}

impl ConversionMethod {
    /// Creates a linear conversion
    pub fn linear(scale: f64, offset: f64) -> Self {
        Self::Linear { scale, offset }
    }

    /// Creates a polynomial conversion
    pub fn polynomial(coefficients: Vec<f64>) -> Self {
        Self::Polynomial { coefficients }
    }

    /// Creates a table conversion
    pub fn table(breakpoints: Vec<f64>, values: Vec<f64>) -> Self {
        Self::Table { breakpoints, values }
    }

    /// Applies the conversion to a value
    pub fn apply(&self, x: f64) -> f64 {
        match self {
            Self::Identity => x,

            Self::Linear { scale, offset } => scale * x + offset,

            Self::Polynomial { coefficients } => {
                let mut result = 0.0;
                let mut x_power = 1.0;
                for coef in coefficients {
                    result += coef * x_power;
                    x_power *= x;
                }
                result
            }

            Self::Table { breakpoints, values } => {
                if breakpoints.is_empty() || values.is_empty() {
                    return x;
                }

                // Handle out of range
                if x <= breakpoints[0] {
                    return values[0];
                }
                if x >= breakpoints[breakpoints.len() - 1] {
                    return values[values.len() - 1];
                }

                // Find the interval and interpolate
                for i in 0..breakpoints.len() - 1 {
                    if x >= breakpoints[i] && x <= breakpoints[i + 1] {
                        let t = (x - breakpoints[i]) / (breakpoints[i + 1] - breakpoints[i]);
                        return values[i] + t * (values[i + 1] - values[i]);
                    }
                }

                x
            }

            Self::Rational {
                numerator,
                denominator,
            } => {
                let num = Self::eval_polynomial(numerator, x);
                let den = Self::eval_polynomial(denominator, x);
                if den.abs() < 1e-15 {
                    f64::NAN
                } else {
                    num / den
                }
            }

            Self::Logarithmic { scale, offset } => {
                let arg = x + offset;
                if arg <= 0.0 {
                    f64::NAN
                } else {
                    scale * arg.ln()
                }
            }

            Self::Custom(f) => f(x),
        }
    }

    /// Evaluates a polynomial
    fn eval_polynomial(coefficients: &[f64], x: f64) -> f64 {
        let mut result = 0.0;
        let mut x_power = 1.0;
        for coef in coefficients {
            result += coef * x_power;
            x_power *= x;
        }
        result
    }
}

/// Converter service for parameter value conversion
#[derive(Default)]
pub struct ConverterService {
    conversions: HashMap<u32, ConversionMethod>,
}

impl ConverterService {
    /// Creates a new converter service
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a converter service from a parameter registry
    ///
    /// Extracts conversion methods from parameter metadata if available.
    pub fn from_registry(registry: &ParameterRegistry) -> Self {
        let mut service = Self::new();

        // Iterate through registry and extract conversions
        // This assumes the registry has conversion info stored
        for (id, def) in registry.iter() {
            // Check for conversion metadata
            // Default to identity if no conversion defined
            if def.unit.is_some() {
                // If there's a unit, the parameter might need conversion
                // For now, register identity - real impl would parse conversion from def
                service.register(*id, ConversionMethod::Identity);
            }
        }

        service
    }

    /// Registers a conversion method for a parameter
    pub fn register(&mut self, parameter_id: u32, method: ConversionMethod) {
        self.conversions.insert(parameter_id, method);
    }

    /// Unregisters a conversion method
    pub fn unregister(&mut self, parameter_id: u32) -> Option<ConversionMethod> {
        self.conversions.remove(&parameter_id)
    }

    /// Checks if a parameter has a conversion registered
    pub fn has_conversion(&self, parameter_id: u32) -> bool {
        self.conversions.contains_key(&parameter_id)
    }

    /// Gets the conversion method for a parameter
    pub fn get(&self, parameter_id: u32) -> Option<&ConversionMethod> {
        self.conversions.get(&parameter_id)
    }

    /// Converts a single value
    pub fn convert(&self, parameter_id: u32, value: f64) -> f64 {
        match self.conversions.get(&parameter_id) {
            Some(method) => method.apply(value),
            None => value, // Pass through if no conversion
        }
    }

    /// Converts a telemetry sample in place
    pub fn convert_sample(&self, sample: &mut TelemetrySample) {
        if let Some(method) = self.conversions.get(&sample.parameter_id) {
            sample.value = method.apply(sample.value);
        }
    }

    /// Converts all samples in a frame
    ///
    /// Returns a new frame with converted values.
    pub fn convert_frame(&self, frame: &TelemetryFrame) -> TelemetryFrame {
        let mut converted_samples = Vec::with_capacity(frame.sample_count());

        for sample in frame.samples() {
            let mut new_sample = sample.clone();
            if let Some(method) = self.conversions.get(&sample.parameter_id) {
                new_sample.value = method.apply(sample.value);
            }
            converted_samples.push(new_sample);
        }

        TelemetryFrame::new(
            frame.source_id(),
            frame.sequence(),
            frame.timestamp(),
            converted_samples,
        )
    }

    /// Returns the number of registered conversions
    pub fn len(&self) -> usize {
        self.conversions.len()
    }

    /// Checks if the service has no conversions
    pub fn is_empty(&self) -> bool {
        self.conversions.is_empty()
    }

    /// Returns an iterator over parameter IDs with conversions
    pub fn parameter_ids(&self) -> impl Iterator<Item = &u32> {
        self.conversions.keys()
    }
}

/// Batch converter for high-throughput scenarios
pub struct BatchConverter {
    service: ConverterService,
    buffer: Vec<f64>,
}

impl BatchConverter {
    /// Creates a new batch converter
    pub fn new(service: ConverterService) -> Self {
        Self {
            service,
            buffer: Vec::with_capacity(1024),
        }
    }

    /// Converts a batch of values for a single parameter
    pub fn convert_batch(&mut self, parameter_id: u32, values: &[f64]) -> &[f64] {
        self.buffer.clear();
        self.buffer.reserve(values.len());

        if let Some(method) = self.service.get(parameter_id) {
            for &value in values {
                self.buffer.push(method.apply(value));
            }
        } else {
            self.buffer.extend_from_slice(values);
        }

        &self.buffer
    }

    /// Converts values in place
    pub fn convert_in_place(&self, parameter_id: u32, values: &mut [f64]) {
        if let Some(method) = self.service.get(parameter_id) {
            for value in values {
                *value = method.apply(*value);
            }
        }
    }
}

/// Conversion table builder for importing from external sources
pub struct ConversionTableBuilder {
    breakpoints: Vec<f64>,
    values: Vec<f64>,
}

impl ConversionTableBuilder {
    /// Creates a new table builder
    pub fn new() -> Self {
        Self {
            breakpoints: Vec::new(),
            values: Vec::new(),
        }
    }

    /// Adds a point to the table
    pub fn add_point(mut self, breakpoint: f64, value: f64) -> Self {
        self.breakpoints.push(breakpoint);
        self.values.push(value);
        self
    }

    /// Adds multiple points
    pub fn add_points(mut self, points: &[(f64, f64)]) -> Self {
        for (bp, val) in points {
            self.breakpoints.push(*bp);
            self.values.push(*val);
        }
        self
    }

    /// Sorts the points by breakpoint and builds the conversion method
    pub fn build(mut self) -> ConversionMethod {
        // Sort by breakpoint
        let mut pairs: Vec<_> = self
            .breakpoints
            .drain(..)
            .zip(self.values.drain(..))
            .collect();
        pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        let (breakpoints, values): (Vec<_>, Vec<_>) = pairs.into_iter().unzip();

        ConversionMethod::Table { breakpoints, values }
    }
}

impl Default for ConversionTableBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_conversion() {
        let method = ConversionMethod::linear(0.1, -40.0);
        assert!((method.apply(1200.0) - 80.0).abs() < 1e-10);
        assert!((method.apply(400.0) - 0.0).abs() < 1e-10);
    }

    #[test]
    fn polynomial_conversion() {
        // y = 1 + 2x + 3x^2
        let method = ConversionMethod::polynomial(vec![1.0, 2.0, 3.0]);
        assert!((method.apply(0.0) - 1.0).abs() < 1e-10);
        assert!((method.apply(1.0) - 6.0).abs() < 1e-10); // 1 + 2 + 3
        assert!((method.apply(2.0) - 17.0).abs() < 1e-10); // 1 + 4 + 12
    }

    #[test]
    fn table_conversion() {
        let method = ConversionMethod::table(
            vec![0.0, 100.0, 200.0],
            vec![0.0, 10.0, 40.0],
        );

        // Exact points
        assert!((method.apply(0.0) - 0.0).abs() < 1e-10);
        assert!((method.apply(100.0) - 10.0).abs() < 1e-10);
        assert!((method.apply(200.0) - 40.0).abs() < 1e-10);

        // Interpolated
        assert!((method.apply(50.0) - 5.0).abs() < 1e-10);
        assert!((method.apply(150.0) - 25.0).abs() < 1e-10);

        // Out of range
        assert!((method.apply(-10.0) - 0.0).abs() < 1e-10);
        assert!((method.apply(250.0) - 40.0).abs() < 1e-10);
    }

    #[test]
    fn converter_service() {
        let mut service = ConverterService::new();
        service.register(1, ConversionMethod::linear(2.0, 0.0));
        service.register(2, ConversionMethod::linear(1.0, 10.0));

        assert!((service.convert(1, 5.0) - 10.0).abs() < 1e-10);
        assert!((service.convert(2, 5.0) - 15.0).abs() < 1e-10);
        assert!((service.convert(99, 5.0) - 5.0).abs() < 1e-10); // Unknown, pass-through
    }

    #[test]
    fn batch_converter() {
        let mut service = ConverterService::new();
        service.register(1, ConversionMethod::linear(2.0, 0.0));

        let mut batch = BatchConverter::new(service);
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let converted = batch.convert_batch(1, &values);

        assert_eq!(converted, &[2.0, 4.0, 6.0, 8.0, 10.0]);
    }

    #[test]
    fn table_builder() {
        let method = ConversionTableBuilder::new()
            .add_point(100.0, 10.0)
            .add_point(0.0, 0.0)
            .add_point(200.0, 40.0)
            .build();

        // Points should be sorted
        assert!((method.apply(50.0) - 5.0).abs() < 1e-10);
    }

    #[test]
    fn rational_conversion() {
        // y = (1 + x) / (1 + 0.1x)
        let method = ConversionMethod::Rational {
            numerator: vec![1.0, 1.0],
            denominator: vec![1.0, 0.1],
        };

        assert!((method.apply(0.0) - 1.0).abs() < 1e-10);
        assert!((method.apply(10.0) - 11.0 / 2.0).abs() < 1e-10);
    }

    #[test]
    fn logarithmic_conversion() {
        let method = ConversionMethod::Logarithmic {
            scale: 1.0,
            offset: 0.0,
        };

        assert!((method.apply(1.0) - 0.0).abs() < 1e-10);
        assert!((method.apply(std::f64::consts::E) - 1.0).abs() < 1e-10);
        assert!(method.apply(-1.0).is_nan());
    }
}
