//! Experimental spatial work/energy integrator. No historical model selects it.
//! One segment uses midpoint road loads, a held input torque, an ideal slipping
//! clutch below idle lock speed, and the same elapsed time for every subsystem.
use crate::{
    MechanicalParamsV3, TireContactParamsV3, VehicleParams, aggregate_tire_force_capacity_v3,
    remaining_longitudinal_force, rpm_from_speed_gear,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum LongitudinalEnergyModelV1 {
    #[serde(rename = "pitgun.racing-longitudinal-energy/v1")]
    V1,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct EnergyLedgerV1 {
    pub initial_vehicle_kinetic_j: f64,
    pub final_vehicle_kinetic_j: f64,
    pub fuel_mass_carried_kinetic_j: f64,
    pub unresolved_service_velocity_reset_j: f64,
    pub maximum_raw_contact_utilization: f64,
    pub service_time_s: f64,
    pub service_idle_fuel_kg: f64,
    pub driving_time_s: f64,
    pub distance_m: f64,
    pub kinetic_change_j: f64,
    pub potential_change_j: f64,
    pub drag_work_j: f64,
    pub rolling_work_j: f64,
    pub wheel_propulsion_work_j: f64,
    pub friction_brake_work_j: f64,
    pub engine_shaft_work_j: f64,
    pub driveline_loss_j: f64,
    pub clutch_slip_loss_j: f64,
    pub generated_engine_heat_j: f64,
    pub rejected_engine_heat_signed_j: f64,
    pub maximum_vehicle_balance_residual_j: f64,
    pub maximum_driveline_balance_residual_j: f64,
    pub maximum_thermal_balance_residual_j: f64,
    pub segments: u64,
}
impl EnergyLedgerV1 {
    pub fn accumulate(&mut self, other: &Self) {
        if self.segments == 0 {
            self.initial_vehicle_kinetic_j = other.initial_vehicle_kinetic_j;
        }
        self.final_vehicle_kinetic_j = other.final_vehicle_kinetic_j;
        self.fuel_mass_carried_kinetic_j += other.fuel_mass_carried_kinetic_j;
        self.unresolved_service_velocity_reset_j += other.unresolved_service_velocity_reset_j;
        self.maximum_raw_contact_utilization = self
            .maximum_raw_contact_utilization
            .max(other.maximum_raw_contact_utilization);
        self.service_time_s += other.service_time_s;
        self.service_idle_fuel_kg += other.service_idle_fuel_kg;
        self.driving_time_s += other.driving_time_s;
        self.distance_m += other.distance_m;
        self.kinetic_change_j += other.kinetic_change_j;
        self.potential_change_j += other.potential_change_j;
        self.drag_work_j += other.drag_work_j;
        self.rolling_work_j += other.rolling_work_j;
        self.wheel_propulsion_work_j += other.wheel_propulsion_work_j;
        self.friction_brake_work_j += other.friction_brake_work_j;
        self.engine_shaft_work_j += other.engine_shaft_work_j;
        self.driveline_loss_j += other.driveline_loss_j;
        self.clutch_slip_loss_j += other.clutch_slip_loss_j;
        self.generated_engine_heat_j += other.generated_engine_heat_j;
        self.rejected_engine_heat_signed_j += other.rejected_engine_heat_signed_j;
        self.maximum_vehicle_balance_residual_j = self
            .maximum_vehicle_balance_residual_j
            .max(other.maximum_vehicle_balance_residual_j);
        self.maximum_driveline_balance_residual_j = self
            .maximum_driveline_balance_residual_j
            .max(other.maximum_driveline_balance_residual_j);
        self.maximum_thermal_balance_residual_j = self
            .maximum_thermal_balance_residual_j
            .max(other.maximum_thermal_balance_residual_j);
        self.segments += other.segments;
    }
    pub fn boundary_balance_residual_j(&self) -> f64 {
        self.kinetic_change_j
            - self.fuel_mass_carried_kinetic_j
            - self.unresolved_service_velocity_reset_j
            - (self.final_vehicle_kinetic_j - self.initial_vehicle_kinetic_j)
    }
    pub fn vehicle_balance_residual_j(&self) -> f64 {
        self.wheel_propulsion_work_j
            - self.friction_brake_work_j
            - self.drag_work_j
            - self.rolling_work_j
            - self.potential_change_j
            - self.kinetic_change_j
    }
}

pub(crate) struct SegmentContext<'a> {
    pub vehicle: &'a VehicleParams,
    pub mechanical: &'a MechanicalParamsV3,
    pub contact: &'a TireContactParamsV3,
    // Prepared once for the unchanged engine, using the exact interpolator even
    // at knots (direct map ordinates can differ in their final floating bits).
    pub torque_knots_nm: Option<&'a [f64]>,
    pub thermal: Option<(
        &'a crate::thermal_physics::EngineHeatParamsV2,
        &'a crate::FuelMassParamsV3,
    )>,
    pub tire_deformation_coefficient: f64,
    pub mass_kg: f64,
    pub ds_m: f64,
    pub dz_m: f64,
    pub curvature_radpm: f64,
    pub vertical_curvature_per_m: f64,
    pub mu: f64,
    pub traction_utilization: f64,
    pub braking_utilization: f64,
}
#[derive(Debug, PartialEq)]
pub(crate) struct SegmentAdvance {
    pub end_speed_mps: f64,
    pub duration_s: f64,
    pub engine_budget_kw: f64,
    pub engine_shaft_power_w: f64,
    pub end_temperature_c: f64,
    pub delivered_shift_fraction: f64,
    pub shift_time_s: f64,
    pub normal_load_n: f64,
    pub available_force_n: f64,
    pub brake_force_n: f64,
    pub contact_longitudinal_force_n: f64,
    pub ledger: EnergyLedgerV1,
    pub thermal_ledger: Option<crate::thermal_physics::ThermalLedgerV2>,
}

impl SegmentContext<'_> {
    // Midpoint quadrature of forces; potential uses the exact endpoint height.
    fn road_load(&self, v0: f64, v1: f64) -> (f64, f64, f64, f64) {
        let v = 0.5 * (v0 + v1);
        let c = &self.vehicle.chassis;
        let drag = 0.5 * c.rho * self.mechanical.fixed_drag_area_m2 * v * v;
        let downforce = 0.5 * c.rho * self.mechanical.fixed_downforce_area_m2 * v * v;
        let normal =
            (self.mass_kg * (c.g + v * v * self.vertical_curvature_per_m) + downforce).max(0.0);
        let grade = self.mass_kg * c.g * self.dz_m / self.ds_m;
        let mut rolling = c.c_rr * normal;
        if self.tire_deformation_coefficient > 0.0 {
            let available = aggregate_tire_force_capacity_v3(self.mu, normal, self.contact)
                * self.mechanical.chassis_force_transfer_efficiency;
            let base_longitudinal =
                self.mass_kg * (v1 * v1 - v0 * v0) / (2.0 * self.ds_m) + drag + rolling + grade;
            let lateral = self.mass_kg * v * v * self.curvature_radpm;
            let utilization = if available > 0.0 {
                base_longitudinal.hypot(lateral) / available
            } else {
                0.0
            };
            // An explicitly reduced deformation loss. It is included in the
            // propulsion/braking solve and energy ledger, not invented as heat.
            rolling +=
                self.tire_deformation_coefficient * normal * utilization.clamp(0.0, 1.0).powi(2);
        }
        (drag, rolling, grade, normal)
    }
    fn limits(&self, v0: f64, v1: f64) -> (f64, f64, f64, f64) {
        self.limits_with_load(v0, v1, self.road_load(v0, v1))
    }
    fn limits_with_load(
        &self,
        v0: f64,
        v1: f64,
        load: (f64, f64, f64, f64),
    ) -> (f64, f64, f64, f64) {
        let (drag, rolling, grade, normal) = load;
        let v = 0.5 * (v0 + v1);
        let available = aggregate_tire_force_capacity_v3(self.mu, normal, self.contact)
            * self.mechanical.chassis_force_transfer_efficiency;
        let lateral = self.mass_kg * v * v * self.curvature_radpm.abs();
        let longitudinal = remaining_longitudinal_force(available, lateral);
        (
            drag + rolling + grade,
            longitudinal * self.traction_utilization,
            (longitudinal * self.braking_utilization).min(self.mechanical.maximum_brake_force_n),
            available,
        )
    }
    pub(crate) fn braking_start_limit(&self, end: f64, upper: f64) -> Result<f64, String> {
        let residual = |start: f64| {
            let (resistance, _, brake, _) = self.limits(start, end);
            -(self.mass_kg * (end * end - start * start) / (2.0 * self.ds_m) + resistance) - brake
        };
        let value = residual(upper);
        if !value.is_finite() {
            return Err("energy V1: nonfinite braking envelope".into());
        }
        if value <= 0.0 {
            return Ok(upper);
        }
        feasible_root(0.0, upper, residual(0.0), value, residual)
    }
    pub(crate) fn advance(
        &self,
        start: f64,
        target: f64,
        gear: u8,
        derating: f64,
        shift_remaining_s: f64,
        temperature_c: f64,
    ) -> Result<SegmentAdvance, String> {
        let ratio = self.vehicle.engine.gear_ratios[usize::from(gear) - 1];
        let redline =
            self.vehicle.engine.n_max * std::f64::consts::TAU * self.vehicle.chassis.r_wheel
                / (60.0 * ratio);
        let target = target.min(redline);
        let duration = |end: f64| {
            if start + end > 0.0 {
                2.0 * self.ds_m / (start + end)
            } else {
                f64::INFINITY
            }
        };
        let e = &self.vehicle.engine;
        let rpm = |v: f64| rpm_from_speed_gear(v, ratio, &self.vehicle.chassis).max(e.n_idle);
        let torque = |r: f64| {
            if r < e.n_rpm[0] || r > *e.n_rpm.last().unwrap() {
                0.0
            } else {
                crate::interp_linear(r, &e.n_rpm, &e.trq) * 1000.0
            }
        };
        let initial_rpm = rpm(start);
        let initial_torque = torque(initial_rpm);
        let endpoint_torque = |r: f64| {
            if r == initial_rpm {
                initial_torque
            } else {
                torque(r)
            }
        };
        let shaft_torque_limit = |end: f64| {
            // Same held minimum across RPM endpoints and interior map knots.
            let low = rpm(start.min(end));
            let high = rpm(start.max(end));
            let mut minimum = endpoint_torque(low).min(endpoint_torque(high));
            for (index, &r) in e.n_rpm.iter().enumerate() {
                if r > low && r < high {
                    let at_knot = self
                        .torque_knots_nm
                        .map_or_else(|| torque(r), |values| values[index]);
                    minimum = minimum.min(at_knot);
                }
            }
            let shift_fraction = (shift_remaining_s / duration(end)).clamp(0.0, 1.0);
            minimum
                * derating
                * (1.0 - shift_fraction * (1.0 - self.mechanical.shift_power_fraction))
        };
        let drive_residual = |end: f64| {
            let (resistance, traction, _, _) = self.limits(start, end);
            let torque_force =
                shaft_torque_limit(end) * ratio * self.mechanical.driveline_efficiency
                    / self.vehicle.chassis.r_wheel;
            self.mass_kg * (end * end - start * start) / (2.0 * self.ds_m) + resistance
                - torque_force.min(traction)
        };
        let target_residual = drive_residual(target);
        let end = if target_residual <= 0.0 {
            target
        } else {
            let zero_residual = drive_residual(0.0);
            if zero_residual > 0.0 {
                return Err(
                    "energy V1: no feasible motion under the resolved torque and road load".into(),
                );
            }
            feasible_root(0.0, target, zero_residual, target_residual, drive_residual)?
        };
        let dt = duration(end);
        if !dt.is_finite() || dt <= 0.0 {
            return Err("energy V1: vehicle stalled; cannot traverse this segment".into());
        }
        let load = self.road_load(start, end);
        let (resistance, _, brake_limit, available) = self.limits_with_load(start, end, load);
        let signed_force =
            self.mass_kg * (end * end - start * start) / (2.0 * self.ds_m) + resistance;
        let drive = signed_force.max(0.0);
        let brake = (-signed_force).max(0.0);
        if brake > brake_limit + 1e-2 {
            return Err(format!(
                "energy V1: braking envelope inconsistent by {:.6} N",
                brake - brake_limit
            ));
        }
        let (drag, rolling, grade, normal) = load;
        let wheel = drive * self.ds_m;
        // Clutch upstream of the gearbox: input speed cannot drop below idle.
        let lock_speed =
            self.vehicle.engine.n_idle * std::f64::consts::TAU * self.vehicle.chassis.r_wheel
                / (60.0 * ratio);
        let a = (lock_speed - start).max(0.0);
        let b = (lock_speed - end).max(0.0);
        let slip_distance = if a > 0.0 && b > 0.0 {
            0.5 * (a + b) * dt
        } else if a + b > 0.0 {
            0.5 * (a + b).powi(2) / (end - start).abs() * dt
        } else {
            0.0
        };
        let eta = self.mechanical.driveline_efficiency;
        let clutch = drive * slip_distance / eta;
        let loss = wheel * (1.0 / eta - 1.0);
        let shaft = wheel + loss + clutch;
        let (next_temp, heat, rejected, thermal_ledger) =
            if let Some((thermal, fuel)) = self.thermal {
                let (temperature, ledger) = thermal.step(
                    temperature_c,
                    self.vehicle.engine.t_amb,
                    self.vehicle.engine.c_th,
                    fuel,
                    shaft,
                    0.5 * (start + end),
                    dt,
                )?;
                (
                    temperature,
                    ledger.engine_coolant_heat_j,
                    ledger.engine_ambient_exchange_signed_j,
                    Some(ledger),
                )
            } else {
                let heat = self.vehicle.engine.alpha_heat * shaft;
                let conductance =
                    self.vehicle.engine.p_cool0 + self.vehicle.engine.k_cool * 0.5 * (start + end);
                let (temperature, rejected) = thermal_step(
                    temperature_c,
                    self.vehicle.engine.t_amb,
                    self.vehicle.engine.c_th,
                    conductance,
                    heat / dt,
                    dt,
                )?;
                (temperature, heat, rejected, None)
            };
        let kinetic = 0.5 * self.mass_kg * (end * end - start * start);
        let vehicle_residual = wheel
            - brake * self.ds_m
            - drag * self.ds_m
            - rolling * self.ds_m
            - grade * self.ds_m
            - kinetic;
        let thermal_residual =
            self.vehicle.engine.c_th * (next_temp - temperature_c) - (heat - rejected);
        let mid_speed = 0.5 * (start + end);
        let lateral = self.mass_kg * mid_speed * mid_speed * self.curvature_radpm;
        let demand = (drive - brake).hypot(lateral);
        let raw_contact = if available > 0.0 {
            demand / available
        } else if demand == 0.0 {
            0.0
        } else {
            f64::INFINITY
        };
        if ![
            end,
            dt,
            drive,
            brake,
            normal,
            available,
            shaft,
            wheel,
            kinetic,
            heat,
            rejected,
            vehicle_residual,
            raw_contact,
        ]
        .iter()
        .all(|v| v.is_finite())
        {
            return Err("energy V1: nonfinite physical segment".into());
        }
        if raw_contact > 1.0 + 1e-6 {
            return Err(format!(
                "energy V1: prescribed trajectory exceeds raw contact capacity: {raw_contact}"
            ));
        }
        let ledger = EnergyLedgerV1 {
            initial_vehicle_kinetic_j: 0.5 * self.mass_kg * start * start,
            final_vehicle_kinetic_j: 0.5 * self.mass_kg * end * end,
            fuel_mass_carried_kinetic_j: 0.0,
            unresolved_service_velocity_reset_j: 0.0,
            maximum_raw_contact_utilization: raw_contact,
            service_time_s: 0.0,
            service_idle_fuel_kg: 0.0,
            driving_time_s: dt,
            distance_m: self.ds_m,
            kinetic_change_j: kinetic,
            potential_change_j: grade * self.ds_m,
            drag_work_j: drag * self.ds_m,
            rolling_work_j: rolling * self.ds_m,
            wheel_propulsion_work_j: wheel,
            friction_brake_work_j: brake * self.ds_m,
            engine_shaft_work_j: shaft,
            driveline_loss_j: loss,
            clutch_slip_loss_j: clutch,
            generated_engine_heat_j: heat,
            rejected_engine_heat_signed_j: rejected,
            maximum_vehicle_balance_residual_j: vehicle_residual.abs(),
            maximum_driveline_balance_residual_j: (shaft - wheel - loss - clutch).abs(),
            maximum_thermal_balance_residual_j: thermal_residual.abs(),
            segments: 1,
        };
        let shift_time = shift_remaining_s.min(dt);
        Ok(SegmentAdvance {
            thermal_ledger,
            end_speed_mps: end,
            duration_s: dt,
            engine_budget_kw: shaft * eta / dt / 1000.0,
            engine_shaft_power_w: shaft / dt,
            end_temperature_c: next_temp,
            delivered_shift_fraction: 1.0
                - shift_time / dt * (1.0 - self.mechanical.shift_power_fraction),
            shift_time_s: shift_time,
            normal_load_n: normal,
            available_force_n: available,
            brake_force_n: brake,
            contact_longitudinal_force_n: drive - brake,
            ledger,
        })
    }
}

