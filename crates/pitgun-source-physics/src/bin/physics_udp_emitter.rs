use anyhow::{Context, Result};
use clap::Parser;
use pitgun_codec_udp::encode_pitgun_v1;
use pitgun_contract::game::v1::GameSimulationRequestV1;
use pitgun_source_physics::game::events::telemetry_point_to_events;
use pitgun_source_physics::game::{simulate_request_with_registry, TrackRegistry};
use socket2::{Domain, Protocol, Socket, Type};
use std::fs;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};
use std::path::PathBuf;
use std::thread::sleep;
use std::time::{Duration, Instant};

/// UDP emitter that runs the physics engine with a GameSimulationRequestV1.
#[derive(Parser, Debug)]
#[command(
    name = "pitgun-physics-udp-emitter",
    version,
    about = "Run physics simulation and emit telemetry over UDP"
)]
struct Args {
    /// Target address, e.g. 239.10.0.1:5001 (multicast) or 127.0.0.1:5001 (unicast)
    #[arg(long, value_name = "HOST:PORT")]
    target: String,

    /// Path to GameSimulationRequestV1 JSON
    #[arg(long, value_name = "PATH")]
    request_json: PathBuf,

    /// Optional track CSV to register for the request track_id
    #[arg(long)]
    track_csv: Option<PathBuf>,

    /// Optional track id when registering --track-csv
    #[arg(long)]
    track_id: Option<String>,

    /// Respect telemetry timing (pacing based on time_s). If not set, emit as fast as possible.
    #[arg(long, default_value_t = false)]
    pace: bool,

    /// Multicast TTL (only used for multicast targets)
    #[arg(long, default_value_t = 1)]
    mcast_ttl: u32,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let raw = fs::read_to_string(&args.request_json)
        .with_context(|| format!("reading request JSON at {:?}", args.request_json))?;
    let request: GameSimulationRequestV1 = serde_json::from_str(&raw).with_context(|| {
        format!(
            "invalid GameSimulationRequestV1 JSON at {:?}",
            args.request_json
        )
    })?;

    let mut registry = TrackRegistry::default();
    if let Some(track_csv) = args.track_csv {
        let track_id = args
            .track_id
            .as_deref()
            .unwrap_or(request.track_id.as_str())
            .to_string();
        registry.insert_path(track_id, track_csv);
    }

    let result = simulate_request_with_registry(&request, &registry)?;
    let telemetry = result.telemetry.unwrap_or_default();

    let target = resolve_target(&args.target)?;
    let sock = make_udp_socket(&target, args.mcast_ttl)?;
    let std_sock: std::net::UdpSocket = sock.into();

    eprintln!(
        "Emitting {} telemetry points to {} (pace={})",
        telemetry.len(),
        target,
        args.pace
    );

    let start = Instant::now();
    let mut sent: usize = 0;

    for point in telemetry {
        if args.pace {
            pace_realtime(point.time_s, start);
        }

        let ts_ns = (point.time_s * 1_000_000_000.0) as u64;
        for event in telemetry_point_to_events(&point, ts_ns) {
            let frame = encode_pitgun_v1(&event.channel, ts_ns as u128, event.value);
            std_sock.send(&frame)?;
            sent += 1;
        }
    }

    eprintln!("Done. total_sent={}", sent);
    Ok(())
}

/// Resolve HOST:PORT to SocketAddr
fn resolve_target(s: &str) -> Result<SocketAddr> {
    let mut iter = s
        .to_socket_addrs()
        .with_context(|| format!("invalid target '{s}'"))?;
    iter.next().context("could not resolve target")
}

/// Create a UDP socket configured for unicast or multicast and connect it
pub fn make_udp_socket(target: &SocketAddr, mcast_ttl: u32) -> Result<Socket> {
    let domain = match target {
        SocketAddr::V4(_) => Domain::IPV4,
        SocketAddr::V6(_) => Domain::IPV6,
    };
    let sock = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP))?;

    let _ = sock.set_reuse_address(true);

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

    if let SocketAddr::V4(addr_v4) = target {
        let first_octet = addr_v4.ip().octets()[0];
        if (224..=239).contains(&first_octet) {
            sock.set_multicast_loop_v4(false)?;
            sock.set_multicast_ttl_v4(mcast_ttl)?;
        }
    }

    sock.connect(&(*target).into())?;
    Ok(sock)
}

/// Sleep until simulated time catches up (1x speed)
fn pace_realtime(time_s: f32, start: Instant) {
    let due = Duration::from_secs_f32(time_s);
    if let Some(rem) = due.checked_sub(start.elapsed()) {
        sleep(rem);
    }
}
