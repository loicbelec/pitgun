//! Conservative distance-domain enclosure of the implemented V2 tire laws.
//! No empirical coefficients or future observations enter this forecast.
use crate::{
    MechanicalParamsV3, TireParams, Track, VehicleParams, thermal_physics::TireHeatParamsV2,
};

pub(crate) struct ReachabilityContext<'a> {
    pub vehicle: &'a VehicleParams,
    pub mechanical: &'a MechanicalParamsV3,
    pub track: &'a Track,
    pub vertical_curvature: &'a [f64],
    pub tire: &'a TireParams,
    /// Includes the already-accounted correction-work multiplier.
    pub heat: TireHeatParamsV2,
    pub mass_upper_kg: f64,
    pub air_c: f64,
    pub road_c: f64,
}
#[derive(Debug, Clone)]
pub(crate) struct ReachableTireStates {
    pub temperature_lower_c: f64,
    pub temperature_upper_c: Vec<f64>,
    pub wear_upper: Vec<f64>,
}
impl ReachabilityContext<'_> {
    pub fn speed_upper_mps(&self, configured: f64) -> f64 {
        let e = &self.vehicle.engine;
        let redline = e
            .gear_ratios
            .iter()
            .map(|r| e.n_max * std::f64::consts::TAU * self.vehicle.chassis.r_wheel / (60.0 * r))
            .fold(0.0_f64, f64::max);
        configured.min(redline)
    }
    pub fn bound(
        &self,
        surface_c: f64,
        carcass_c: f64,
        wear: f64,
        configured_speed: f64,
    ) -> Result<ReachableTireStates, String> {
        let p = self.heat;
        let n = self.track.s.len();
        if n < 3
            || self.vertical_curvature.len() != n
            || ![
                surface_c,
                carcass_c,
                wear,
                self.mass_upper_kg,
                configured_speed,
                self.air_c,
                self.road_c,
            ]
            .iter()
            .all(|v| v.is_finite())
            || !(0.0..=1.0).contains(&wear)
            || self.mass_upper_kg < self.vehicle.chassis.mass_empty
            || configured_speed <= 0.0
        {
            return Err("reachable tire bound: invalid initial state or geometry".into());
        }
        let speed = self.speed_upper_mps(configured_speed);
        let c = &self.vehicle.chassis;
        let lower = surface_c.min(carcass_c).min(self.air_c).min(self.road_c);
        let initial_upper = surface_c.max(carcass_c).max(self.air_c).max(self.road_c);
        let capacity = p.surface_capacity_j_per_k.min(p.carcass_capacity_j_per_k);
        let mut heat_upper = 0.0;
        let mut wear_upper = vec![wear; n];
        let mut temperatures = vec![initial_upper; n];
        for i in 0..n - 1 {
            let ds = self.track.s[i + 1] - self.track.s[i];
            // Drop negative vertical acceleration; retain all possible positive load.
            let normal = self.mass_upper_kg
                * (c.g + speed * speed * self.vertical_curvature[i].max(0.0))
                + 0.5 * c.rho * self.mechanical.fixed_downforce_area_m2 * speed * speed;
            let rolling = c.c_rr * normal * ds;
            let deformation = p.deformation_resistance_coefficient * normal * ds;
            heat_upper +=
                p.rolling_heat_fraction * rolling + p.deformation_heat_fraction * deformation;
            temperatures[i + 1] = initial_upper + heat_upper / capacity;
            let deviation = ((lower - self.tire.temp_opt) / self.tire.temp_sigma)
                .abs()
                .max(((temperatures[i + 1] - self.tire.temp_opt) / self.tire.temp_sigma).abs());
            let increment = (deformation + p.rolling_wear_fraction * rolling)
                / p.deformation_energy_to_full_wear_j
                * (1.0 + p.thermal_wear_gain * deviation * deviation);
            wear_upper[i + 1] = (wear_upper[i] + increment).min(1.0);
        }
        if temperatures
            .iter()
            .chain(&wear_upper)
            .any(|v| !v.is_finite())
        {
            return Err("reachable tire bound: nonfinite enclosure".into());
        }
        Ok(ReachableTireStates {
            temperature_lower_c: lower,
            temperature_upper_c: temperatures,
            wear_upper,
        })
    }
}

