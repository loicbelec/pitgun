//! Non-secret policy identities used to pin the local runtime qualification.
fn main() {
    println!(
        "{}",
        serde_json::json!({
            "power":pitgun_racing_simulator::power_telemetry_policy_identity(),
            "service":pitgun_racing_simulator::service_control::power_service_control_policy_identity(),
            "legacyService":pitgun_racing_simulator::service_control::service_control_policy_identity(),
            "channels":pitgun_racing_simulator::POWER_CHANNELS,
        })
    );
}
