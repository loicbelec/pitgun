use pitgun_contract::game::v1::GameTelemetryPointV1;
use pitgun_core::Event;

pub fn telemetry_point_to_events(point: &GameTelemetryPointV1, ts_ns: u64) -> Vec<Event> {
    let mut events = Vec::with_capacity(14);
    push_event(&mut events, "sim.time_s", ts_ns, point.time_s as f64);
    push_event(&mut events, "sim.s_m", ts_ns, point.s_m as f64);
    push_event(&mut events, "sim.x_m", ts_ns, point.x_m as f64);
    push_event(&mut events, "sim.y_m", ts_ns, point.y_m as f64);
    push_event(
        &mut events,
        "sim.heading_rad",
        ts_ns,
        point.heading_rad as f64,
    );
    push_event(&mut events, "sim.speed_kph", ts_ns, point.speed_kph as f64);
    push_event(&mut events, "sim.rpm", ts_ns, point.rpm as f64);
    push_event(&mut events, "sim.gear", ts_ns, point.gear as f64);
    push_event(
        &mut events,
        "sim.throttle_pct",
        ts_ns,
        point.throttle_pct as f64,
    );
    push_event(&mut events, "sim.brake_pct", ts_ns, point.brake_pct as f64);
    push_event(&mut events, "sim.g_lat", ts_ns, point.g_lat as f64);
    push_event(&mut events, "sim.g_long", ts_ns, point.g_long as f64);
    push_event(
        &mut events,
        "sim.engine_temp_c",
        ts_ns,
        point.engine_temp_c as f64,
    );
    push_event(
        &mut events,
        "sim.engine_power_w",
        ts_ns,
        point.engine_power_w as f64,
    );
    events
}

fn push_event(events: &mut Vec<Event>, channel: &str, ts_ns: u64, value: f64) {
    events.push(Event {
        channel: channel.to_string(),
        ts_ns,
        value,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telemetry_channels_match_expected() {
        let point = GameTelemetryPointV1 {
            time_s: 1.0,
            s_m: 2.0,
            x_m: 3.0,
            y_m: 4.0,
            heading_rad: 5.0,
            speed_kph: 6.0,
            rpm: 7.0,
            gear: 8,
            throttle_pct: 9.0,
            brake_pct: 10.0,
            g_lat: 11.0,
            g_long: 12.0,
            engine_temp_c: 13.0,
            engine_power_w: 14.0,
        };
        let ts_ns = 42;
        let events = telemetry_point_to_events(&point, ts_ns);

        let channels: Vec<&str> = events.iter().map(|event| event.channel.as_str()).collect();
        let expected = vec![
            "sim.time_s",
            "sim.s_m",
            "sim.x_m",
            "sim.y_m",
            "sim.heading_rad",
            "sim.speed_kph",
            "sim.rpm",
            "sim.gear",
            "sim.throttle_pct",
            "sim.brake_pct",
            "sim.g_lat",
            "sim.g_long",
            "sim.engine_temp_c",
            "sim.engine_power_w",
        ];

        assert_eq!(channels, expected);
        assert!(events.iter().all(|event| event.ts_ns == ts_ns));
    }
}
