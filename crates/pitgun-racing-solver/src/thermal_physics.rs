//! Opt-in reduced thermal candidate. Parameters are physical inputs, never HQ points.
//! No battery, measured slip, four-wheel distribution or experimental identification.
use crate::{FuelMassParamsV3, TireParams, longitudinal_energy::thermal_step};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ThermalModelV2 {
    #[serde(rename = "pitgun.racing-thermal/v2")]
    V2,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ThermalPhysicsV2 {
    pub schema_version: ThermalModelV2,
    pub engine: EngineHeatParamsV2,
    pub compounds: Vec<TireCompoundHeatV2>,
    pub initial_carcass_temperature_c: f64,
    pub pit_carcass_temperature_c: f64,
    pub road_temperature_c: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EngineHeatParamsV2 {
    pub fuel_lower_heating_value_j_per_kg: f64,
    /// Fraction of chemical input minus shaft work entering the lumped engine/coolant.
    pub coolant_waste_fraction: f64,
    pub passive_conductance_w_per_k: f64,
    pub radiator_idle_conductance_w_per_k: f64,
    pub radiator_speed_conductance_w_per_k_mps: f64,
    pub thermostat_start_c: f64,
    pub thermostat_full_c: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TireCompoundHeatV2 {
    /// Exact resolved tire identity, also for a replacement at a service boundary.
    pub tire: TireParams,
    pub heat: TireHeatParamsV2,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TireHeatParamsV2 {
    /// Aggregate of all four tires; surface is the grip temperature.
    pub surface_capacity_j_per_k: f64,
    pub carcass_capacity_j_per_k: f64,
    pub surface_carcass_conductance_w_per_k: f64,
    pub surface_air_idle_conductance_w_per_k: f64,
    pub surface_air_speed_conductance_w_per_k_mps: f64,
    pub carcass_air_conductance_w_per_k: f64,
    pub surface_road_conductance_w_per_k: f64,
    /// Reduced deformation resistance k*N*u^2. This is not measured tire slip.
    pub deformation_resistance_coefficient: f64,
    pub rolling_heat_fraction: f64,
    pub deformation_heat_fraction: f64,
    pub deformation_surface_fraction: f64,
    pub deformation_energy_to_full_wear_j: f64,
    pub rolling_wear_fraction: f64,
    pub thermal_wear_gain: f64,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct ThermalLedgerV2 {
    pub maximum_relative_grip_coupling_error: f64,
    pub maximum_contact_utilization_upper_bound: f64,
    pub maximum_coupling_passes: u32,
    pub fuel_chemical_energy_j: f64,
    pub engine_shaft_work_j: f64,
    pub engine_coolant_heat_j: f64,
    pub exhaust_and_other_energy_j: f64,
    pub engine_ambient_exchange_signed_j: f64,
    pub engine_storage_change_j: f64,
    pub tire_rolling_dissipation_j: f64,
    pub tire_deformation_dissipation_j: f64,
    pub tire_generated_heat_j: f64,
    pub tire_direct_environment_heat_j: f64,
    pub tire_road_exchange_signed_j: f64,
    pub tire_air_exchange_signed_j: f64,
    pub tire_storage_change_j: f64,
    pub maximum_engine_balance_residual_j: f64,
    pub maximum_tire_balance_residual_j: f64,
    pub maximum_fuel_balance_residual_j: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThermalResultV2 {
    pub profile: ThermalPhysicsV2,
    pub final_carcass_temperature_c: f64,
    pub ledger: ThermalLedgerV2,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct ThermalLapV2 {
    pub carcass_temperature_c: f64,
    pub ledger: ThermalLedgerV2,
}
impl ThermalLedgerV2 {
    pub fn accumulate(&mut self, other: &Self) {
        self.maximum_relative_grip_coupling_error = self
            .maximum_relative_grip_coupling_error
            .max(other.maximum_relative_grip_coupling_error);
        self.maximum_contact_utilization_upper_bound = self
            .maximum_contact_utilization_upper_bound
            .max(other.maximum_contact_utilization_upper_bound);
        self.maximum_coupling_passes = self
            .maximum_coupling_passes
            .max(other.maximum_coupling_passes);
        self.fuel_chemical_energy_j += other.fuel_chemical_energy_j;
        self.engine_shaft_work_j += other.engine_shaft_work_j;
        self.engine_coolant_heat_j += other.engine_coolant_heat_j;
        self.exhaust_and_other_energy_j += other.exhaust_and_other_energy_j;
        self.engine_ambient_exchange_signed_j += other.engine_ambient_exchange_signed_j;
        self.engine_storage_change_j += other.engine_storage_change_j;
        self.tire_rolling_dissipation_j += other.tire_rolling_dissipation_j;
        self.tire_deformation_dissipation_j += other.tire_deformation_dissipation_j;
        self.tire_generated_heat_j += other.tire_generated_heat_j;
        self.tire_direct_environment_heat_j += other.tire_direct_environment_heat_j;
        self.tire_road_exchange_signed_j += other.tire_road_exchange_signed_j;
        self.tire_air_exchange_signed_j += other.tire_air_exchange_signed_j;
        self.tire_storage_change_j += other.tire_storage_change_j;
        self.maximum_engine_balance_residual_j = self
            .maximum_engine_balance_residual_j
            .max(other.maximum_engine_balance_residual_j);
        self.maximum_tire_balance_residual_j = self
            .maximum_tire_balance_residual_j
            .max(other.maximum_tire_balance_residual_j);
        self.maximum_fuel_balance_residual_j = self
            .maximum_fuel_balance_residual_j
            .max(other.maximum_fuel_balance_residual_j);
    }
}

impl ThermalPhysicsV2 {
    pub fn validate(&self, fuel: &FuelMassParamsV3) -> Result<(), String> {
        let e = self.engine;
        if ![
            e.fuel_lower_heating_value_j_per_kg,
            e.coolant_waste_fraction,
            e.passive_conductance_w_per_k,
            e.radiator_idle_conductance_w_per_k,
            e.radiator_speed_conductance_w_per_k_mps,
            e.thermostat_start_c,
            e.thermostat_full_c,
            self.initial_carcass_temperature_c,
            self.pit_carcass_temperature_c,
            self.road_temperature_c,
        ]
        .iter()
        .all(|x| x.is_finite())
            || !(30e6..=50e6).contains(&e.fuel_lower_heating_value_j_per_kg)
            || !(0.0..=1.0).contains(&e.coolant_waste_fraction)
            || e.passive_conductance_w_per_k < 0.0
            || e.radiator_idle_conductance_w_per_k < 0.0
            || e.radiator_speed_conductance_w_per_k_mps < 0.0
            || !(0.0..=150.0).contains(&e.thermostat_start_c)
            || e.thermostat_full_c <= e.thermostat_start_c
            || e.thermostat_full_c > 180.0
            || ![
                self.initial_carcass_temperature_c,
                self.pit_carcass_temperature_c,
                self.road_temperature_c,
            ]
            .iter()
            .all(|t| (0.0..=200.0).contains(t))
            || fuel.brake_specific_fuel_consumption_kg_per_kwh * e.fuel_lower_heating_value_j_per_kg
                < 3_600_000.0
            || self.compounds.is_empty()
            || self.compounds.len() > 16
        {
            return Err("thermal V2: invalid engine, environment or chemical efficiency".into());
        }
        for (i, c) in self.compounds.iter().enumerate() {
            crate::validate_tire_params(&c.tire)?;
            let p = c.heat;
            if ![
                p.surface_capacity_j_per_k,
                p.carcass_capacity_j_per_k,
                p.surface_carcass_conductance_w_per_k,
                p.surface_air_idle_conductance_w_per_k,
                p.surface_air_speed_conductance_w_per_k_mps,
                p.carcass_air_conductance_w_per_k,
                p.surface_road_conductance_w_per_k,
                p.deformation_resistance_coefficient,
                p.rolling_heat_fraction,
                p.deformation_heat_fraction,
                p.deformation_surface_fraction,
                p.deformation_energy_to_full_wear_j,
                p.rolling_wear_fraction,
                p.thermal_wear_gain,
            ]
            .iter()
            .all(|v| v.is_finite() && *v >= 0.0)
                || p.surface_capacity_j_per_k < 100.0
                || p.carcass_capacity_j_per_k < 100.0
                || p.deformation_energy_to_full_wear_j < 1.0
                || p.deformation_resistance_coefficient > 0.1
                || [
                    p.rolling_heat_fraction,
                    p.deformation_heat_fraction,
                    p.deformation_surface_fraction,
                    p.rolling_wear_fraction,
                ]
                .iter()
                .any(|f| *f > 1.0)
                || self.compounds[..i]
                    .iter()
                    .any(|previous| previous.tire == c.tire)
            {
                return Err("thermal V2: invalid or duplicate tire heat parameters".into());
            }
        }
        Ok(())
    }
    pub fn tire(&self, tire: &TireParams) -> Result<&TireHeatParamsV2, String> {
        self.compounds
            .iter()
            .find(|c| &c.tire == tire)
            .map(|c| &c.heat)
            .ok_or_else(|| "thermal V2: unresolved compound heat parameters".into())
    }
}

/// Sub-stepping affects only thermal integration, never observation sampling.
const MAX_THERMAL_STEP_S: f64 = 0.25;

impl EngineHeatParamsV2 {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn step(
        &self,
        temperature_c: f64,
        ambient_c: f64,
        capacity_j_per_k: f64,
        fuel: &FuelMassParamsV3,
        shaft_work_j: f64,
        speed_mps: f64,
        duration_s: f64,
    ) -> Result<(f64, ThermalLedgerV2), String> {
        let chemical = (shaft_work_j / 3_600_000.0
            * fuel.brake_specific_fuel_consumption_kg_per_kwh
            + fuel.idle_fuel_flow_kg_per_s * duration_s)
            * self.fuel_lower_heating_value_j_per_kg;
        let waste = chemical - shaft_work_j;
        if !waste.is_finite() || waste < -1e-6 || duration_s <= 0.0 || duration_s > 3600.0 {
            return Err("thermal V2: impossible combustion balance/duration".into());
        }
        let heat = self.coolant_waste_fraction * waste.max(0.0);
        let mut temperature = temperature_c;
        let mut exchange = 0.0;
        let steps = (duration_s / MAX_THERMAL_STEP_S).ceil().max(1.0) as u32;
        let dt = duration_s / f64::from(steps);
        for _ in 0..steps {
            let opening = ((temperature - self.thermostat_start_c)
                / (self.thermostat_full_c - self.thermostat_start_c))
                .clamp(0.0, 1.0);
            let conductance = self.passive_conductance_w_per_k
                + opening
                    * (self.radiator_idle_conductance_w_per_k
                        + self.radiator_speed_conductance_w_per_k_mps * speed_mps);
            let (next, q) = thermal_step(
                temperature,
                ambient_c,
                capacity_j_per_k,
                conductance,
                heat / duration_s,
                dt,
            )?;
            temperature = next;
            exchange += q;
        }
        let storage = capacity_j_per_k * (temperature - temperature_c);
        Ok((
            temperature,
            ThermalLedgerV2 {
                fuel_chemical_energy_j: chemical,
                engine_shaft_work_j: shaft_work_j,
                engine_coolant_heat_j: heat,
                exhaust_and_other_energy_j: waste - heat,
                engine_ambient_exchange_signed_j: exchange,
                engine_storage_change_j: storage,
                maximum_engine_balance_residual_j: (heat - exchange - storage).abs(),
                maximum_fuel_balance_residual_j: (chemical - shaft_work_j - heat - (waste - heat))
                    .abs(),
                ..Default::default()
            },
        ))
    }
}

impl TireHeatParamsV2 {
    /// Two capacities, implicit heat exchange (positive M-matrix), no temperature clipping.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn step(
        &self,
        surface_c: f64,
        carcass_c: f64,
        air_c: f64,
        road_c: f64,
        speed_mps: f64,
        rolling_work_j: f64,
        deformation_work_j: f64,
        duration_s: f64,
        tire: &TireParams,
    ) -> Result<(f64, f64, f64, ThermalLedgerV2), String> {
        if !duration_s.is_finite() || duration_s <= 0.0 || duration_s > 3600.0 {
            return Err("thermal V2: invalid tire integration duration".into());
        }
        let heat_roll = self.rolling_heat_fraction * rolling_work_j;
        let heat_deformation = self.deformation_heat_fraction * deformation_work_j;
        let surface_power = self.deformation_surface_fraction * heat_deformation / duration_s;
        let carcass_power =
            (heat_roll + (1.0 - self.deformation_surface_fraction) * heat_deformation) / duration_s;
        let h_air = self.surface_air_idle_conductance_w_per_k
            + self.surface_air_speed_conductance_w_per_k_mps * speed_mps;
        let g = self.surface_carcass_conductance_w_per_k;
        let h_road = self.surface_road_conductance_w_per_k;
        let h_bulk = self.carcass_air_conductance_w_per_k;
        let cs = self.surface_capacity_j_per_k;
        let cb = self.carcass_capacity_j_per_k;
        let steps = (duration_s / MAX_THERMAL_STEP_S).ceil().max(1.0) as u32;
        let dt = duration_s / f64::from(steps);
        let a = cs / dt + g + h_air + h_road;
        let d = cb / dt + g + h_bulk;
        let determinant = a * d - g * g;
        let (mut ts, mut tb, mut road, mut air) = (surface_c, carcass_c, 0.0, 0.0);
        for _ in 0..steps {
            let b1 = cs / dt * ts + surface_power + h_air * air_c + h_road * road_c;
            let b2 = cb / dt * tb + carcass_power + h_bulk * air_c;
            ts = (d * b1 + g * b2) / determinant;
            tb = (g * b1 + a * b2) / determinant;
            road += h_road * (ts - road_c) * dt;
            air += (h_air * (ts - air_c) + h_bulk * (tb - air_c)) * dt;
        }
        if ![ts, tb, determinant].iter().all(|x| x.is_finite())
            || determinant <= 0.0
            || ts <= -273.15
            || tb <= -273.15
        {
            return Err("thermal V2: invalid tire heat step".into());
        }
        let deviation = (0.5 * (surface_c + ts) - tire.temp_opt) / tire.temp_sigma;
        let wear = (deformation_work_j + self.rolling_wear_fraction * rolling_work_j)
            / self.deformation_energy_to_full_wear_j
            * (1.0 + self.thermal_wear_gain * deviation * deviation);
        if !wear.is_finite() || wear < 0.0 {
            return Err("thermal V2: invalid requested tire wear".into());
        }
        let stored = cs * (ts - surface_c) + cb * (tb - carcass_c);
        Ok((
            ts,
            tb,
            wear,
            ThermalLedgerV2 {
                tire_rolling_dissipation_j: rolling_work_j,
                tire_deformation_dissipation_j: deformation_work_j,
                tire_generated_heat_j: heat_roll + heat_deformation,
                tire_direct_environment_heat_j: rolling_work_j + deformation_work_j
                    - heat_roll
                    - heat_deformation,
                tire_road_exchange_signed_j: road,
                tire_air_exchange_signed_j: air,
                tire_storage_change_j: stored,
                maximum_tire_balance_residual_j: (heat_roll + heat_deformation
                    - road
                    - air
                    - stored)
                    .abs(),
                ..Default::default()
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn engine() -> EngineHeatParamsV2 {
        EngineHeatParamsV2 {
            fuel_lower_heating_value_j_per_kg: 43e6,
            coolant_waste_fraction: 0.4,
            passive_conductance_w_per_k: 0.0,
            radiator_idle_conductance_w_per_k: 0.0,
            radiator_speed_conductance_w_per_k_mps: 0.0,
            thermostat_start_c: 85.0,
            thermostat_full_c: 98.0,
        }
    }
    fn tire() -> TireHeatParamsV2 {
        TireHeatParamsV2 {
            surface_capacity_j_per_k: 10000.0,
            carcass_capacity_j_per_k: 50000.0,
            surface_carcass_conductance_w_per_k: 100.0,
            surface_air_idle_conductance_w_per_k: 20.0,
            surface_air_speed_conductance_w_per_k_mps: 0.5,
            carcass_air_conductance_w_per_k: 10.0,
            surface_road_conductance_w_per_k: 20.0,
            deformation_resistance_coefficient: 0.01,
            rolling_heat_fraction: 0.5,
            deformation_heat_fraction: 0.7,
            deformation_surface_fraction: 0.7,
            deformation_energy_to_full_wear_j: 10e6,
            rolling_wear_fraction: 0.05,
            thermal_wear_gain: 0.6,
        }
    }
    fn compound() -> TireParams {
        TireParams {
            mu_scale: 1.0,
            wear_per_s: 0.0,
            wear_load_k: 0.0,
            wear_grip_k: 0.5,
            wear_min: 0.5,
            wear_grip_curve: None,
            temp_opt: 100.0,
            temp_sigma: 40.0,
            temp_min_k: 0.55,
            heat_k: 0.0,
            cool_k: 0.0,
        }
    }
    fn close(a: f64, b: f64) {
        assert!(
            (a - b).abs() < 1e-6 + 1e-10 * a.abs().max(b.abs()),
            "{a} != {b}"
        );
    }
    #[test]
    fn chemical_balance_drives_engine_storage_and_counts_idle_heat() {
        let p = engine();
        let fuel = FuelMassParamsV3::default();
        let (t, l) = p
            .step(90.0, 35.0, 100000.0, &fuel, 1e6, 50.0, 10.0)
            .unwrap();
        let chemical = (1e6 / 3_600_000.0 * 0.24 + 0.0004 * 10.0) * 43e6;
        close(l.fuel_chemical_energy_j, chemical);
        close(l.engine_coolant_heat_j, (chemical - 1e6) * 0.4);
        close(100000.0 * (t - 90.0), l.engine_coolant_heat_j);
        close(
            chemical,
            l.engine_shaft_work_j + l.engine_coolant_heat_j + l.exhaust_and_other_energy_j,
        );
        let (idle, li) = p.step(90.0, 35.0, 100000.0, &fuel, 0.0, 0.0, 20.0).unwrap();
        assert!(idle > 90.0);
        close(li.engine_coolant_heat_j, 0.0004 * 20.0 * 43e6 * 0.4);
        assert!(l.maximum_engine_balance_residual_j < 1e-6);
    }
    #[test]
    fn engine_fully_open_equilibrium_is_preserved() {
        let mut p = engine();
        p.radiator_idle_conductance_w_per_k = 1000.0;
        let fuel = FuelMassParamsV3 {
            brake_specific_fuel_consumption_kg_per_kwh: 0.24,
            idle_fuel_flow_kg_per_s: 0.0,
        };
        let target = 120.0;
        let ambient = 20.0;
        let shaft = 1000.0 * (target - ambient) / (0.4 * (0.24 * 43e6 / 3_600_000.0 - 1.0));
        let (t, l) = p
            .step(target, ambient, 100000.0, &fuel, shaft * 10.0, 0.0, 10.0)
            .unwrap();
        close(t, target);
        close(l.engine_coolant_heat_j, l.engine_ambient_exchange_signed_j);
    }
    #[test]
    fn thermostat_cold_closed_does_not_expose_full_radiator() {
        let mut p = engine();
        p.radiator_idle_conductance_w_per_k = 10000.0;
        let fuel = FuelMassParamsV3 {
            brake_specific_fuel_consumption_kg_per_kwh: 0.24,
            idle_fuel_flow_kg_per_s: 0.0,
        };
        let (t, l) = p.step(60.0, 20.0, 100000.0, &fuel, 0.0, 0.0, 10.0).unwrap();
        close(t, 60.0);
        close(l.engine_ambient_exchange_signed_j, 0.0);
    }
    #[test]
    fn two_node_equilibrium_and_passive_maximum_principle() {
        let p = tire();
        let c = compound();
        let (ts, tb, w, l) = p
            .step(35.0, 35.0, 35.0, 35.0, 50.0, 0.0, 0.0, 100.0, &c)
            .unwrap();
        close(ts, 35.0);
        close(tb, 35.0);
        close(w, 0.0);
        assert!(l.maximum_tire_balance_residual_j < 1e-6);
        let (ts, tb, _, _) = p
            .step(140.0, 60.0, 20.0, 40.0, 100.0, 0.0, 0.0, 100.0, &c)
            .unwrap();
        assert!((20.0..140.0).contains(&ts));
        assert!((20.0..140.0).contains(&tb));
    }
    #[test]
    fn two_node_heat_is_conservative_and_carcass_has_memory() {
        let mut p = tire();
        p.surface_air_idle_conductance_w_per_k = 0.0;
        p.surface_air_speed_conductance_w_per_k_mps = 0.0;
        p.carcass_air_conductance_w_per_k = 0.0;
        p.surface_road_conductance_w_per_k = 0.0;
        let (ts, tb, _, l) = p
            .step(
                80.0,
                100.0,
                20.0,
                20.0,
                50.0,
                10000.0,
                20000.0,
                10.0,
                &compound(),
            )
            .unwrap();
        close(
            p.surface_capacity_j_per_k * (ts - 80.0) + p.carcass_capacity_j_per_k * (tb - 100.0),
            19000.0,
        );
        close(
            l.tire_generated_heat_j + l.tire_direct_environment_heat_j,
            30000.0,
        );
        assert!(tb > ts);
        assert!(l.maximum_tire_balance_residual_j < 1e-6);
    }
    #[test]
    fn heating_cooling_hot_wear_and_signed_track_exchange_are_distinct() {
        let p = tire();
        let c = compound();
        let (hot, _, wear, l) = p
            .step(80.0, 80.0, 35.0, 42.0, 50.0, 100000.0, 300000.0, 10.0, &c)
            .unwrap();
        assert!(hot > 80.0);
        assert!(wear > 0.0);
        assert!(l.tire_road_exchange_signed_j > 0.0);
        let (cool, _, no_wear, _) = p
            .step(80.0, 80.0, 35.0, 42.0, 50.0, 0.0, 0.0, 10.0, &c)
            .unwrap();
        assert!(cool < 80.0);
        close(no_wear, 0.0);
        let (_, _, hot_wear, _) = p
            .step(180.0, 180.0, 35.0, 42.0, 50.0, 100000.0, 300000.0, 10.0, &c)
            .unwrap();
        assert!(hot_wear > wear);
        let (_, _, _, l) = p
            .step(20.0, 20.0, 35.0, 42.0, 0.0, 0.0, 0.0, 10.0, &c)
            .unwrap();
        assert!(l.tire_road_exchange_signed_j < 0.0 && l.tire_air_exchange_signed_j < 0.0);
    }
    #[test]
    fn implicit_heat_step_converges_under_time_refinement() {
        let p = tire();
        let c = compound();
        // Constant sources. Internal250ms substeps compared with125ms/62.5ms calls.
        let run = |dt: f64| {
            let (mut ts, mut tb) = (80.0, 100.0);
            for _ in 0..(20.0 / dt) as u32 {
                (ts, tb, _, _) = p
                    .step(ts, tb, 35.0, 42.0, 50.0, 1000.0 * dt, 2000.0 * dt, dt, &c)
                    .unwrap();
            }
            (ts, tb)
        };
        let a = run(0.25);
        let b = run(0.125);
        let c = run(0.0625);
        assert!((b.0 - c.0).abs() < (a.0 - b.0).abs());
        assert!((a.0 - c.0).abs() < 0.02);
        assert!((b.1 - c.1).abs() < (a.1 - b.1).abs());
    }
    #[test]
    fn two_node_cooling_matches_independent_analytic_relaxation() {
        let mut p = tire();
        p.surface_air_idle_conductance_w_per_k = 0.0;
        p.surface_air_speed_conductance_w_per_k_mps = 0.0;
        p.carcass_air_conductance_w_per_k = 0.0;
        p.surface_road_conductance_w_per_k = 0.0;
        let (ts, tb, _, _) = p
            .step(80.0, 100.0, 20.0, 20.0, 0.0, 0.0, 0.0, 10.0, &compound())
            .unwrap();
        let total = p.surface_capacity_j_per_k + p.carcass_capacity_j_per_k;
        let mean = (p.surface_capacity_j_per_k * 80.0 + p.carcass_capacity_j_per_k * 100.0) / total;
        let delta = -20.0
            * (-p.surface_carcass_conductance_w_per_k
                * (1.0 / p.surface_capacity_j_per_k + 1.0 / p.carcass_capacity_j_per_k)
                * 10.0)
                .exp();
        assert!((ts - (mean + p.carcass_capacity_j_per_k / total * delta)).abs() < 0.004);
        assert!((tb - (mean - p.surface_capacity_j_per_k / total * delta)).abs() < 0.001);
    }
    #[test]
    fn nonlinear_thermostat_step_converges_when_time_is_refined() {
        let mut p = engine();
        p.radiator_speed_conductance_w_per_k_mps = 30.0;
        let fuel = FuelMassParamsV3::default();
        let run = |dt: f64| {
            let mut t = 80.0;
            for _ in 0..(60.0 / dt) as u32 {
                t = p
                    .step(t, 35.0, 100000.0, &fuel, 100000.0 * dt, 50.0, dt)
                    .unwrap()
                    .0;
            }
            t
        };
        let a = run(0.25);
        let b = run(0.125);
        let c = run(0.0625);
        assert!((b - c).abs() < (a - b).abs());
        assert!((a - c).abs() < 0.03);
    }
}