impl ReachableTireStates {
    pub fn contains(&self, temperatures: &[f64], carcass: &[f64], wear: &[f64]) -> bool {
        let n = self.wear_upper.len();
        temperatures.len() == n
            && carcass.len() == n
            && wear.len() == n
            && (0..n).all(|i| {
                temperatures[i].is_finite()
                    && carcass[i].is_finite()
                    && wear[i].is_finite()
                    && temperatures[i] >= self.temperature_lower_c - 1e-9
                    && carcass[i] >= self.temperature_lower_c - 1e-9
                    && temperatures[i] <= self.temperature_upper_c[i] + 1e-9
                    && carcass[i] <= self.temperature_upper_c[i] + 1e-9
                    && wear[i] <= self.wear_upper[i] + 1e-12
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distance_enclosure_contains_two_node_heat_and_wear_under_variable_time_and_load() {
        // Read an existing physical fixture; the excitation below is manufactured,
        // not another lap solver or a fitted trace from the new candidate.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let rows: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.join("experiments/thermal_physics_v2/results/screen-v1.json"))
                .unwrap(),
        )
        .unwrap();
        let p = &rows["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["case"] == "tires-medium" && r["candidate"] == true)
            .unwrap()["data"]["resolved_parameters"];
        let vehicle: VehicleParams = serde_json::from_value(p["vehicle"].clone()).unwrap();
        let mechanical: MechanicalParamsV3 =
            serde_json::from_value(p["mechanical"].clone()).unwrap();
        let profile: crate::ThermalPhysicsV2 =
            serde_json::from_value(p["thermal_physics_v2"].clone()).unwrap();
        let mut heat = *profile.tire(&vehicle.tire).unwrap();
        heat.deformation_resistance_coefficient *= 3.0;
        let n = 121;
        let mut positions = vec![0.0];
        for i in 1..n {
            positions.push(positions[i - 1] + if i % 2 == 0 { 0.1 } else { 17.0 });
        }
        let track = Track {
            s: positions,
            x: vec![0.0; n],
            y: vec![0.0; n],
            z: vec![0.0; n],
            kappa: vec![0.0; n],
            slope: vec![0.0; n],
            heading: vec![0.0; n],
        };
        let vertical: Vec<_> = (0..n)
            .map(|i| if i % 3 == 0 { -0.003 } else { 0.002 })
            .collect();
        let mass_upper = vehicle.chassis.mass_empty + 100.0;
        let context = ReachabilityContext {
            vehicle: &vehicle,
            mechanical: &mechanical,
            track: &track,
            vertical_curvature: &vertical,
            tire: &vehicle.tire,
            heat,
            mass_upper_kg: mass_upper,
            air_c: 15.0,
            road_c: 60.0,
        };
        for initial in [0.0, 35.0, 70.0, 110.0, 200.0] {
            let b = context.bound(initial, initial, 0.35, 140.0).unwrap();
            let mut surface = vec![initial];
            let mut carcass = vec![initial];
            let mut wears = vec![0.35];
            for i in 0..n - 1 {
                let ds = track.s[i + 1] - track.s[i];
                let speed = context.speed_upper_mps(140.0) * (0.2 + 0.8 * (i % 7) as f64 / 6.0);
                let mass = vehicle.chassis.mass_empty + 100.0 * (1.0 - i as f64 / n as f64);
                let normal = (mass * (vehicle.chassis.g + speed * speed * vertical[i])
                    + 0.5
                        * vehicle.chassis.rho
                        * mechanical.fixed_downforce_area_m2
                        * speed
                        * speed)
                    .max(0.0);
                let roll = vehicle.chassis.c_rr * normal * ds;
                let deform = heat.deformation_resistance_coefficient
                    * normal
                    * ds
                    * ((i % 5) as f64 / 4.0).powi(2);
                let duration = if i % 2 == 0 { 120.0 } else { 0.05 };
                let (ts, tb, dw, _) = heat
                    .step(
                        surface[i],
                        carcass[i],
                        15.0,
                        60.0,
                        speed,
                        roll,
                        deform,
                        duration,
                        &vehicle.tire,
                    )
                    .unwrap();
                surface.push(ts);
                carcass.push(tb);
                wears.push((wears[i] + dw).min(1.0));
            }
            assert!(b.contains(&surface, &carcass, &wears));
            let mut invalid = surface.clone();
            invalid[3] = b.temperature_upper_c[3] + 1.0;
            assert!(!b.contains(&invalid, &carcass, &wears));
        }
    }
}
