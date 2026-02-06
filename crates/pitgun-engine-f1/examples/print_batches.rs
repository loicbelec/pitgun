use pitgun_core::Source;
use pitgun_engine_f1::{PhysicsSource, PhysicsSourceConfig};

fn main() {
    let config = PhysicsSourceConfig::default();
    let mut source = PhysicsSource::new(config);

    while let Some(batch) = source.next_batch() {
        println!("{:?}", batch);
        if batch.end_of_stream {
            break;
        }
    }
}
