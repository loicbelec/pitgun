use anyhow::Context;
use reqwest::{Client, StatusCode};
use serde::Serialize;
use serde_json::Value;

use crate::model::PitWallSessionConfiguredPayload;

#[derive(Clone)]
pub struct RunRegistryClient {
    client: Client,
    base_url: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct RunRegistryUpsertRequest {
    pub player_id: String,
    pub weekend_id: Option<String>,
    pub session_id: String,
    pub run_id: String,
    pub track_id: String,
    pub vehicle_id: String,
    pub session_type: String,
    pub seed: u64,
    pub sampling_hz: f64,
    pub game_version: Option<String>,
    pub wasm_source_commit: Option<String>,
    pub wasm_build_time: Option<String>,
    pub setup: Value,
    pub setup_offsets: Value,
    pub effective_setup: Value,
    pub stint_strategy: Option<Value>,
}

impl RunRegistryClient {
    pub fn new(base_url: impl Into<String>) -> anyhow::Result<Self> {
        let client = Client::builder()
            .build()
            .context("failed to build run registry HTTP client")?;

        Ok(Self {
            client,
            base_url: base_url.into(),
        })
    }

    pub async fn upsert_run(&self, payload: &RunRegistryUpsertRequest) -> anyhow::Result<()> {
        let response = self
            .client
            .post(&self.base_url)
            .json(payload)
            .send()
            .await
            .with_context(|| format!("failed to POST pitwall run to {}", self.base_url))?;

        if matches!(response.status(), StatusCode::OK | StatusCode::CREATED) {
            return Ok(());
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!(
            "run registry rejected pitwall run with status {}: {}",
            status,
            body
        );
    }
}

impl RunRegistryUpsertRequest {
    pub fn from_configured_event(
        player_id: &str,
        session_id: &str,
        payload: &PitWallSessionConfiguredPayload,
    ) -> Self {
        Self {
            player_id: player_id.to_string(),
            weekend_id: payload.weekend_id.clone(),
            session_id: session_id.to_string(),
            run_id: payload.run_id.clone(),
            track_id: payload.track_id.clone(),
            vehicle_id: payload.vehicle_id.clone(),
            session_type: payload.session_type.clone(),
            seed: payload.seed,
            sampling_hz: payload.sampling_hz,
            game_version: payload.game_version.clone(),
            wasm_source_commit: payload.wasm_source_commit.clone(),
            wasm_build_time: payload.wasm_build_time.clone(),
            setup: payload.setup.clone(),
            setup_offsets: payload.setup_offsets.clone(),
            effective_setup: payload.effective_setup.clone(),
            stint_strategy: payload.stint_strategy.clone(),
        }
    }
}
