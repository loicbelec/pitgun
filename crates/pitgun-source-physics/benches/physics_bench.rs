use std::fs;
use std::path::PathBuf;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use pitgun_codec_json::{EventBatchDto, SESSION_ENVELOPE_SCHEMA_VERSION};
use pitgun_contract::game::v1::GameSimulationRequestV1;
use pitgun_source_physics::game::batching::telemetry_to_event_batches;
use pitgun_source_physics::game::{TrackRegistry, DEFAULT_TRACK_ID};
use pitgun_source_physics::run_simulation;
use serde::Serialize;

const FIXTURES: &[&str] = &["demo-oval_default", "demo-oval_extreme"];
const BATCH_EVENTS: usize = 260;

#[derive(Clone)]
struct Fixture {
    name: &'static str,
    request: GameSimulationRequestV1,
}

#[derive(Serialize)]
struct SessionEnvelope {
    schema_version: u32,
    session_id: String,
    sent_at_ms: Option<i64>,
    batch: EventBatchDto,
}

fn load_fixtures() -> Vec<Fixture> {
    FIXTURES
        .iter()
        .map(|name| Fixture {
            name,
            request: load_request(name),
        })
        .collect()
}

fn load_request(name: &str) -> GameSimulationRequestV1 {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("requests")
        .join(format!("{name}.json"));
    let raw =
        fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    serde_json::from_str(&raw).expect("fixture request json")
}

fn load_track() -> Vec<pitgun_source_physics::TrackPoint> {
    let registry = TrackRegistry::default();
    registry
        .load(DEFAULT_TRACK_ID)
        .expect("demo-oval track should load")
}

fn bench_physics_only(c: &mut Criterion) {
    let fixtures = load_fixtures();
    let track = load_track();
    let mut group = c.benchmark_group("physics_only");

    for fixture in &fixtures {
        group.bench_function(fixture.name, |b| {
            let tuning = fixture.request.tuning;
            let hz = fixture.request.hz;
            b.iter(|| {
                let telemetry = run_simulation(&track, tuning, hz).expect("simulate");
                black_box(telemetry);
            });
        });
    }
    group.finish();
}

fn bench_physics_to_batches(c: &mut Criterion) {
    let fixtures = load_fixtures();
    let track = load_track();
    let mut group = c.benchmark_group("physics_to_batches");

    for fixture in &fixtures {
        group.bench_function(fixture.name, |b| {
            let tuning = fixture.request.tuning;
            let hz = fixture.request.hz;
            b.iter(|| {
                let telemetry = run_simulation(&track, tuning, hz).expect("simulate");
                let summary = telemetry_to_event_batches(&telemetry, BATCH_EVENTS);
                let envelopes: Vec<SessionEnvelope> = summary
                    .batches
                    .iter()
                    .map(|batch| SessionEnvelope {
                        schema_version: SESSION_ENVELOPE_SCHEMA_VERSION,
                        session_id: "bench-session".to_string(),
                        sent_at_ms: None,
                        batch: EventBatchDto::from(batch),
                    })
                    .collect();
                black_box(envelopes);
            });
        });
    }
    group.finish();
}

fn bench_physics_to_batches_json(c: &mut Criterion) {
    let fixtures = load_fixtures();
    let track = load_track();
    let mut group = c.benchmark_group("physics_to_batches_json");

    for fixture in &fixtures {
        group.bench_function(fixture.name, |b| {
            let tuning = fixture.request.tuning;
            let hz = fixture.request.hz;
            b.iter(|| {
                let telemetry = run_simulation(&track, tuning, hz).expect("simulate");
                let summary = telemetry_to_event_batches(&telemetry, BATCH_EVENTS);
                let envelopes: Vec<SessionEnvelope> = summary
                    .batches
                    .iter()
                    .map(|batch| SessionEnvelope {
                        schema_version: SESSION_ENVELOPE_SCHEMA_VERSION,
                        session_id: "bench-session".to_string(),
                        sent_at_ms: None,
                        batch: EventBatchDto::from(batch),
                    })
                    .collect();
                let json = serde_json::to_string(&envelopes).expect("serialize batches");
                black_box(json);
            });
        });
    }
    group.finish();
}

criterion_group!(
    physics_benches,
    bench_physics_only,
    bench_physics_to_batches,
    bench_physics_to_batches_json
);
criterion_main!(physics_benches);
