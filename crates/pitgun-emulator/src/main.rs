use anyhow::{Context, Result};
use clap::Parser;
use csv::ReaderBuilder;
use pitgun_codec_udp::encode_pitgun_v1;
use serde::Deserialize;
use socket2::{Domain, Protocol, Socket, Type};
use std::fs::File;
use std::io::BufReader;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};
use std::path::PathBuf;
use std::thread::sleep;
use std::time::{Duration, Instant};

/// UDP telemetry emitter reading multiple 2-col CSVs: Timestamp,ChannelValue
#[derive(Parser, Debug)]
#[command(
    name = "pitgun-emulator",
    version,
    about = "Emit telemetry from CSV over UDP"
)]
struct Args {
    /// Target address, e.g. 239.10.0.1:5001 (multicast) or 127.0.0.1:5001 (unicast)
    #[arg(long, value_name = "HOST:PORT")]
    target: String,

    /// Repeatable: CHANNEL=PATH (e.g. --input nEngine=... --input throttle=...)
    #[arg(long, value_parser = parse_input)]
    input: Vec<(String, PathBuf)>,

    /// Respect CSV timing (pacing based on Timestamp deltas). If not set, emit as fast as possible.
    #[arg(long, default_value_t = false)]
    pace: bool,

    /// Multicast TTL (only used for multicast targets)
    #[arg(long, default_value_t = 1)]
    mcast_ttl: u32,
}

fn parse_input(s: &str) -> Result<(String, PathBuf), String> {
    let (ch, p) = s.split_once('=').ok_or("expected CHANNEL=PATH")?;
    Ok((ch.to_string(), PathBuf::from(p)))
}

#[derive(Debug, Deserialize)]
struct Row {
    #[serde(rename = "Timestamp")]
    ts: u128, // ns
    #[serde(rename = "ChannelValue", alias = "Value")]
    val: f64,
}

struct Cursor {
    channel: String,
    it: csv::DeserializeRecordsIntoIter<BufReader<File>, Row>,
    next: Option<Row>,
}

fn open_cursor(channel: String, path: PathBuf) -> anyhow::Result<Cursor> {
    let file = File::open(&path)
        .with_context(|| format!("opening CSV for channel '{}' at {:?}", channel, path))?;
    let rdr = ReaderBuilder::new()
        .has_headers(true)
        .trim(csv::Trim::All)
        .from_reader(BufReader::new(file));
    let mut it = rdr.into_deserialize::<Row>();
    let next = it
        .next()
        .transpose()
        .with_context(|| format!("reading first row of {:?}", path))?;
    Ok(Cursor { channel, it, next })
}

fn main() -> Result<()> {
    let args = Args::parse();
    anyhow::ensure!(
        !args.input.is_empty(),
        "provide at least one --input CHANNEL=PATH"
    );

    // ---- Resolve and prepare UDP socket (already connected)
    let target = resolve_target(&args.target)?;
    let sock = make_udp_socket(&target, args.mcast_ttl)?;
    // Convert to std::net::UdpSocket for send()
    let std_sock: std::net::UdpSocket = sock.into();

    // ---- Open one cursor per channel
    let mut cursors: Vec<Cursor> = args
        .input
        .into_iter()
        .map(|(ch, p)| open_cursor(ch, p))
        .collect::<Result<_, _>>()?;

    // ---- Compute reference t0 (earliest timestamp across files)
    let t0_ns = min_ts_across(&cursors).context("no data in provided CSVs")?;
    let start_monotonic = Instant::now();

    eprintln!(
        "Emitting from {} channel file(s) to {} (pace={})",
        cursors.len(),
        target,
        args.pace
    );

    // ---- K-way merge by timestamp
    let mut sent: usize = 0;
    loop {
        let Some(i) = pick_min_index(&cursors) else {
            break;
        }; // all exhausted
        let (channel, row_ts, row_val) = {
            let c = &mut cursors[i];
            let row = c.next.take().expect("pick_min_index guaranteed Some");
            (c.channel.clone(), row.ts, row.val)
        };

        if args.pace {
            pace_realtime(row_ts, t0_ns, start_monotonic);
        }

        let frame = encode_pitgun_v1(&channel, row_ts, row_val);
        std_sock.send(&frame)?;
        sent += 1;

        // advance cursor i
        cursors[i].next = cursors[i]
            .it
            .next()
            .transpose()
            .with_context(|| format!("reading next row for channel '{}'", channel))?;

        if sent.is_multiple_of(1_000) {
            let rate = (sent as f64 / start_monotonic.elapsed().as_secs_f64().max(1e-6)).round();
            eprintln!("sent={} rate≈{} fps", sent, rate);
        }
    }

    eprintln!("Done. total_sent={}", sent);
    Ok(())
}

/// Resolve HOST:PORT to SocketAddr
fn resolve_target(s: &str) -> Result<SocketAddr> {
    let mut iter = s
        .to_socket_addrs()
        .with_context(|| format!("invalid target '{}'", s))?;
    iter.next().context("could not resolve target")
}

/// Create a UDP socket configured for unicast or multicast and connect it
pub fn make_udp_socket(target: &SocketAddr, mcast_ttl: u32) -> Result<Socket> {
    let domain = match target {
        SocketAddr::V4(_) => Domain::IPV4,
        SocketAddr::V6(_) => Domain::IPV6,
    };
    let sock = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP))?;

    // Useful when multiple emitters run concurrently
    let _ = sock.set_reuse_address(true);
    // let _ = sock.set_reuse_port(true);

    // Bind ephemeral
    match target {
        SocketAddr::V4(_) => {
            let bind_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0);
            sock.bind(&bind_addr.into())?;
        }
        SocketAddr::V6(_) => {
            let bind_addr = SocketAddr::new(Ipv6Addr::UNSPECIFIED.into(), 0);
            sock.bind(&bind_addr.into())?;
        }
    }

    // Multicast tweaks if needed
    if let SocketAddr::V4(addr_v4) = target {
        let first_octet = addr_v4.ip().octets()[0];
        if (224..=239).contains(&first_octet) {
            sock.set_multicast_loop_v4(false)?;
            sock.set_multicast_ttl_v4(mcast_ttl)?;
        }
    }

    // Connect so `send()` has an implicit destination
    sock.connect(&(*target).into())?;
    Ok(sock)
}

/// Pick the index of the cursor with the smallest next timestamp
fn pick_min_index(cursors: &[Cursor]) -> Option<usize> {
    let mut min_i: Option<usize> = None;
    let mut min_ts: u128 = u128::MAX;
    for (i, c) in cursors.iter().enumerate() {
        if let Some(row) = &c.next {
            if row.ts < min_ts {
                min_ts = row.ts;
                min_i = Some(i);
            }
        }
    }
    min_i
}

/// Smallest available timestamp across all cursors
fn min_ts_across(cursors: &[Cursor]) -> Option<u128> {
    cursors
        .iter()
        .filter_map(|c| c.next.as_ref().map(|r| r.ts))
        .min()
}

/// Sleep until simulated time catches up (1x speed)
fn pace_realtime(ts_ns: u128, ts0: u128, t0: Instant) {
    let sim_ns = ts_ns.saturating_sub(ts0);
    let due = Duration::from_nanos(sim_ns as u64);
    if let Some(rem) = due.checked_sub(t0.elapsed()) {
        sleep(rem);
    }
}
