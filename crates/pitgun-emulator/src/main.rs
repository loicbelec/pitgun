use anyhow::{Context, Result};
use clap::Parser;
use csv::ReaderBuilder;
use socket2::{Domain, Protocol, Socket, Type};
use std::fs::File;
use std::io::BufReader;
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::PathBuf;
use std::thread::sleep;
use std::time::{Duration, Instant};

/// UDP telemetry emitter reading a 2-col CSV: Timestamp,ChannelValue
#[derive(Parser, Debug)]
#[command(name="pitgun-emulator", version, about="Emit telemetry from CSV over UDP")]
struct Args {
    /// Target address, e.g. 239.10.0.1:5001 (multicast) or 127.0.0.1:5001 (unicast)
    #[arg(long, value_name="HOST:PORT")]
    target: String,

    /// Input CSV with headers: Timestamp,ChannelValue
    #[arg(long, value_name="PATH")]
    csv: PathBuf,

    /// Respect CSV timing (pacing based on Timestamp deltas). If not set, emit as fast as possible.
    #[arg(long, default_value_t = false)]
    pace: bool,

    /// Optional: override channel name (default = file stem, e.g. FIA-nEngine)
    #[arg(long)]
    channel: Option<String>,

    /// Multicast TTL (only used for multicast targets)
    #[arg(long, default_value_t = 1)]
    mcast_ttl: u32,
}

#[derive(Debug, Clone)]
struct Row {
    ts_ns: u128,
    value: f64,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // ---- Channel name from file name (or --channel)
    let channel = args
        .channel
        .clone()
        .or_else(|| args.csv.file_stem().map(|s| s.to_string_lossy().to_string()))
        .unwrap_or_else(|| "unknown-channel".to_string());

    // ---- Resolve and prepare UDP socket
    let target = resolve_target(&args.target)?;
    let sock = make_udp_socket(&target, args.mcast_ttl)?;
    sock.set_nonblocking(false)?;
    let std_sock = std::net::UdpSocket::from(sock); // <- remplace into_udp_socket()
    std_sock.connect(target)?;

    // ---- Read CSV rows
    let rows = read_csv(&args.csv)
        .with_context(|| format!("while reading CSV {:?}", args.csv))?;
    if rows.is_empty() {
        anyhow::bail!("CSV contains no data rows");
    }

    eprintln!(
        "Emitting {} rows from {:?} on channel '{}' to {} (pace={})",
        rows.len(),
        args.csv,
        channel,
        target,
        args.pace
    );

    // ---- Emit loop
    let start_monotonic = Instant::now();
    let t0_ns = rows.first().unwrap().ts_ns;

    for (i, r) in rows.iter().enumerate() {
        if args.pace {
            // Sleep until (row_ts - t0) has elapsed since we started
            let target_elapsed_ns = r.ts_ns.saturating_sub(t0_ns);
            let target_elapsed =
                Duration::from_nanos((target_elapsed_ns as u64).min(u64::MAX));
            let now_elapsed = start_monotonic.elapsed();
            if target_elapsed > now_elapsed {
                sleep(target_elapsed - now_elapsed);
            }
        }

        let frame = encode_frame(&channel, r.ts_ns, r.value);
        std_sock.send(&frame)?;

        if i % 1_000 == 0 && i > 0 {
            let rate = (i as f64 / start_monotonic.elapsed().as_secs_f64().max(1e-6)).round();
            eprintln!("sent={} rate≈{} fps", i, rate);
        }
    }

    eprintln!("Done. total_sent={}", rows.len());
    Ok(())
}

/// Resolve HOST:PORT to SocketAddr
fn resolve_target(s: &str) -> Result<SocketAddr> {
    let mut iter = s
        .to_socket_addrs()
        .with_context(|| format!("invalid target '{}'", s))?;
    iter.next().context("could not resolve target")
}

/// Create a UDP socket configured for unicast or multicast
fn make_udp_socket(target: &SocketAddr, mcast_ttl: u32) -> Result<Socket> {
    let domain = match target {
        SocketAddr::V4(_) => Domain::IPV4,
        SocketAddr::V6(_) => Domain::IPV6,
    };
    let sock = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP))?;

    // Multicast-specific options (IPv4)
    if let SocketAddr::V4(addr_v4) = target {
        let first_octet = addr_v4.ip().octets()[0];
        if (224..=239).contains(&first_octet) {
            sock.set_multicast_ttl_v4(mcast_ttl)?;
            // sock.set_multicast_if_v4(&Ipv4Addr::UNSPECIFIED)?; // si besoin d'une interface spécifique
        }
    }

    Ok(sock)
}

/// Read CSV Timestamp,ChannelValue → Vec<Row>
/// Timestamp parsing rules:
/// - If numeric: auto-scale → ns since today
fn read_csv(path: &PathBuf) -> Result<Vec<Row>> {
    let file = File::open(path)?;
    let mut rdr = ReaderBuilder::new()
        .has_headers(true)
        .trim(csv::Trim::All)
        .from_reader(BufReader::new(file));

    let mut out = Vec::with_capacity(1 << 14);
    for rec in rdr.records() {
        let rec = rec?;
        let ts_str = rec
            .get(0)
            .context("missing 'Timestamp' (col 0)")?
            .trim();
        let val_str = rec
            .get(1)
            .context("missing 'ChannelValue' (col 1)")?
            .trim();

        let ts_ns = parse_timestamp_to_ns(ts_str)
            .with_context(|| format!("bad Timestamp '{}'", ts_str))?;
        let value: f64 = val_str
            .parse()
            .with_context(|| format!("bad ChannelValue '{}'", val_str))?;

        out.push(Row { ts_ns, value });
    }
    Ok(out)
}

/// Minimal timestamp parser: expect integer nanoseconds
fn parse_timestamp_to_ns(s: &str) -> Result<u128> {
    let val = s.trim().parse::<u128>()
        .context("invalid nanosecond timestamp")?;
    anyhow::ensure!(val > 1_000_000_000, "timestamp too small, expected ns");
    Ok(val)
}

/// Minimal wire-encoding:
/// [len_channel:u16][channel][ts_csv:u128 LE][value:f64 LE]
fn encode_frame(channel: &str, ts_csv_ns: u128, value: f64) -> Vec<u8> {
    let name = channel.as_bytes();
    let mut buf = Vec::with_capacity(2 + name.len() + 16 + 8);
    let len = u16::try_from(name.len()).unwrap_or(u16::MAX);
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(name);
    buf.extend_from_slice(&ts_csv_ns.to_le_bytes());
    buf.extend_from_slice(&value.to_le_bytes());
    buf
}