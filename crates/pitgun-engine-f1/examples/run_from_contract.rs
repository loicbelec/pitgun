use pitgun_contract::SignedSimulationContractV1;
use pitgun_core::Source;
use pitgun_engine_f1::{PhysicsSource, PhysicsSourceConfig};
use std::fs::File;
use std::io::BufReader;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = match args.next() {
        Some(path) => path,
        None => {
            eprintln!("usage: run_from_contract <signed_contract.json>");
            std::process::exit(1);
        }
    };

    let file = File::open(&path).unwrap_or_else(|err| {
        eprintln!("failed to open {path}: {err}");
        std::process::exit(1);
    });
    let reader = BufReader::new(file);
    let signed: SignedSimulationContractV1 = serde_json::from_reader(reader).unwrap_or_else(|err| {
        eprintln!("invalid contract JSON: {err}");
        std::process::exit(1);
    });

    let config = PhysicsSourceConfig::from_signed_simulation_contract(&signed).unwrap_or_else(|err| {
        eprintln!("contract error: {err}");
        std::process::exit(1);
    });

    println!("signature_ok=true");
    println!("expired=false");

    let mut source = PhysicsSource::new(config);
    for batch_index in 0..3u32 {
        let Some(batch) = source.next_batch() else {
            println!("batch[{batch_index}]: end of stream");
            break;
        };

        let first_ts = batch.events.first().map(|event| event.ts_ns);
        let last_ts = batch.events.last().map(|event| event.ts_ns);
        println!(
            "batch[{batch_index}]: events={} first_ts={:?} last_ts={:?}",
            batch.events.len(),
            first_ts,
            last_ts
        );

        let speed = find_value(&batch.events, "speed_kph");
        let rpm = find_value(&batch.events, "rpm");
        let instability = find_value(&batch.events, "instability_index");
        println!(
            "  samples: speed_kph={:?} rpm={:?} instability_index={:?}",
            speed, rpm, instability
        );

        if batch.end_of_stream {
            break;
        }
    }
}

fn find_value(events: &[pitgun_core::Event], channel: &str) -> Option<f64> {
    events
        .iter()
        .find(|event| event.channel == channel)
        .map(|event| event.value)
}
