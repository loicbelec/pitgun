use anyhow::Result;
use clap::{Args, Parser, Subcommand, ValueEnum};
use pitgun_codec_udp::UdpWireFormat;
use pitgun_core::{
    ChannelFilterProcessor, ConsoleSink, Expr, FormulaProcessor, Pipeline, Processor,
    ScaleProcessor, SegmentAggregateProcessor, SegmentMetric, SegmentTarget, Sink, Source,
    StatsProcessor,
};
use pitgun_source_udp::UdpSource;
use pitgun_source_ws::WsSource;
use std::fmt;

mod demo;
mod manifest;
mod sinks;

#[derive(Parser, Debug)]
#[command(
    name = "pitgun",
    version,
    about = "Pitgun deterministic simulation tools"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Run a complete built-in demonstration workload
    Demo(DemoArgs),
    /// Subscribe to telemetry and process via core pipeline
    Subscribe(SubscribeArgs),
}

#[derive(Args, Debug)]
struct DemoArgs {
    #[command(subcommand)]
    workload: DemoWorkload,
}

#[derive(Subcommand, Debug)]
enum DemoWorkload {
    /// Run the versioned Racing reference scenario
    Racing(demo::racing::RacingArgs),
}

#[derive(ValueEnum, Clone, Debug)]
enum Transport {
    Udp,
    Ws, /*, Grpc, Kafka */
}

#[derive(clap::Args, Debug)]
struct SubscribeArgs {
    /// Select underlying transport
    #[arg(long, value_enum, default_value_t = Transport::Udp)]
    transport: Transport,

    /// Local bind address (e.g. 127.0.0.1:5001 or 0.0.0.0:5001 for multicast)
    #[arg(long, value_name = "HOST:PORT", default_value = "127.0.0.1:5001")]
    bind: String,

    /// Multicast group (IPv4 only), e.g. 239.10.0.1
    #[arg(long)]
    mcast: Option<std::net::Ipv4Addr>,

    /// Interface IPv4 for multicast join (e.g. 0.0.0.0 or your NIC address)
    #[arg(long, default_value = "0.0.0.0")]
    iface: std::net::Ipv4Addr,

    /// Stats print interval in seconds (0 = disabled)
    #[arg(long, default_value_t = 1)]
    stats_interval: u64,

    /// WebSocket URL (e.g. ws://127.0.0.1:8080/ws)
    #[arg(long, value_name = "URL")]
    ws_url: Option<String>,

    /// Optional directory to write per-channel CSV recording
    #[arg(long)]
    write_csv: Option<std::path::PathBuf>,

    /// Print each frame as JSON (noisy)
    #[arg(long, default_value_t = false)]
    json: bool,

    /// Optional channel filters (repeatable)
    #[arg(long)]
    channel: Vec<String>,

    /// Optional YAML manifest controlling the pipeline
    #[arg(long)]
    config: Option<std::path::PathBuf>,
}

type UdpV1Source = UdpSource<UdpWireFormat>;

enum SubscribeSource {
    Udp(UdpV1Source),
    Ws(WsSource),
}

impl Source for SubscribeSource {
    fn next_batch(&mut self) -> Option<pitgun_core::EventBatch> {
        match self {
            SubscribeSource::Udp(source) => source.next_batch(),
            SubscribeSource::Ws(source) => source.next_batch(),
        }
    }
}

fn main() {
    let cli = Cli::parse();
    if let Err(error) = execute(cli) {
        eprintln!("{error}");
        std::process::exit(i32::from(error.exit_code()));
    }
}

#[derive(Debug)]
enum CommandError {
    General(anyhow::Error),
    Racing(demo::racing::RacingDemoError),
    Bundle(demo::bundle::BundleError),
}

impl CommandError {
    const fn exit_code(&self) -> u8 {
        match self {
            Self::General(_) => 1,
            Self::Racing(error) => error.exit_code(),
            Self::Bundle(error) => error.exit_code(),
        }
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::General(error) => error.fmt(formatter),
            Self::Racing(error) => error.fmt(formatter),
            Self::Bundle(error) => error.fmt(formatter),
        }
    }
}

fn execute(cli: Cli) -> Result<(), CommandError> {
    match cli.cmd {
        Cmd::Demo(args) => run_demo(args),
        Cmd::Subscribe(args) => run_subscribe(args).map_err(CommandError::General),
    }
}

