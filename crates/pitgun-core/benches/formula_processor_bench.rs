use std::cmp::Ordering;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use criterion::{BatchSize, Criterion, black_box};
use pitgun_core::{BinaryOp, Event, EventBatch, Expr, FormulaProcessor, Processor};
use serde::Serialize;

const ENGINE_PATH: &str = "datasets/telemetry/FIA-nEngine.csv";
const THROTTLE_PATH: &str = "datasets/telemetry/Controller-rThrottleR.csv";

static REPORT: OnceLock<Mutex<Vec<ScenarioMetrics>>> = OnceLock::new();

#[derive(Debug, Serialize, Clone)]
struct ScenarioMetrics {
    name: String,
    iterations: u64,
    dataset_events: usize,
    formulas: usize,
    mean_ns: f64,
    p95_ns: f64,
    p99_ns: f64,
    throughput_eps: f64,
}

#[derive(Debug, Serialize)]
struct BenchReport {
    runner: &'static str,
    scenarios: Vec<ScenarioMetrics>,
}

struct Fixtures {
    n_engine: Vec<Event>,
    throttle: Vec<Event>,
}

impl Fixtures {
    fn load() -> Self {
        let n_engine = load_channel(ENGINE_PATH, "FIA-nEngine", 10_000);
        let throttle = load_channel(THROTTLE_PATH, "Controller-rThrottleR", 10_000);
        Self { n_engine, throttle }
    }

