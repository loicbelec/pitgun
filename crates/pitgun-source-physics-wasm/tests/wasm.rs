use wasm_bindgen_test::wasm_bindgen_test;

use pitgun_source_physics_wasm::simulate_batches;

#[wasm_bindgen_test]
fn batches_have_end_of_stream() {
    let request = r#"{
  \"track_id\": \"demo-oval\",
  \"hz\": 60.0,
  \"tuning\": {
    \"aero_points\": 10,
    \"chassis_points\": 10,
    \"engine_points\": 10,
    \"cooling_points\": 10,
    \"downforce_slider\": 0.5,
    \"gear_ratio_slider\": 0.5
  },
  \"seed\": 1,
  \"engine_version\": \"0.1.0\"
}"#;

    let batches_json = simulate_batches(request);
    let value: serde_json::Value =
        serde_json::from_str(&batches_json).expect("batch json should parse");
    let batches = value
        .as_array()
        .expect("simulate_batches must return JSON array");
    assert!(!batches.is_empty());

    let mut end_of_stream_count = 0;
    let mut total_events = 0;

    for batch in batches {
        let batch_obj = batch.get("batch").expect("batch object");
        let end_of_stream = batch_obj
            .get("end_of_stream")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        if end_of_stream {
            end_of_stream_count += 1;
        }

        let events_len = batch_obj
            .get("events")
            .and_then(|value| value.as_array())
            .map(|events| events.len())
            .unwrap_or(0);
        total_events += events_len;
    }

    assert_eq!(end_of_stream_count, 1);
    assert!(total_events > 0);
}