/// Safeguarded Illinois regula-falsi. Retain a feasible endpoint, never a
/// positive-residual trial. Force tolerance is1e-6 N; speed tolerance is1e-9
/// relative. The48-step bound protects pathological/coarse inputs.
fn feasible_root(
    mut low: f64,
    mut high: f64,
    mut left: f64,
    mut right: f64,
    mut residual: impl FnMut(f64) -> f64,
) -> Result<f64, String> {
    let mut actual_left = left;
    if !left.is_finite() || !right.is_finite() || left > 0.0 || right <= 0.0 {
        return Err("energy V1: no finite bracket for the segment constraint".into());
    }
    let mut last_side = 0_i8;
    for _ in 0..48 {
        if actual_left.abs() <= 1e-6 || high - low <= 1e-9 * (1.0 + low.abs()) {
            return Ok(low);
        }
        let fraction = -left / (right - left);
        let trial = if fraction > 0.0 && fraction < 1.0 {
            low + (high - low) * fraction
        } else {
            0.5 * (low + high)
        };
        if trial <= low || trial >= high {
            return Ok(low);
        }
        let value = residual(trial);
        if !value.is_finite() {
            return Err("energy V1: nonfinite segment constraint".into());
        }
        if value <= 0.0 {
            low = trial;
            left = value;
            actual_left = value;
            if last_side == -1 {
                right *= 0.5;
            }
            last_side = -1;
        } else {
            high = trial;
            right = value;
            if last_side == 1 {
                left *= 0.5;
            }
            last_side = 1;
        }
    }
    Err("energy V1: segment constraint did not converge within48 trials".into())
}

