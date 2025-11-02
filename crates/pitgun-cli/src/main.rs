use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use futures_util::StreamExt;
use pitgun_core::{Source, SourceConfig, Sink};

mod source_udp;
mod sinks;

#[derive(Parser, Debug)]
#[command(name = "pitgun-cli", version, about = "Pitgun CLI tools")]
struct Cli { #[command(subcommand)] cmd: Cmd }

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Subscribe to telemetry and process via core pipeline
    Subscribe(SubscribeArgs),
}

#[derive(ValueEnum, Clone, Debug)]
enum Transport { Udp /*, Grpc, Kafka */ }

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

    /// Optional directory to write per-channel CSV recording
    #[arg(long)]
    write_csv: Option<std::path::PathBuf>,

    /// Print each frame as JSON (noisy)
    #[arg(long, default_value_t = false)]
    json: bool,

    /// Optional channel filters (repeatable)
    #[arg(long)]
    channel: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Subscribe(args) => run_subscribe(args).await,
    }
}

async fn run_subscribe(args: SubscribeArgs) -> Result<()> {
    // 1) Construire la Source (UDP pour l’instant)
    let source_cfg = SourceConfig {
        channels: if args.channel.is_empty() { None } else { Some(args.channel.clone()) },
        batch_max_len: 1024,
        batch_max_ns:  50_000_000, // 50ms
    };

    let source: Box<dyn Source<Error=anyhow::Error> + Send + Sync> = match args.transport {
        Transport::Udp => {
            let bind: std::net::SocketAddr = args.bind.parse()?;
            Box::new(source_udp::UdpSource::new(bind, args.mcast, args.iface))
        }
        // Transport::Grpc => un jour: Box::new(source_grpc::GrpcSource::new(...)),
        // Transport::Kafka => ...
    };

    // 2) Brancher les sinks (CSV / JSON / Stats)
    let mut sink_list: Vec<Box<dyn Sink<Error=anyhow::Error> + Send + Sync>> = vec![];
    if let Some(dir) = &args.write_csv {
        sink_list.push(Box::new(sinks::CsvSink::new(dir.clone())?));
    }
    if args.json { sink_list.push(Box::new(sinks::JsonSink)); }
    sink_list.push(Box::new(sinks::StatsSink::new(args.stats_interval)));

    // 3) Exécuter le pipeline
    let mut stream = source.stream(source_cfg).await?;
    while let Some(batch) = stream.next().await {
        let batch = batch?;
        for s in sink_list.iter() {
            s.write(batch.clone()).await?;
        }
    }
    Ok(())
}