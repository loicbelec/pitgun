//! Versioned read-only projection of the existing V3 native lap solution.
//! This does not change dynamics or silently reinterpret historical channels5008/5014.
use serde::{Deserialize, Serialize};

use crate::{
    FuelMassParamsV3, MechanicalParamsV3, SimulationSolution, VehicleParams, power_kw_from_rpm,
    rpm_from_speed_gear,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PowerTelemetryVersion {
    #[serde(rename = "pitgun.racing-power-telemetry/v1")]
    V1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PowerPointV1 {
    pub time_s: f64,
    /// Thermally available crankshaft power, before shift interruption.
    pub engine_available_w: f64,
    /// The load currently used by fuel/heat accounting, not a pedal command.
    pub engine_delivered_w: f64,
    /// Historical positive propulsion budget after transmission efficiency.
    pub drive_budget_w: f64,
    /// F_drive * v at the actual native-node vehicle speed.
    pub wheel_propulsion_w: f64,
    pub driveline_loss_w: f64,
    /// Unresolved low-speed conversion caused by the max(v,10m/s) force rule.
    /// This is not a measured clutch loss or an additional source of heat.
    pub launch_regularization_w: f64,
    pub equivalent_engine_load_pct: f64,
    pub shift_power_fraction: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PowerLedgerV1 {
    /// Integrated with the existing force/thermal clock ds/max(v,1).
    pub solver_engine_work_j: f64,
    pub solver_drive_budget_work_j: f64,
    pub solver_wheel_work_j: f64,
    pub solver_driveline_loss_j: f64,
    pub solver_launch_regularization_j: f64,
    /// Same power trace with the separately reintegrated final trajectory clock.
    pub trajectory_engine_work_j: f64,
    pub trajectory_wheel_work_j: f64,
    pub trajectory_driveline_loss_j: f64,
    pub trajectory_launch_regularization_j: f64,
    pub generated_engine_heat_j: f64,
    /// Signed exchange; negative means ambient heats the engine.
    pub removed_engine_heat_signed_j: f64,
    pub engine_storage_change_j: f64,
    pub engine_thermal_residual_j: f64,
    pub combustion_fuel_consumption_kg: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PowerTelemetryV1 {
    pub schema_version: PowerTelemetryVersion,
    /// No driver pedal is recorded by the present maximum-envelope solver.
    pub pedal_command_pct: Option<Vec<f64>>,
    /// Zero propulsion does not prove that the brakes were commanded.
    pub brake_command_pct: Option<Vec<f64>>,
    pub points: Vec<PowerPointV1>,
    pub ledger: PowerLedgerV1,
}

impl PowerTelemetryV1 {
    /// Sample only this already computed native prefix, using common causal hold.
    /// No extrapolation and no independent reconstruction of pedal commands.
    pub fn sample_at_times(&self, times: &[f64]) -> Result<Vec<PowerPointV1>, String> {
        if self.points.len() < 2
            || times.len() > 1_000_000
            || self.points.iter().any(|p| !p.time_s.is_finite())
            || self.points.windows(2).any(|p| p[1].time_s <= p[0].time_s)
            || times.iter().any(|t| {
                !t.is_finite()
                    || *t < self.points[0].time_s
                    || *t > self.points.last().unwrap().time_s
            })
            || times.windows(2).any(|t| t[1] <= t[0])
        {
            return Err(
                "power samples require bounded increasing times inside the native prefix".into(),
            );
        }
        let mut out = Vec::with_capacity(times.len());
        let mut index = 0;
        for &time in times {
            while index + 1 < self.points.len() && self.points[index + 1].time_s <= time {
                index += 1;
            }
            let mut point = self.points[index].clone();
            point.time_s = time;
            out.push(point);
        }
        Ok(out)
    }

    /// Bounded export only. Common zero-order hold preserves power-port
    /// relationships and does not interpolate percentages across gear changes.
    /// The ledger always remains the native-grid ledger, never this export.
    pub fn resample(&self, hz: f64) -> Result<Vec<PowerPointV1>, String> {
        if !(hz.is_finite() && 0.0 < hz && hz <= 20.0) || self.points.len() < 2 {
            return Err("power export requires at least two nodes and frequency in(0,20]".into());
        }
        if self.points.iter().any(|p| !p.time_s.is_finite())
            || self.points.windows(2).any(|p| p[1].time_s <= p[0].time_s)
        {
            return Err("power export requires finite increasing native times".into());
        }
        let first = self.points[0].time_s;
        let end = self.points.last().unwrap().time_s;
        let count = ((end - first) * hz).ceil();
        if !count.is_finite() || !(1.0..=1_000_000.0).contains(&count) {
            return Err("power export exceeds the bounded sample count".into());
        }
        let mut out = Vec::with_capacity(count as usize);
        let mut index = 0;
        for i in 0..count as usize {
            let time = first + i as f64 / hz;
            if time >= end {
                break;
            }
            while index + 1 < self.points.len() && self.points[index + 1].time_s <= time {
                index += 1;
            }
            let mut point = self.points[index].clone();
            point.time_s = time;
            out.push(point);
        }
        Ok(out)
    }
}

fn finite_nonnegative(label: &str, values: &[f64]) -> Result<(), String> {
    if values.iter().any(|x| !x.is_finite() || *x < 0.0) {
        return Err(format!(
            "power telemetry requires finite nonnegative {label}"
        ));
    }
    Ok(())
}

/// One lap only, before its native nodes are stitched across service boundaries.
/// Mechanical/vehicle inputs must be the already resolved ones used by the Solver.
pub fn project_lap_power_v1(
    solution: &SimulationSolution,
    vehicle: &VehicleParams,
    mechanical: MechanicalParamsV3,
    fuel: Option<FuelMassParamsV3>,
) -> Result<PowerTelemetryV1, String> {
    if solution.longitudinal_model.is_some() || solution.thermal_model.is_some() {
        return Err("power telemetry V1 cannot reinterpret the energy V1 physical model".into());
    }
    let n = solution.t.len();
    if n < 2
        || [
            solution.s.len(),
            solution.v.len(),
            solution.power.len(),
            solution.temp.len(),
            solution.gear.len(),
            solution.engine_derating_factor.len(),
            solution.shift_power_fraction.len(),
        ]
        .iter()
        .any(|len| *len != n)
    {
        return Err("power telemetry requires one complete aligned V3 native lap".into());
    }
    if !solution.lap_index.is_empty()
        && (solution.lap_index.len() != n || solution.lap_index.windows(2).any(|w| w[0] != w[1]))
    {
        return Err("power telemetry requires one native lap, not stitched history".into());
    }
    for (label, values) in [
        ("time", &solution.t),
        ("distance", &solution.s),
        ("speed", &solution.v),
        ("propulsion budget", &solution.power),
        ("temperature", &solution.temp),
    ] {
        finite_nonnegative(label, values)?;
    }
    if solution.t.windows(2).any(|w| w[1] <= w[0])
        || solution.s.windows(2).any(|w| w[1] <= w[0])
        || !mechanical.driveline_efficiency.is_finite()
        || !(0.0 < mechanical.driveline_efficiency && mechanical.driveline_efficiency <= 1.0)
    {
        return Err("power telemetry requires increasing grids and efficiency in(0,1]".into());
    }
    crate::validate_resolved_vehicle(vehicle)?;
    if let Some(fuel) = fuel {
        fuel.validate()?;
    }
    let mut points = Vec::with_capacity(n);
    for i in 0..n {
        let gear = solution.gear[i];
        let ratio = *vehicle
            .engine
            .gear_ratios
            .get(usize::from(gear).wrapping_sub(1))
            .ok_or("power telemetry has an invalid gear")?;
        let derating = solution.engine_derating_factor[i];
        let shift = solution.shift_power_fraction[i];
        if !derating.is_finite()
            || !(0.0..=1.0).contains(&derating)
            || !shift.is_finite()
            || !(0.0..=1.0).contains(&shift)
        {
            return Err("power telemetry has an invalid derating or shift fraction".into());
        }
        let v = solution.v[i].max(1.0);
        let rpm = rpm_from_speed_gear(v, ratio, &vehicle.chassis).max(vehicle.engine.n_idle);
        let available = power_kw_from_rpm(rpm, &vehicle.engine) * 1000.0 * derating;
        let budget = solution.power[i] * 1000.0;
        let delivered = budget / mechanical.driveline_efficiency;
        let capacity = available * shift;
        // Permit floating-point roundoff only. Never hide a material capacity
        // mismatch by clamping the proposed load to100%.
        if !available.is_finite()
            || available < 0.0
            || delivered > capacity + 1e-6 + capacity.abs() * 1e-10
        {
            return Err(format!(
                "power telemetry capacity mismatch at native node{i}: load{delivered}, available after shift{capacity}"
            ));
        }
        let equivalent_load = if available > 0.0 {
            (delivered / available).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let wheel = budget * solution.v[i] / v.max(10.0);
        points.push(PowerPointV1 {
            time_s: solution.t[i],
            engine_available_w: available,
            engine_delivered_w: delivered,
            drive_budget_w: budget,
            wheel_propulsion_w: wheel,
            driveline_loss_w: delivered - budget,
            launch_regularization_w: budget - wheel,
            equivalent_engine_load_pct: equivalent_load * 100.0,
            shift_power_fraction: shift,
        });
    }
    let mut ledger = PowerLedgerV1::default();
    for (i, p) in points.iter().enumerate().take(n - 1) {
        let v = solution.v[i].max(1.0);
        let dt_solver = (solution.s[i + 1] - solution.s[i]) / v;
        let dt_trajectory = solution.t[i + 1] - solution.t[i];
        ledger.solver_engine_work_j += p.engine_delivered_w * dt_solver;
        ledger.solver_drive_budget_work_j += p.drive_budget_w * dt_solver;
        ledger.solver_wheel_work_j += p.wheel_propulsion_w * dt_solver;
        ledger.solver_driveline_loss_j += p.driveline_loss_w * dt_solver;
        ledger.solver_launch_regularization_j += p.launch_regularization_w * dt_solver;
        ledger.trajectory_engine_work_j += p.engine_delivered_w * dt_trajectory;
        ledger.trajectory_wheel_work_j += p.wheel_propulsion_w * dt_trajectory;
        ledger.trajectory_driveline_loss_j += p.driveline_loss_w * dt_trajectory;
        ledger.trajectory_launch_regularization_j += p.launch_regularization_w * dt_trajectory;
        ledger.generated_engine_heat_j +=
            vehicle.engine.alpha_heat * p.engine_delivered_w * dt_solver;
        ledger.removed_engine_heat_signed_j += (vehicle.engine.p_cool0 + vehicle.engine.k_cool * v)
            * (solution.temp[i] - vehicle.engine.t_amb)
            * dt_solver;
    }
    ledger.engine_storage_change_j =
        vehicle.engine.c_th * (solution.temp[n - 1] - solution.temp[0]);
    ledger.engine_thermal_residual_j = ledger.engine_storage_change_j
        - ledger.generated_engine_heat_j
        + ledger.removed_engine_heat_signed_j;
    ledger.combustion_fuel_consumption_kg = fuel.map(|f| {
        ledger.solver_engine_work_j / 3_600_000.0 * f.brake_specific_fuel_consumption_kg_per_kwh
            + f.idle_fuel_flow_kg_per_s * (solution.t[n - 1] - solution.t[0])
    });
    Ok(PowerTelemetryV1 {
        schema_version: PowerTelemetryVersion::V1,
        pedal_command_pct: None,
        brake_command_pct: None,
        points,
        ledger,
    })
}
