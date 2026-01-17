use anyhow::{bail, Context, Result};
use clap::Parser;
use pitgun_codec_json::EventBatchDto;
use pitgun_contract::game::v1::GameSimulationRequestV1;
use pitgun_core::EventBatch;
use pitgun_signing::{verify_game_contract_v1_with_key, GameContractVerification, SigningKey};
use pitgun_source_physics::game::batching::telemetry_to_event_batches;
use pitgun_source_physics::game::json::{
    deserialize_game_simulation_contract_v1, deserialize_game_simulation_request_v1,
    extract_game_simulation_request_v1,
};
use pitgun_source_physics::game::simulate_request;
use reqwest::blocking::Client;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tungstenite::{connect, Message};
use url::Url;

#[derive(Parser, Debug)]
#[command(
    name = "pitgun-physics-http-emitter",
    version,
    about = "Run physics simulation and emit telemetry batches to pitgun-telemetryd"
)]
struct Args {
    /// Telemetryd beacon endpoint (SessionEnvelope JSON)
    #[arg(long, default_value = "http://127.0.0.1:8080/beacon")]
    telemetryd_url: String,

    /// Session id to stamp on emitted batches
    #[arg(long, default_value = "dev-session-1")]
    session_id: String,

    /// Configd response JSON { request, signature }
    #[arg(long, value_name = "PATH")]
    contract_json: Option<PathBuf>,

    /// Raw GameSimulationRequestV1 JSON
    #[arg(long, value_name = "PATH")]
    request_json: Option<PathBuf>,

    /// Override request hz
    #[arg(long)]
    hz: Option<f32>,

    /// Max events per batch
    #[arg(long, default_value_t = 260)]
    batch_events: usize,

    /// WebSocket URL for telemetryd (/ws). If set, uses WS instead of HTTP.
    #[arg(long, value_name = "URL")]
    ws_url: Option<String>,

    /// Skip HTTP POST, print summary only
    #[arg(long, default_value_t = false)]
    dry_run: bool,

    /// Verify signature for contract-json (requires PITGUN_SIGNING_SECRET)
    #[arg(long, default_value_t = false)]
    verify_signature: bool,

    /// Allow expired contracts when verifying signatures
    #[arg(long, default_value_t = false)]
    allow_expired: bool,
}

#[derive(Serialize)]
struct SessionEnvelopeOut {
    schema_version: u32,
    session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    sent_at_ms: Option<i64>,
    batch: EventBatchDto,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let (request, verification) = load_request(&args)?;

    if args.batch_events == 0 {
        bail!("--batch-events must be greater than 0");
    }

    let simulation = simulate_request(&request).map_err(|err| anyhow::anyhow!("{err}"))?;

    let telemetry = simulation.telemetry.unwrap_or_default();
    let batch_summary = telemetry_to_event_batches(&telemetry, args.batch_events);
    let total_events = batch_summary.total_events;
    let first_ts = batch_summary.first_ts_ns;
    let last_ts = batch_summary.last_ts_ns;
    let batches = batch_summary.batches;
    let total_batches = batches.len();

    if !args.dry_run {
        if let Some(ws_url) = args.ws_url.as_deref() {
            send_ws_batches(ws_url, &args.session_id, &batches)?;
        } else {
            send_http_batches(&args.telemetryd_url, &args.session_id, &batches)?;
        }
    }

    print_summary(
        &request,
        telemetry.len(),
        total_events,
        total_batches,
        first_ts,
        last_ts,
        verification,
    );

    Ok(())
}

fn load_request(
    args: &Args,
) -> Result<(GameSimulationRequestV1, Option<GameContractVerification>)> {
    let has_contract = args.contract_json.is_some();
    let has_request = args.request_json.is_some();

    if has_contract == has_request {
        bail!("exactly one of --contract-json or --request-json must be provided");
    }

    if args.verify_signature && !has_contract {
        bail!("--verify-signature requires --contract-json");
    }

    let mut verification = None;
    let mut request = if let Some(path) = &args.contract_json {
        let raw = fs::read(path).with_context(|| format!("reading contract JSON at {path:?}"))?;
        let contract =
            deserialize_game_simulation_contract_v1(&raw).context("invalid contract JSON")?;
        if args.verify_signature {
            let key = SigningKey::from_env()
                .map_err(|err| anyhow::anyhow!("signature verification requested but {err}"))?;
            let result = verify_game_contract_v1_with_key(&contract, &key)
                .context("failed to verify contract signature")?;
            verification = Some(result);
            if !result.signature_ok {
                bail!("signature verification failed");
            }
            if !result.time_bounds_ok {
                bail!("contract time bounds are invalid");
            }
            if result.expired && !args.allow_expired {
                bail!("contract has expired");
            }
        }
        extract_game_simulation_request_v1(&contract).clone()
    } else {
        let path = args.request_json.as_ref().expect("request_json checked");
        let raw = fs::read(path).with_context(|| format!("reading request JSON at {path:?}"))?;
        deserialize_game_simulation_request_v1(&raw).map_err(|err| anyhow::anyhow!("{err}"))?
    };

    if let Some(hz) = args.hz {
        request.hz = hz;
    }

    Ok((request, verification))
}

