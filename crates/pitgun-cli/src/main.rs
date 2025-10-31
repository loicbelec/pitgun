use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use csv::Writer;
use std::{
    collections::HashMap,
    io,
    net::{Ipv4Addr, SocketAddr, UdpSocket},
    path::PathBuf,
    str,
    time::{Duration, Instant},
};

/// pitgun-cli: tools for receiving and inspecting Pitgun telemetry
#[derive(Parser, Debug)]
#[command(name = "pitgun-cli", version, about = "Pitgun CLI tools")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Subscribe to UDP telemetry and print/record metrics
    Subscribe(SubscribeArgs),
}

#[derive(clap::Args, Debug)]
struct SubscribeArgs {
    /// Local bind address (e.g. 127.0.0.1:5001 or 0.0.0.0:5001 for multicast)
    #[arg(long, value_name = "HOST:PORT")]
    bind: String,

    /// Multicast group (IPv4 only), e.g. 239.10.0.1
    #[arg(long)]
    mcast: Option<Ipv4Addr>,

    /// Interface IPv4 for multicast join (e.g. 0.0.0.0 or your NIC address)
    #[arg(long, default_value = "0.0.0.0")]
    iface: Ipv4Addr,

    /// Stats print interval in seconds (0 = disabled)
    #[arg(long, default_value_t = 1)]
    stats_interval: u64,

    /// Optional directory to write per-channel CSV recording
    #[arg(long)]
    write_csv: Option<PathBuf>,

    /// Print each frame as JSON (noisy)
    #[arg(long, default_value_t = false)]
    json: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Subscribe(args) => run_subscribe(args),
    }
}

fn run_subscribe(args: SubscribeArgs) -> Result<()> {
    // ---- Bind socket
    let bind: SocketAddr = args
        .bind
        .parse()
        .with_context(|| format!("invalid --bind '{}'", args.bind))?;
    let sock = UdpSocket::bind(bind).with_context(|| format!("bind failed on {}", bind))?;

    // ---- Multicast (optional)
    if let Some(group) = args.mcast {
        // Join multicast on given interface (IPv4)
        sock.join_multicast_v4(&group, &args.iface)
            .with_context(|| format!("failed to join multicast {} on iface {}", group, args.iface))?;
        // Optional tweaks: disable loopback, set TTL; not strictly needed for receiver
        sock.set_nonblocking(false)?;
        eprintln!("Joined multicast group {} on iface {}", group, args.iface);
    }

    // ---- Recording setup (lazy writers per channel)
    let mut writers: HashMap<String, Writer<std::fs::File>> = HashMap::new();
    let out_dir = args.write_csv.clone();

    // ---- Metrics
    let mut total: u64 = 0;
    let mut per_ch_count: HashMap<String, u64> = HashMap::new();
    let mut per_ch_last_ts: HashMap<String, u128> = HashMap::new();
    let mut gaps: u64 = 0;

    let mut buf = vec![0u8; 64 * 1024];
    let start = Instant::now();
    let mut last_stats = start;

    loop {
        let (n, _src) = sock.recv_from(&mut buf)?;
        if n < 2 + 16 + 8 {
            continue; // too small to be a valid frame
        }

        match decode_frame(&buf[..n]) {
            Ok((channel, ts_ns, value)) => {
                total += 1;
                *per_ch_count.entry(channel.clone()).or_default() += 1;

                // Simple gap check per channel (strictly increasing ts)
                if let Some(prev) = per_ch_last_ts.insert(channel.clone(), ts_ns) {
                    if ts_ns <= prev {
                        gaps += 1;
                    }
                }

                // Optional per-frame print
                if args.json {
                    println!(
                        "{{\"channel\":\"{}\",\"ts_ns\":{},\"value\":{}}}",
                        channel, ts_ns, value
                    );
                }

                // Optional recording
                if let Some(dir) = &out_dir {
                    let w = writers.entry(channel.clone()).or_insert_with(|| {
                        std::fs::create_dir_all(dir).ok();
                        let path = dir.join(format!("{}.csv", channel));
                        let file = std::fs::File::create(&path)
                            .expect("failed to create channel CSV");
                        let mut w = csv::Writer::from_writer(file);
                        // header
                        w.write_record(&["Timestamp", "ChannelValue"]).ok();
                        w
                    });
                    w.write_record(&[ts_ns.to_string(), value.to_string()]).ok();
                }

                // Periodic stats
                if args.stats_interval > 0
                    && last_stats.elapsed() >= Duration::from_secs(args.stats_interval)
                {
                    let elapsed = start.elapsed().as_secs_f64().max(1e-9);
                    let rate = (total as f64 / elapsed);
                    eprintln!(
                        "frames={} rate={:.1} fps gaps={} chans={}",
                        total,
                        rate,
                        gaps,
                        per_ch_count.len()
                    );
                    // short per-channel summary (top 5 by count)
                    let mut items: Vec<_> = per_ch_count.iter().collect();
                    items.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
                    for (ch, c) in items.into_iter().take(5) {
                        eprint!("  {}:{}", ch, c);
                    }
                    eprintln!();
                    last_stats = Instant::now();
                }
            }
            Err(e) => {
                // Corrupt frame (ignore but log occasionally)
                eprintln!("decode error: {e}");
            }
        }
    }
}

/// Decode `[len:u16][channel][ts:u128 LE][val:f64 LE]`
fn decode_frame(mut bytes: &[u8]) -> Result<(String, u128, f64)> {
    use std::convert::TryInto;

    // channel length
    if bytes.len() < 2 {
        return Err(anyhow::anyhow!("short frame (no length)"));
    }
    let len = u16::from_le_bytes(bytes[0..2].try_into().unwrap()) as usize;
    bytes = &bytes[2..];

    // channel string
    if bytes.len() < len {
        return Err(anyhow::anyhow!("short frame (channel)"));
    }
    let channel = str::from_utf8(&bytes[..len]).context("invalid utf8 channel")?.to_string();
    bytes = &bytes[len..];

    // timestamp (u128 LE)
    if bytes.len() < 16 {
        return Err(anyhow::anyhow!("short frame (timestamp)"));
    }
    let mut ts_arr = [0u8; 16];
    ts_arr.copy_from_slice(&bytes[..16]);
    let ts_ns = u128::from_le_bytes(ts_arr);
    bytes = &bytes[16..];

    // value (f64 LE)
    if bytes.len() < 8 {
        return Err(anyhow::anyhow!("short frame (value)"));
    }
    let mut v_arr = [0u8; 8];
    v_arr.copy_from_slice(&bytes[..8]);
    let value = f64::from_le_bytes(v_arr);

    Ok((channel, ts_ns, value))
}