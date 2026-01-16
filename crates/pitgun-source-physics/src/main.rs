use anyhow::Result;
use clap::Parser;
use pitgun_source_physics::{load_track_from_csv_path, run_simulation, PlayerTuning};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long)]
    track_csv: PathBuf,

    #[arg(long, default_value = "telemetry.csv")]
    out_csv: PathBuf,

    #[arg(long, default_value_t = 60.0)]
    hz: f32,

    #[arg(long, default_value_t = 10)]
    aero: i32,
    #[arg(long, default_value_t = 10)]
    chassis: i32,
    #[arg(long, default_value_t = 10)]
    cooling: i32,
    #[arg(long, default_value_t = 10)]
    engine: i32,
    #[arg(long, default_value_t = 0.5)]
    downforce: f32,
    #[arg(long, default_value_t = 0.5)]
    gear_ratio: f32,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let track = load_track_from_csv_path(&args.track_csv)?;
    let tuning = PlayerTuning {
        aero_points: args.aero,
        chassis_points: args.chassis,
        cooling_points: args.cooling,
        engine_points: args.engine,
        downforce_slider: args.downforce,
        gear_ratio_slider: args.gear_ratio,
    };

    let start = std::time::Instant::now();
    let telemetry = run_simulation(&track, tuning, args.hz)?;
    println!(
        "Simulated {} frames in {:.2?}",
        telemetry.len(),
        start.elapsed()
    );

    let mut wtr = csv::Writer::from_path(&args.out_csv)?;
    for point in telemetry {
        wtr.serialize(point)?;
    }
    wtr.flush()?;
    println!("Wrote to {:?}", args.out_csv);

    Ok(())
}
