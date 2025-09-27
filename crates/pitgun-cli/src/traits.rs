use anyhow::Result;
use crate::types::{Signal, Frame};

/// Un opérateur de transformation de signal ou de frame.
/// Exemple : filtre, normalisation, resample…
pub trait Transform {
    fn name(&self) -> &'static str;

    fn apply_signal(&self, s: &Signal) -> Result<Signal>;

    fn apply_frame(&self, f: &Frame) -> Result<Frame> {
        let signals = f.signals
            .iter()
            .map(|s| self.apply_signal(s))
            .collect::<Result<Vec<_>>>()?;
        Ok(Frame {
            signals,
            start: f.start,
            end: f.end,
        })
    }
}

/// Extraction d’une caractéristique (scalaire ou struct) depuis une Frame.
/// Exemple : max, RMS, FFT peak…
pub trait FeatureExtractor {
    type Output;
    fn extract(&self, f: &Frame) -> Result<Self::Output>;
}