use crate::EventBatch;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

pub trait Source {
    fn next_batch(&mut self) -> Option<EventBatch>;
}

pub trait Processor: Send {
    fn process(&mut self, batch: &mut EventBatch);
}

pub trait Sink: Send {
    fn write(&mut self, batch: &EventBatch);
}

pub struct Pipeline<S, K>
where
    S: Source,
    K: Sink,
{
    pub source: S,
    pub processors: Vec<Box<dyn Processor>>,
    pub sink: K,
}

impl<S, K> Pipeline<S, K>
where
    S: Source,
    K: Sink,
{
    pub fn run_once(&mut self) {
        if let Some(mut batch) = self.source.next_batch() {
            for processor in self.processors.iter_mut() {
                processor.process(&mut batch);
            }
            self.sink.write(&batch);
        }
    }
}

pub struct StatsProcessor {
    every_s: u64,
    start: Instant,
    total: u64,
    per_channel: HashMap<String, u64>,
    last_ts: HashMap<String, u64>,
    gaps: u64,
}

impl StatsProcessor {
    pub fn new(every_s: u64) -> Self {
        Self {
            every_s,
            start: Instant::now(),
            total: 0,
            per_channel: HashMap::new(),
            last_ts: HashMap::new(),
            gaps: 0,
        }
    }
}

impl Processor for StatsProcessor {
    fn process(&mut self, batch: &mut EventBatch) {
        if batch.events.is_empty() {
            return;
        }
        let mut batch_total = 0u64;
        for event in &batch.events {
            batch_total += 1;
            *self.per_channel.entry(event.channel.clone()).or_default() += 1;
            #[allow(clippy::collapsible_if)]
            if let Some(prev) = self.last_ts.insert(event.channel.clone(), event.ts_ns) {
                if event.ts_ns <= prev {
                    self.gaps += 1;
                }
            }
        }
        self.total += batch_total;
        if self.every_s > 0 && self.start.elapsed().as_secs_f64() >= self.every_s as f64 {
            let elapsed = self.start.elapsed().as_secs_f64().max(1e-9);
            let rate = self.total as f64 / elapsed;
            eprint!(
                "frames={} rate={:.1} fps gaps={} chans={}",
                self.total,
                rate,
                self.gaps,
                self.per_channel.len()
            );
            let mut items: Vec<_> = self
                .per_channel
                .iter()
                .map(|(ch, count)| (ch.clone(), *count))
                .collect();
            items.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
            for (ch, count) in items.into_iter().take(5) {
                eprint!("  {}:{}", ch, count);
            }
            eprintln!();
        }
    }
}

pub struct ChannelFilterProcessor {
    channels: Option<HashSet<String>>,
}

impl ChannelFilterProcessor {
    pub fn new(channels: Vec<String>) -> Self {
        if channels.is_empty() {
            Self { channels: None }
        } else {
            Self {
                channels: Some(channels.into_iter().collect()),
            }
        }
    }
}

impl Processor for ChannelFilterProcessor {
    fn process(&mut self, batch: &mut EventBatch) {
        if let Some(channels) = &self.channels {
            batch
                .events
                .retain(|event| channels.contains(&event.channel));
        }
    }
}

pub struct ScaleProcessor {
    channel: String,
    factor: f64,
}

impl ScaleProcessor {
    pub fn new(channel: String, factor: f64) -> Self {
        Self { channel, factor }
    }
}

impl Processor for ScaleProcessor {
    fn process(&mut self, batch: &mut EventBatch) {
        for event in &mut batch.events {
            if event.channel == self.channel {
                event.value *= self.factor;
            }
        }
    }
}

pub struct ConsoleSink {
    print_events: bool,
}

impl ConsoleSink {
    pub fn new(print_events: bool) -> Self {
        Self { print_events }
    }
}

impl Sink for ConsoleSink {
    fn write(&mut self, batch: &EventBatch) {
        for aggregate in &batch.aggregates {
            match serde_json::to_string(aggregate) {
                Ok(json) => println!("{json}"),
                Err(err) => eprintln!("pitgun-core: failed to print aggregate: {err}"),
            }
        }

        if !self.print_events {
            return;
        }
        for event in &batch.events {
            println!(
                r#"{{"channel":"{}","ts_ns":{},"value":{}}}"#,
                event.channel, event.ts_ns, event.value
            );
        }
    }
}
