#[cfg(target_arch = "wasm32")]
mod wasm {
    use pitgun_contract::game::v1::GameSimulationRequestV1;
    use pitgun_source_physics::game::simulate_request;
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    pub fn simulate(request_json: &str) -> Result<String, JsValue> {
        let request: GameSimulationRequestV1 = serde_json::from_str(request_json)
            .map_err(|err| js_error(format!("invalid request JSON: {err}")))?;
        let result = simulate_request(&request)
            .map_err(|err| js_error(format!("simulation failed: {err}")))?;
        serde_json::to_string(&result)
            .map_err(|err| js_error(format!("failed to serialize result: {err}")))
    }

    fn js_error(message: impl ToString) -> JsValue {
        JsValue::from_str(&message.to_string())
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm::simulate;
