//! Reduced recoverable throttle hesitation, not collision/spin/retirement physics.
//! Pure distance exposure: retries, numerical iterations and export cadence draw nothing.
use md5::{Digest as _, Md5};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DriverLapseVersion {
    #[serde(rename = "pitgun.driver-throttle-lapse/v1")]
    V1,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DriverLapseParamsV1 {
    pub schema_version: DriverLapseVersion,
    /// Actor-owned noise identity, separate from named ability/catalog identity.
    pub noise_stream_id: String,
    /// First-arrival hazard within each independent distance cell, per100km.
    pub hazard_per_100km: f64,
    pub exposure_cell_m: f64,
    /// Spatial footprint of a recoverable hesitation; not a measured reaction time.
    pub recovery_distance_m: f64,
    pub minimum_engine_request_fraction: f64,
}
impl DriverLapseParamsV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.noise_stream_id.is_empty()
            || self.noise_stream_id.len() > 128
            || !self.hazard_per_100km.is_finite()
            || !(0.0..=1000.0).contains(&self.hazard_per_100km)
            || !self.exposure_cell_m.is_finite()
            || !(100.0..=5000.0).contains(&self.exposure_cell_m)
            || !self.recovery_distance_m.is_finite()
            || self.recovery_distance_m <= 0.0
            || self.recovery_distance_m > self.exposure_cell_m
            || !self.minimum_engine_request_fraction.is_finite()
            || !(0.0..=1.0).contains(&self.minimum_engine_request_fraction)
        {
            return Err("driver lapse: invalid distance-exposure profile".into());
        }
        Ok(())
    }
    pub fn cell_probability(&self) -> f64 {
        -(-self.hazard_per_100km * self.exposure_cell_m / 100000.0).exp_m1()
    }
    /// At most one first-arrival event per cell. This is not a full Poisson process.
    pub fn intervals(
        &self,
        seed: u64,
        from_m: f64,
        to_m: f64,
    ) -> Result<Vec<LapseIntervalV1>, String> {
        self.validate()?;
        if !from_m.is_finite() || !to_m.is_finite() || from_m < 0.0 || to_m < from_m || to_m > 1e9 {
            return Err("driver lapse: invalid exposure range".into());
        }
        if self.hazard_per_100km == 0.0 || to_m == from_m {
            return Ok(Vec::new());
        }
        let first =
            ((from_m - self.recovery_distance_m).max(0.0) / self.exposure_cell_m).floor() as u64;
        let last = (to_m / self.exposure_cell_m).floor() as u64;
        if last - first > 100000 {
            return Err("driver lapse: exposure budget exceeded".into());
        }
        let mut result = Vec::new();
        for index in first..=last {
            let id = format!(
                "pitgun.throttle-lapse/v1:{seed}:{}:{index}",
                self.noise_stream_id
            );
            let hash = Md5::digest(id.as_bytes());
            let raw = u64::from_be_bytes(hash[..8].try_into().unwrap());
            // Strictly inside (0,1), with representable half-integer52-bit draws.
            let uniform = ((raw >> 12) as f64 + 0.5) / 4503599627370496.0;
            let offset = -(-uniform).ln_1p() * 100000.0 / self.hazard_per_100km;
            if offset >= self.exposure_cell_m {
                continue;
            }
            let start = index as f64 * self.exposure_cell_m + offset;
            let end = start + self.recovery_distance_m;
            if start < to_m && end > from_m {
                result.push(LapseIntervalV1 {
                    exposure_index: index,
                    start_distance_m: start,
                    end_distance_m: end,
                });
            }
        }
        Ok(result)
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LapseIntervalV1 {
    pub exposure_index: u64,
    pub start_distance_m: f64,
    pub end_distance_m: f64,
}

/// Accepted native observation of a known modeled onset. No sample-exact/root-time claim.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DriverLapseOccurrenceV1 {
    pub schema_version: DriverLapseVersion,
    pub exposure_index: u64,
    pub lap: u16,
    pub start_distance_m: f64,
    pub end_distance_m: f64,
    pub observed_native_point: u32,
    pub observed_at_s: f64,
    pub minimum_engine_request_fraction: f64,
}

/// Distance-averaged engine-request cap over a native segment. Overlaps are a union,
/// never double-charged. A driver request reduction cannot create a speed jump.
pub fn segment_engine_request_fraction(
    intervals: &[LapseIntervalV1],
    from: f64,
    to: f64,
    minimum: f64,
) -> f64 {
    if to <= from {
        return 1.0;
    }
    let mut covered = 0.0;
    let mut until = from;
    for i in intervals {
        let start = i.start_distance_m.max(from).max(until);
        let end = i.end_distance_m.min(to);
        if end > start {
            covered += end - start;
            until = end;
        }
    }
    (1.0 - (1.0 - minimum) * covered / (to - from)).clamp(minimum, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn profile() -> DriverLapseParamsV1 {
        DriverLapseParamsV1 {
            schema_version: DriverLapseVersion::V1,
            noise_stream_id: "player".into(),
            hazard_per_100km: 1.0,
            exposure_cell_m: 500.0,
            recovery_distance_m: 250.0,
            minimum_engine_request_fraction: 0.15,
        }
    }
    #[test]
    fn hazard_and_seeded_exposure_are_monotone_and_independent_of_chunking() {
        let mut p = profile();
        p.hazard_per_100km = 20.0;
        let all = p.intervals(42, 0.0, 50000.0).unwrap();
        assert!(!all.is_empty());
        let mut split = p.intervals(42, 0.0, 27000.0).unwrap();
        split.extend(p.intervals(42, 27000.0, 50000.0).unwrap());
        split.sort_by_key(|i| i.exposure_index);
        split.dedup_by_key(|i| i.exposure_index);
        assert_eq!(split, all);
        assert_ne!(all, p.intervals(43, 0.0, 50000.0).unwrap());
        let high = p.cell_probability();
        p.hazard_per_100km = 1.0;
        assert!(p.cell_probability() < high);
        p.hazard_per_100km = 0.0;
        assert_eq!(p.cell_probability(), 0.0);
        assert!(p.intervals(42, 0.0, 50000.0).unwrap().is_empty());
    }
    #[test]
    fn segment_losses_are_bounded_and_conserved_under_spatial_partition() {
        let intervals = [
            LapseIntervalV1 {
                exposure_index: 0,
                start_distance_m: 5.0,
                end_distance_m: 15.0,
            },
            LapseIntervalV1 {
                exposure_index: 1,
                start_distance_m: 12.0,
                end_distance_m: 18.0,
            },
        ];
        let coarse = 20.0 * (1.0 - segment_engine_request_fraction(&intervals, 0.0, 20.0, 0.2));
        let fine = (0..20)
            .map(|i| {
                1.0 - segment_engine_request_fraction(&intervals, i as f64, (i + 1) as f64, 0.2)
            })
            .sum::<f64>();
        assert!((coarse - 13.0 * 0.8).abs() < 1e-12);
        assert!((coarse - fine).abs() < 1e-12);
        assert_eq!(
            segment_engine_request_fraction(&intervals, 20.0, 30.0, 0.2),
            1.0
        );
    }
    #[test]
    fn malformed_profiles_and_ranges_fail_before_drawing() {
        let mut p = profile();
        p.recovery_distance_m = f64::NAN;
        assert!(p.intervals(0, 0.0, 10.0).is_err());
        let p = profile();
        assert!(p.intervals(0, 10.0, 0.0).is_err());
        assert!(p.intervals(0, 0.0, f64::INFINITY).is_err());
    }
}