    fn interleaved_batch(&self, limit_each: usize) -> EventBatch {
        let available = limit_each.min(self.n_engine.len()).min(self.throttle.len());
        let mut events = Vec::with_capacity(available * 2);
        for i in 0..available {
            events.push(self.n_engine[i].clone());
            events.push(self.throttle[i].clone());
        }
        EventBatch {
            events,
            aggregates: Vec::new(),
            end_of_stream: false,
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct CsvRow {
    #[serde(rename = "Timestamp")]
    ts: u128,
    #[serde(rename = "ChannelValue")]
    val: f64,
}

fn load_channel(path: &str, channel: &str, max_rows: usize) -> Vec<Event> {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(path);

    let rdr = match csv::Reader::from_path(&base) {
        Ok(rdr) => rdr,
        Err(err) => {
            eprintln!(
                "bench fixture {} missing ({}), falling back to synthetic data",
                base.display(),
                err
            );
            // Fallback: generate synthetic telemetry
            return (0..max_rows)
                .map(|i| Event {
                    channel: channel.to_string(),
                    ts_ns: i as u64 * 1_000_000,    // 1 ms step, par ex.
                    value: (i as f64 * 0.01).sin(), // n'importe quel pattern déterministe
                })
                .collect();
        }
    };

    rdr.into_deserialize::<CsvRow>()
        .take(max_rows)
        .enumerate()
        .filter_map(|(i, row)| match row {
            Ok(row) => Some(Event {
                channel: channel.to_string(),
                ts_ns: row.ts as u64,
                value: row.val,
            }),
            Err(err) => {
                eprintln!("skipping row {} in {}: {}", i, base.display(), err);
                None
            }
        })
        .collect()
}

fn percentile(values: &[f64], pct: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut data = values.to_owned();
    data.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    let rank = ((pct / 100.0) * (data.len() as f64 - 1.0)).round() as usize;
    data[rank.min(data.len() - 1)]
}

fn record_metrics(metrics: ScenarioMetrics) {
    REPORT
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .unwrap()
        .push(metrics);
}

fn write_report() {
    let Some(results) = REPORT.get() else {
        return;
    };
    let guard = results.lock().unwrap();
    let report = BenchReport {
        runner: "formula_processor_v1",
        scenarios: guard.clone(),
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let out_path = root.join("benchmarks").join("results");
    std::fs::create_dir_all(&out_path)
        .unwrap_or_else(|e| panic!("failed to create {}: {}", out_path.display(), e));
    let file = out_path.join("formula_processor_v1.json");
    let json = serde_json::to_string_pretty(&report).expect("serialize report");
    std::fs::write(&file, json).unwrap_or_else(|e| panic!("write {}: {}", file.display(), e));
    println!("Benchmark report written to {}", file.display());
}

fn run_scenario<F>(
    name: &str,
    batch_template: &EventBatch,
    iterations: u64,
    mut make_procs: F,
) -> ScenarioMetrics
where
    F: FnMut() -> Vec<FormulaProcessor>,
{
    let mut procs = make_procs();
    let mut samples_ns = Vec::with_capacity(iterations as usize);
    let mut processed_events: usize = 0;

    let wall_start = Instant::now();
    for _ in 0..iterations {
        let mut batch = batch_template.clone();
        let iter_start = Instant::now();
        for proc in procs.iter_mut() {
            proc.process(&mut batch);
        }
        let elapsed = iter_start.elapsed();
        samples_ns.push(elapsed.as_secs_f64() * 1e9);
        processed_events += batch.events.len();
        black_box(&batch);
    }
    let total_time = wall_start.elapsed().as_secs_f64().max(f64::EPSILON);
    let mean_ns = samples_ns.iter().sum::<f64>() / samples_ns.len() as f64;
    let p95_ns = percentile(&samples_ns, 95.0);
    let p99_ns = percentile(&samples_ns, 99.0);
    let throughput_eps = processed_events as f64 / total_time;

    ScenarioMetrics {
        name: name.to_string(),
        iterations,
        dataset_events: batch_template.events.len(),
        formulas: procs.len(),
        mean_ns,
        p95_ns,
        p99_ns,
        throughput_eps,
    }
}

fn build_single_formula() -> Vec<FormulaProcessor> {
    let expr = Expr::BinaryOp {
        op: BinaryOp::Mul,
        left: Box::new(Expr::Var("FIA-nEngine".into())),
        right: Box::new(Expr::Var("Controller-rThrottleR".into())),
    };
    vec![FormulaProcessor::new("omega_engine".into(), expr)]
}

fn build_medium_formulas() -> Vec<FormulaProcessor> {
    vec![
        FormulaProcessor::new(
            "power_proxy".into(),
            Expr::BinaryOp {
                op: BinaryOp::Mul,
                left: Box::new(Expr::Var("FIA-nEngine".into())),
                right: Box::new(Expr::Var("Controller-rThrottleR".into())),
            },
        ),
        FormulaProcessor::new(
            "load_adjusted".into(),
            Expr::BinaryOp {
                op: BinaryOp::Add,
                left: Box::new(Expr::Var("FIA-nEngine".into())),
                right: Box::new(Expr::BinaryOp {
                    op: BinaryOp::Mul,
                    left: Box::new(Expr::Var("Controller-rThrottleR".into())),
                    right: Box::new(Expr::Literal(100.0)),
                }),
            },
        ),
        FormulaProcessor::new(
            "efficiency".into(),
            Expr::BinaryOp {
                op: BinaryOp::Div,
                left: Box::new(Expr::Var("Controller-rThrottleR".into())),
                right: Box::new(Expr::BinaryOp {
                    op: BinaryOp::Add,
                    left: Box::new(Expr::Var("FIA-nEngine".into())),
                    right: Box::new(Expr::Literal(1.0)),
                }),
            },
        ),
        FormulaProcessor::new(
            "delta_rpm".into(),
            Expr::BinaryOp {
                op: BinaryOp::Sub,
                left: Box::new(Expr::Var("FIA-nEngine".into())),
                right: Box::new(Expr::Literal(10_000.0)),
            },
        ),
    ]
}

fn build_stress_formulas(count: usize) -> Vec<FormulaProcessor> {
    (0..count)
        .map(|i| {
            let factor = 0.1 * (i as f64 + 1.0);
            let expr = Expr::BinaryOp {
                op: BinaryOp::Mul,
                left: Box::new(Expr::BinaryOp {
                    op: BinaryOp::Add,
                    left: Box::new(Expr::Var("FIA-nEngine".into())),
                    right: Box::new(Expr::Literal(factor)),
                }),
                right: Box::new(Expr::BinaryOp {
                    op: BinaryOp::Add,
                    left: Box::new(Expr::Var("Controller-rThrottleR".into())),
                    right: Box::new(Expr::Literal(1.0 + (i as f64 % 5.0))),
                }),
            };
            FormulaProcessor::new(format!("stress_{}", i), expr)
        })
        .collect()
}

fn bench_single_formula(c: &mut Criterion, fixtures: &Fixtures) {
    let batch = fixtures.interleaved_batch(256);
    let metrics = run_scenario("single_formula_small", &batch, 5_000, build_single_formula);
    record_metrics(metrics);

    let mut procs = build_single_formula();
    c.bench_function("formula_v1/single_small", |b| {
        b.iter_batched(
            || batch.clone(),
            |mut batch_run| {
                for proc in procs.iter_mut() {
                    proc.process(&mut batch_run);
                }
                black_box(batch_run);
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_multi_formula(c: &mut Criterion, fixtures: &Fixtures) {
    let batch = fixtures.interleaved_batch(2_048);
    let metrics = run_scenario("multi_formula_medium", &batch, 2_000, || {
        build_medium_formulas()
    });
    record_metrics(metrics);

    let mut procs = build_medium_formulas();
    c.bench_function("formula_v1/multi_medium", |b| {
        b.iter_batched(
            || batch.clone(),
            |mut batch_run| {
                for proc in procs.iter_mut() {
                    proc.process(&mut batch_run);
                }
                black_box(batch_run);
            },
            BatchSize::LargeInput,
        );
    });
}

fn bench_stress(c: &mut Criterion, fixtures: &Fixtures) {
    let batch = fixtures.interleaved_batch(8_000);
    let stress_formulas = build_stress_formulas(32);
    let metrics = run_scenario("stress_many_formulas", &batch, 500, || {
        build_stress_formulas(32)
    });
    record_metrics(metrics);

    let mut procs = stress_formulas;
    c.bench_function("formula_v1/stress_large", |b| {
        b.iter_batched(
            || batch.clone(),
            |mut batch_run| {
                for proc in procs.iter_mut() {
                    proc.process(&mut batch_run);
                }
                black_box(batch_run);
            },
            BatchSize::LargeInput,
        );
    });
}

fn main() {
    let fixtures = Fixtures::load();
    let mut c = Criterion::default().configure_from_args();
    bench_single_formula(&mut c, &fixtures);
    bench_multi_formula(&mut c, &fixtures);
    bench_stress(&mut c, &fixtures);
    write_report();
}