/// Exact constant-input lumped thermal step C dT/dt = Q - H(T-Tamb).
/// Exp-m1 avoids cancellation for short steps; outward heat is signed.
pub fn thermal_step(
    initial_c: f64,
    ambient_c: f64,
    capacity_j_per_k: f64,
    conductance_w_per_k: f64,
    heat_w: f64,
    duration_s: f64,
) -> Result<(f64, f64), String> {
    if ![
        initial_c,
        ambient_c,
        capacity_j_per_k,
        conductance_w_per_k,
        heat_w,
        duration_s,
    ]
    .iter()
    .all(|v| v.is_finite())
        || capacity_j_per_k <= 0.0
        || conductance_w_per_k < 0.0
        || heat_w < 0.0
        || duration_s <= 0.0
    {
        return Err("energy V1: invalid thermal step".into());
    }
    let energy = heat_w * duration_s;
    let z = conductance_w_per_k * duration_s / capacity_j_per_k;
    let (decay, phi) = if z == 0.0 {
        (0.0, 1.0)
    } else {
        let decay = -(-z).exp_m1();
        (decay, decay / z)
    };
    let delta = (ambient_c - initial_c) * decay + energy / capacity_j_per_k * phi;
    let final_c = initial_c + delta;
    let rejected_j = if conductance_w_per_k == 0.0 {
        0.0
    } else {
        energy - capacity_j_per_k * delta
    };
    if !final_c.is_finite() || !rejected_j.is_finite() {
        return Err("energy V1: thermal step overflow".into());
    }
    Ok((final_c, rejected_j))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AeroParams, ChassisParams, EngineParams, TireParams};
    fn vehicle() -> VehicleParams {
        VehicleParams {
            chassis: ChassisParams {
                mass_empty: 1000.,
                r_wheel: 0.5,
                mu0: 2.,
                c_rr: 0.,
                rho: 1.,
                g: 9.81,
            },
            aero: AeroParams {
                cd_a_x: 0.,
                cd_a_z: 0.,
                cl_a_x: 0.,
                cl_a_z: 0.,
            },
            engine: EngineParams {
                n_rpm: vec![1000., 10000.],
                trq: vec![0.1, 0.1],
                gear_ratios: vec![10.],
                n_upshift: 9000.,
                n_downshift: 2000.,
                n_idle: 1000.,
                n_max: 10000.,
                t_amb: 20.,
                t_init: 90.,
                c_th: 10000.,
                alpha_heat: 0.3,
                p_cool0: 100.,
                k_cool: 0.,
                t_soft: 120.,
                beta_derate: 0.01,
                fuel_burn_kg_per_s: 0.01,
            },
            tire: TireParams {
                mu_scale: 1.,
                wear_per_s: 0.,
                wear_load_k: 0.,
                wear_grip_k: 0.,
                wear_min: 1.,
                temp_opt: 90.,
                temp_sigma: 20.,
                temp_min_k: 1.,
                heat_k: 0.,
                cool_k: 0.,
            },
        }
    }
    fn mechanical() -> MechanicalParamsV3 {
        MechanicalParamsV3 {
            driveline_efficiency: 0.9,
            fixed_drag_area_m2: 0.,
            fixed_downforce_area_m2: 0.,
            ..MechanicalParamsV3::default()
        }
    }
    fn context<'a>(
        v: &'a VehicleParams,
        m: &'a MechanicalParamsV3,
        c: &'a TireContactParamsV3,
    ) -> SegmentContext<'a> {
        SegmentContext {
            vehicle: v,
            mechanical: m,
            contact: c,
            torque_knots_nm: None,
            thermal: None,
            tire_deformation_coefficient: 0.0,
            mass_kg: 1000.,
            ds_m: 100.,
            dz_m: 0.,
            curvature_radpm: 0.,
            vertical_curvature_per_m: 0.,
            mu: 2.,
            traction_utilization: 1.,
            braking_utilization: 1.,
        }
    }
    fn close(a: f64, b: f64) {
        assert!(
            (a - b).abs() < 1e-6 + 1e-7 * a.abs().max(b.abs()),
            "{a} != {b}"
        );
    }
    fn balances(r: &SegmentAdvance) {
        assert!(r.ledger.maximum_vehicle_balance_residual_j < 1e-6);
        assert!(r.ledger.maximum_driveline_balance_residual_j < 1e-6);
        assert!(r.ledger.maximum_thermal_balance_residual_j < 1e-6);
        close(r.ledger.driving_time_s, r.duration_s);
    }
    #[test]
    fn prepared_torque_knots_preserve_complete_segment_results() {
        let mut v = vehicle();
        v.engine.n_rpm = vec![1000., 2500., 4500., 7000., 10000.];
        v.engine.trq = vec![0.101, 0.153, 0.087, 0.135, 0.073];
        let knots = v
            .engine
            .n_rpm
            .iter()
            .map(|&r| crate::interp_linear(r, &v.engine.n_rpm, &v.engine.trq) * 1000.)
            .collect::<Vec<_>>();
        let m = mechanical();
        let c = TireContactParamsV3::default();
        for start in [0., 5., 15., 30.] {
            for target in [10., 20., 45.] {
                let plain = context(&v, &m, &c);
                let cached = SegmentContext {
                    torque_knots_nm: Some(&knots),
                    ..context(&v, &m, &c)
                };
                assert_eq!(
                    plain.advance(start, target, 1, 0.7, 0.05, 95.),
                    cached.advance(start, target, 1, 0.7, 0.05, 95.)
                );
            }
        }
    }

    #[test]
    fn energy_v1_launch_uses_finite_wheel_torque_and_records_physical_clutch_slip() {
        let v = vehicle();
        let m = mechanical();
        let c = TireContactParamsV3::default();
        let mut ctx = context(&v, &m, &c);
        ctx.ds_m = 10.;
        let r = ctx.advance(0., 50., 1, 1., 0., 90.).unwrap();
        close(r.end_speed_mps, 6.);
        close(r.duration_s, 10. / 3.);
        close(r.ledger.kinetic_change_j, 18000.);
        assert!(r.ledger.clutch_slip_loss_j > 0.);
        close(
            r.ledger.engine_shaft_work_j,
            r.ledger.wheel_propulsion_work_j
                + r.ledger.driveline_loss_j
                + r.ledger.clutch_slip_loss_j,
        );
        balances(&r);
    }
    #[test]
    fn energy_v1_cruising_pays_drag_instead_of_switching_engine_work_off() {
        let v = vehicle();
        let mut m = mechanical();
        m.fixed_drag_area_m2 = 1.;
        let c = TireContactParamsV3::default();
        let ctx = context(&v, &m, &c);
        let r = ctx.advance(20., 20., 1, 1., 0., 90.).unwrap();
        close(r.end_speed_mps, 20.);
        close(r.duration_s, 5.);
        close(r.ledger.wheel_propulsion_work_j, 20000.);
        close(r.ledger.engine_shaft_work_j, 20000. / 0.9);
        close(r.ledger.clutch_slip_loss_j, 0.);
        close(r.brake_force_n, 0.);
        balances(&r);
    }
    #[test]
    fn energy_v1_no_load_coast_is_not_a_brake_command_or_free_generated_energy() {
        let v = vehicle();
        let m = mechanical();
        let c = TireContactParamsV3::default();
        let ctx = context(&v, &m, &c);
        let r = ctx.advance(20., 20., 1, 1., 0., 90.).unwrap();
        close(r.ledger.engine_shaft_work_j, 0.);
        close(r.brake_force_n, 0.);
        balances(&r);
    }
    #[test]
    fn energy_v1_braking_and_grade_have_signed_work_with_no_positive_propulsion() {
        let v = vehicle();
        let m = mechanical();
        let c = TireContactParamsV3::default();
        let mut ctx = context(&v, &m, &c);
        let r = ctx.advance(20., 10., 1, 1., 0., 90.).unwrap();
        close(r.ledger.friction_brake_work_j, 150000.);
        close(r.ledger.engine_shaft_work_j, 0.);
        balances(&r);
        ctx.dz_m = -10.;
        let r = ctx.advance(20., 20., 1, 1., 0., 90.).unwrap();
        close(r.ledger.potential_change_j, -98100.);
        close(r.ledger.friction_brake_work_j, 98100.);
        balances(&r);
    }
    #[test]
    fn energy_v1_shift_duration_uses_the_actual_motion_clock() {
        let v = vehicle();
        let m = mechanical();
        let c = TireContactParamsV3::default();
        let ctx = context(&v, &m, &c);
        let r = ctx.advance(20., 30., 1, 1., 0.05, 90.).unwrap();
        close(r.shift_time_s, 0.05);
        close(r.delivered_shift_fraction, 1. - 0.05 / r.duration_s);
        balances(&r);
    }
    #[test]
    fn energy_v1_braking_search_and_forward_step_use_the_same_force_budget() {
        let v = vehicle();
        let mut m = mechanical();
        m.maximum_brake_force_n = 2000.;
        let c = TireContactParamsV3::default();
        let ctx = context(&v, &m, &c);
        let upper = ctx.braking_start_limit(10., 40.).unwrap();
        assert!(upper > 10. && upper < 40.);
        let r = ctx.advance(upper, 10., 1, 1., 0., 90.).unwrap();
        close(r.end_speed_mps, 10.);
        balances(&r);
    }
    #[test]
    fn energy_v1_an_incapable_vehicle_stalls_without_inventing_one_metre_per_second() {
        let mut v = vehicle();
        v.engine.trq = vec![0., 0.];
        let m = mechanical();
        let c = TireContactParamsV3::default();
        let ctx = context(&v, &m, &c);
        assert!(ctx.advance(0., 20., 1, 1., 0., 90.).is_err());
    }
    #[test]
    fn energy_v1_rejects_an_infeasible_lateral_path_without_clipping_utilization() {
        let v = vehicle();
        let m = mechanical();
        let c = TireContactParamsV3::default();
        let mut ctx = context(&v, &m, &c);
        ctx.mu = 0.1;
        ctx.curvature_radpm = 0.02;
        assert!(
            ctx.advance(20., 20., 1, 1., 0., 90.)
                .err()
                .unwrap()
                .contains("contact capacity")
        );
    }
    #[test]
    fn energy_v1_tiny_conductance_converges_to_the_adiabatic_limit() {
        let (t, q) = thermal_step(80., 20., 1000., 1e-300, 1000., 10.).unwrap();
        close(t, 90.);
        close(q, 0.);
    }
    #[test]
    fn energy_v1_thermal_step_is_stable_exact_and_composes_for_constant_inputs() {
        let (t, q) = thermal_step(100., 20., 1000., 1000., 0., 10.).unwrap();
        close(t, 20. + 80. * (-10_f64).exp());
        close(q, 1000. * (100. - t));
        assert!((20.0..=100.).contains(&t));
        let (once, _) = thermal_step(80., 20., 1000., 100., 1000., 10.).unwrap();
        let (half, _) = thermal_step(80., 20., 1000., 100., 1000., 5.).unwrap();
        let (twice, _) = thermal_step(half, 20., 1000., 100., 1000., 5.).unwrap();
        close(once, twice);
        let (adiabatic, rejected) = thermal_step(80., 20., 1000., 0., 1000., 10.).unwrap();
        close(adiabatic, 90.);
        close(rejected, 0.);
        let (heated, signed) = thermal_step(0., 20., 1000., 100., 0., 1.).unwrap();
        assert!(heated > 0. && signed < 0.);
        assert!(thermal_step(80., 20., 0., 100., 1000., 10.).is_err());
    }
}
