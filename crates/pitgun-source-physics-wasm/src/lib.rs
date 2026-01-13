#[cfg(target_arch = "wasm32")]
mod wasm {
    use js_sys::Date;
    use pitgun_contract::SignedSimulationContractV1;
    use pitgun_core::Source;
    use pitgun_signing::SigningKey;
    use pitgun_source_physics::{PhysicsSource, PhysicsSourceConfig};
    use serde::Serialize;
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    pub struct PhysicsWasm {
        inner: PhysicsSource,
    }

    #[wasm_bindgen]
    impl PhysicsWasm {
        #[wasm_bindgen(constructor)]
        pub fn new(signed_contract_json: &str) -> Result<PhysicsWasm, JsValue> {
            let signed: SignedSimulationContractV1 = serde_json::from_str(signed_contract_json)
                .map_err(|err| js_error(format!("invalid signed contract JSON: {err}")))?;
            let config =
                PhysicsSourceConfig::from_simulation_contract_at(&signed.contract, now_ms())
                    .map_err(js_error)?;
            Ok(PhysicsWasm {
                inner: PhysicsSource::new(config),
            })
        }

        pub fn new_with_secret(
            signed_contract_json: &str,
            secret: &str,
        ) -> Result<PhysicsWasm, JsValue> {
            let signed: SignedSimulationContractV1 = serde_json::from_str(signed_contract_json)
                .map_err(|err| js_error(format!("invalid signed contract JSON: {err}")))?;
            let key = SigningKey::from_secret(secret.as_bytes())
                .map_err(|err| js_error(err.to_string()))?;
            let config = PhysicsSourceConfig::from_signed_simulation_contract_with_key_at(
                &signed,
                &key,
                now_ms(),
            )
            .map_err(js_error)?;
            Ok(PhysicsWasm {
                inner: PhysicsSource::new(config),
            })
        }

        pub fn next_batch(&mut self) -> JsValue {
            let batch = match self.inner.next_batch() {
                Some(batch) => batch,
                None => {
                    return to_js_value(&JsBatch {
                        end_of_stream: true,
                        events: Vec::new(),
                    });
                }
            };

            let events = batch
                .events
                .into_iter()
                .map(|event| JsEvent {
                    channel: event.channel,
                    ts_ns: event.ts_ns as f64,
                    value: event.value,
                })
                .collect();

            to_js_value(&JsBatch {
                end_of_stream: batch.end_of_stream,
                events,
            })
        }
    }

    #[derive(Serialize)]
    struct JsEvent {
        channel: String,
        ts_ns: f64,
        value: f64,
    }

    #[derive(Serialize)]
    struct JsBatch {
        end_of_stream: bool,
        events: Vec<JsEvent>,
    }

    fn to_js_value<T: Serialize>(value: &T) -> JsValue {
        serde_wasm_bindgen::to_value(value).unwrap_or_else(|err| js_error(err.to_string()))
    }

    fn js_error(message: impl ToString) -> JsValue {
        JsValue::from_str(&message.to_string())
    }

    fn now_ms() -> i64 {
        Date::now() as i64
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm::PhysicsWasm;