fn run_demo(args: DemoArgs) -> Result<(), CommandError> {
    match args.workload {
        DemoWorkload::Racing(args) => {
            let result = demo::racing::run(&args).map_err(CommandError::Racing)?;
            let bundle = demo::bundle::persist(&result, args.output.as_deref())
                .map_err(CommandError::Bundle)?;
            println!("Pitgun Racing deterministic demo\n");
            println!(
                "scenario    {}@{}",
                result.scenario.id, result.scenario.version
            );
            println!("seed        {}", result.seed);
            println!("run_id      {}", result.run_id);
            println!("input       {}", result.contract.input.digest);
            println!("output      {}", result.output_digest);
            println!("telemetry   {}", result.telemetry_summary_digest);
            println!(
                "frames      {} in {} batches",
                result.evidence.telemetry_summary.frame_count(),
                result.evidence.telemetry_summary.batch_count()
            );
            println!("race_time   {} ms", result.output.total_time_ms);
            let metric = &result.metrics.metrics[0];
            println!(
                "metric      {} = {:.2} {}",
                metric.id, metric.value, metric.unit
            );
            println!(
                "bundle      {} ({})",
                bundle.path.display(),
                bundle.disposition
            );
            println!("status      SIMULATED");
            Ok(())
        }
    }
}

fn run_subscribe(args: SubscribeArgs) -> Result<()> {
    if let Some(config_path) = &args.config {
        let path = config_path.to_string_lossy().to_string();
        let manifest = match manifest::load_manifest_from_path(&path) {
            Ok(manifest) => manifest,
            Err(err) => {
                eprintln!("failed to load manifest '{}': {}", path, err);
                std::process::exit(1);
            }
        };
        let mut pipeline = build_pipeline_from_manifest(manifest);
        loop {
            pipeline.run_once();
        }
    }

    let source = match args.transport {
        Transport::Udp => {
            let bind: std::net::SocketAddr = args.bind.parse()?;
            SubscribeSource::Udp(UdpSource::new(
                bind,
                args.mcast,
                args.iface,
                1024,
                50_000_000,
                UdpWireFormat::PitgunV1,
            )?)
        }
        Transport::Ws => {
            let url = args.ws_url.as_deref().unwrap_or_else(|| {
                eprintln!("--ws-url is required when --transport ws is set");
                std::process::exit(1);
            });
            SubscribeSource::Ws(WsSource::connect(url)?)
        }
    };

    let processors: Vec<Box<dyn Processor>> = vec![
        Box::new(ChannelFilterProcessor::new(args.channel.clone())),
        Box::new(StatsProcessor::new(args.stats_interval)),
    ];

    let mut sink = CompositeSink::default();
    if let Some(dir) = args.write_csv {
        sink.push(Box::new(sinks::CsvSink::new(dir)?));
    }
    sink.push(Box::new(ConsoleSink::new(args.json)));

    let mut pipeline = Pipeline {
        source,
        processors,
        sink,
    };

    loop {
        pipeline.run_once();
    }
}