fn now_ms() -> i64 {
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    dur.as_millis() as i64
}

fn print_summary(
    request: &GameSimulationRequestV1,
    telemetry_frames: usize,
    total_events: usize,
    total_batches: usize,
    first_ts: Option<u64>,
    last_ts: Option<u64>,
    verification: Option<GameContractVerification>,
) {
    println!(
        "request track_id={} hz={} tuning={:?}",
        request.track_id, request.hz, request.tuning
    );
    println!("telemetry_frames={}", telemetry_frames);
    println!("total_events={}", total_events);
    println!("total_batches={}", total_batches);
    println!("first_ts_ns={}", first_ts.unwrap_or(0));
    println!("last_ts_ns={}", last_ts.unwrap_or(0));
    if let Some(result) = verification {
        println!("signature_ok={}", result.signature_ok);
        println!("contract_expired={}", result.expired);
        println!("contract_now_ms={}", result.now_ms);
    }
}

fn send_http_batches(url: &str, session_id: &str, batches: &[EventBatch]) -> Result<()> {
    let client = Client::new();
    for batch in batches {
        let envelope = build_envelope(session_id, batch);
        let response = client
            .post(url)
            .json(&envelope)
            .send()
            .context("failed to POST SessionEnvelope")?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            bail!("telemetryd returned {status}: {body}");
        }
    }
    Ok(())
}

fn send_ws_batches(url: &str, session_id: &str, batches: &[EventBatch]) -> Result<()> {
    let url = Url::parse(url).context("invalid ws URL")?;
    let (mut socket, _) = connect(url).context("failed to connect to ws endpoint")?;

    for batch in batches {
        let envelope = build_envelope(session_id, batch);
        let payload = serde_json::to_string(&envelope).context("serialize WS envelope")?;
        socket
            .send(Message::Text(payload))
            .context("failed to send WS message")?;
    }

    let _ = socket.close(None);
    Ok(())
}

fn build_envelope(session_id: &str, batch: &EventBatch) -> SessionEnvelopeOut {
    SessionEnvelopeOut {
        schema_version: 1,
        session_id: session_id.to_string(),
        sent_at_ms: Some(now_ms()),
        batch: EventBatchDto::from(batch),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pitgun_contract::game::v1::GameTelemetryPointV1;

    #[test]
    fn batches_end_with_eos() {
        let telemetry = vec![
            GameTelemetryPointV1 {
                time_s: 0.0,
                s_m: 0.0,
                x_m: 0.0,
                y_m: 0.0,
                heading_rad: 0.0,
                speed_kph: 1.0,
                rpm: 1000.0,
                gear: 1,
                throttle_pct: 0.2,
                brake_pct: 0.0,
                g_lat: 0.0,
                g_long: 0.1,
                engine_temp_c: 90.0,
                engine_power_w: 1000.0,
            },
            GameTelemetryPointV1 {
                time_s: 0.1,
                s_m: 1.0,
                x_m: 1.0,
                y_m: 0.0,
                heading_rad: 0.1,
                speed_kph: 2.0,
                rpm: 1100.0,
                gear: 1,
                throttle_pct: 0.3,
                brake_pct: 0.0,
                g_lat: 0.0,
                g_long: 0.1,
                engine_temp_c: 91.0,
                engine_power_w: 1100.0,
            },
        ];

        let batch_summary = telemetry_to_event_batches(&telemetry, 10);
        let batches = batch_summary.batches;

        assert!(!batches.is_empty());
        for batch in batches.iter().take(batches.len().saturating_sub(1)) {
            assert!(!batch.end_of_stream);
        }
        assert!(batches.last().unwrap().end_of_stream);
    }
}