fn build_pipeline_from_manifest(
    manifest: manifest::Manifest,
) -> Pipeline<UdpV1Source, ConsoleSink> {
    let source = match manifest.source.r#type.as_str() {
        "udp" => {
            let addr = format!("{}:{}", manifest.source.bind_addr, manifest.source.port);
            let socket_addr: std::net::SocketAddr = match addr.parse() {
                Ok(addr) => addr,
                Err(err) => {
                    eprintln!("invalid UDP bind address '{}': {}", addr, err);
                    std::process::exit(1);
                }
            };
            match UdpSource::new(
                socket_addr,
                None,
                std::net::Ipv4Addr::UNSPECIFIED,
                1024,
                50_000_000,
                UdpWireFormat::PitgunV1,
            ) {
                Ok(source) => source,
                Err(err) => {
                    eprintln!("failed to initialize UDP source: {}", err);
                    std::process::exit(1);
                }
            }
        }
        other => {
            eprintln!("unsupported source type '{}'; expected 'udp'", other);
            std::process::exit(1);
        }
    };

    let mut processors: Vec<Box<dyn Processor>> = Vec::new();
    for processor_cfg in manifest.processors {
        match processor_cfg.r#type.as_str() {
            "channel_filter" => {
                let channels = processor_cfg.channels.unwrap_or_default();
                processors.push(Box::new(ChannelFilterProcessor::new(channels)));
            }
            "scale" => {
                let channel = processor_cfg.channel.clone().unwrap_or_else(|| {
                    eprintln!("scale processor requires 'channel'");
                    std::process::exit(1);
                });
                let factor = processor_cfg.factor.unwrap_or_else(|| {
                    eprintln!("scale processor requires 'factor'");
                    std::process::exit(1);
                });
                processors.push(Box::new(ScaleProcessor::new(channel, factor)));
            }
            "formula" => {
                let output = processor_cfg.output.clone().unwrap_or_else(|| {
                    eprintln!("formula processor requires 'output'");
                    std::process::exit(1);
                });
                let ast_path = processor_cfg.ast.clone().unwrap_or_else(|| {
                    eprintln!("formula processor requires 'ast' pointing to JSON file");
                    std::process::exit(1);
                });
                let ast_contents = match std::fs::read_to_string(&ast_path) {
                    Ok(s) => s,
                    Err(err) => {
                        eprintln!("failed to read ast file '{}': {}", ast_path, err);
                        std::process::exit(1);
                    }
                };
                let expr: Expr = match serde_json::from_str(&ast_contents) {
                    Ok(expr) => expr,
                    Err(err) => {
                        eprintln!("failed to parse ast json '{}': {}", ast_path, err);
                        std::process::exit(1);
                    }
                };
                processors.push(Box::new(FormulaProcessor::new(output, expr)));
            }
            "stats" => processors.push(Box::new(StatsProcessor::new(1))),
            "segment_aggregate" => {
                let segment_key = processor_cfg.segment_key.clone().unwrap_or_else(|| {
                    eprintln!("segment_aggregate processor requires 'segment_key'");
                    std::process::exit(1);
                });
                let targets_cfg = processor_cfg.targets.clone().unwrap_or_else(|| {
                    eprintln!("segment_aggregate processor requires 'targets'");
                    std::process::exit(1);
                });
                let mut targets = Vec::new();
                for target in targets_cfg {
                    let metrics_raw = target.metrics.unwrap_or_else(|| {
                        vec![
                            "count".into(),
                            "min".into(),
                            "max".into(),
                            "mean".into(),
                            "sum".into(),
                            "stddev".into(),
                        ]
                    });
                    let mut metrics = Vec::new();
                    for m in metrics_raw {
                        match SegmentMetric::parse(&m) {
                            Some(metric) => metrics.push(metric),
                            None => {
                                eprintln!(
                                    "segment_aggregate: unsupported metric '{}'. Allowed: count,min,max,mean,sum,stddev",
                                    m
                                );
                                std::process::exit(1);
                            }
                        }
                    }
                    if metrics.is_empty() {
                        metrics.extend([
                            SegmentMetric::Count,
                            SegmentMetric::Min,
                            SegmentMetric::Max,
                            SegmentMetric::Mean,
                            SegmentMetric::Sum,
                            SegmentMetric::Stddev,
                        ]);
                    }
                    targets.push(SegmentTarget {
                        channel: target.channel,
                        metrics,
                    });
                }
                let emit_on_change = processor_cfg.emit_on_change.unwrap_or(true);
                let emit_last = processor_cfg.emit_last_segment_on_eof.unwrap_or(true);
                processors.push(Box::new(SegmentAggregateProcessor::new(
                    segment_key,
                    targets,
                    emit_on_change,
                    emit_last,
                )));
            }
            other => {
                eprintln!("unsupported processor type '{}'", other);
                std::process::exit(1);
            }
        }
    }

    let sink = match manifest.sink.r#type.as_str() {
        "console" => ConsoleSink::new(true),
        other => {
            eprintln!("unsupported sink type '{}'; expected 'console'", other);
            std::process::exit(1);
        }
    };

    Pipeline {
        source,
        processors,
        sink,
    }
}

#[derive(Default)]
struct CompositeSink {
    sinks: Vec<Box<dyn Sink>>,
}

impl CompositeSink {
    fn push(&mut self, sink: Box<dyn Sink>) {
        self.sinks.push(sink);
    }
}

impl Sink for CompositeSink {
    fn write(&mut self, batch: &pitgun_core::EventBatch) {
        for sink in self.sinks.iter_mut() {
            sink.write(batch);
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{Cli, Cmd, DemoWorkload};

    #[test]
    fn parses_racing_demo_with_default_seed() {
        let cli = Cli::try_parse_from(["pitgun", "demo", "racing"]).expect("valid CLI");

        match cli.cmd {
            Cmd::Demo(args) => match args.workload {
                DemoWorkload::Racing(args) => {
                    assert_eq!(args.seed, 42);
                    assert_eq!(args.output, None);
                }
            },
            Cmd::Subscribe(_) => panic!("expected demo command"),
        }
    }

    #[test]
    fn parses_explicit_u64_seed() {
        let cli =
            Cli::try_parse_from(["pitgun", "demo", "racing", "--seed", "18446744073709551615"])
                .expect("valid u64 seed");

        match cli.cmd {
            Cmd::Demo(args) => match args.workload {
                DemoWorkload::Racing(args) => assert_eq!(args.seed, u64::MAX),
            },
            Cmd::Subscribe(_) => panic!("expected demo command"),
        }
    }

    #[test]
    fn rejects_invalid_seed() {
        assert!(Cli::try_parse_from(["pitgun", "demo", "racing", "--seed", "not-a-seed"]).is_err());
    }

    #[test]
    fn parses_exact_bundle_destination() {
        let cli = Cli::try_parse_from(["pitgun", "demo", "racing", "--output", "runs/example"])
            .expect("valid bundle destination");

        match cli.cmd {
            Cmd::Demo(args) => match args.workload {
                DemoWorkload::Racing(args) => {
                    assert_eq!(
                        args.output.as_deref(),
                        Some(std::path::Path::new("runs/example"))
                    );
                }
            },
            Cmd::Subscribe(_) => panic!("expected demo command"),
        }
    }
}
