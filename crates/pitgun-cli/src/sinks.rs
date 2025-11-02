use anyhow::Result;
use async_trait::async_trait;
use pitgun_core::{Sink, EventBatch, Event};
use std::{collections::HashMap, path::PathBuf, sync::Mutex, time::Instant};

pub struct JsonSink;
#[async_trait]
impl Sink for JsonSink {
    type Error = anyhow::Error;
    async fn write(&self, batch: EventBatch) -> Result<()> {
        for e in &batch.events {
            if let Event::Telemetry(t) = e {
                println!(r#"{{"channel":"{}","ts_ns":{},"value":{}}}"#, t.channel, t.ts_ns, t.value);
            }
        }
        Ok(())
    }
}

pub struct CsvSink {
    dir: PathBuf,
    writers: Mutex<HashMap<String, csv::Writer<std::fs::File>>>,
}
impl CsvSink {
    pub fn new(dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir, writers: Default::default() })
    }
}
#[async_trait]
impl Sink for CsvSink {
    type Error = anyhow::Error;
    async fn write(&self, batch: EventBatch) -> Result<()> {
        let mut guard = self.writers.lock().unwrap();
        for e in &batch.events {
            if let Event::Telemetry(t) = e {
                let w = guard.entry(t.channel.clone()).or_insert_with(|| {
                    let path = self.dir.join(format!("{}.csv", t.channel));
                    let f = std::fs::File::create(path).expect("csv create");
                    let mut w = csv::Writer::from_writer(f);
                    w.write_record(&["Timestamp","ChannelValue"]).ok();
                    w
                });
                w.write_record(&[t.ts_ns.to_string(), t.value.to_string()])?;
            }
        }
        Ok(())
    }
}

pub struct StatsSink {
    every_s: u64,
    start: Instant,
    total: Mutex<u64>,
    per_ch: Mutex<HashMap<String, u64>>,
    last_ts: Mutex<HashMap<String, u128>>,
    gaps: Mutex<u64>,
}
impl StatsSink {
    pub fn new(every_s: u64) -> Self {
        Self {
            every_s,
            start: Instant::now(),
            total: Mutex::new(0),
            per_ch: Mutex::new(HashMap::new()),
            last_ts: Mutex::new(HashMap::new()),
            gaps: Mutex::new(0),
        }
    }
}
#[async_trait]
impl Sink for StatsSink {
    type Error = anyhow::Error;
    async fn write(&self, batch: EventBatch) -> Result<()> {
        let mut n = 0u64;

        // comptage + gaps
        {
            let mut per_ch = self.per_ch.lock().unwrap();
            let mut last_ts = self.last_ts.lock().unwrap();
            let mut gaps = self.gaps.lock().unwrap();

            for e in &batch.events {
                if let Event::Telemetry(t) = e {
                    n += 1;
                    *per_ch.entry(t.channel.clone()).or_default() += 1;
                    if let Some(prev) = last_ts.insert(t.channel.clone(), t.ts_ns) {
                        if t.ts_ns <= prev { *gaps += 1; }
                    }
                }
            }
        }

        // total + print périodique
        {
            let mut total = self.total.lock().unwrap();
            *total += n;
            if self.every_s > 0 && self.start.elapsed().as_secs_f64() >= self.every_s as f64 {
                let rate = *total as f64 / self.start.elapsed().as_secs_f64().max(1e-9);
                let per_ch = self.per_ch.lock().unwrap();
                let gaps = *self.gaps.lock().unwrap();
                eprint!("frames={} rate={:.1} fps gaps={} chans={}", *total, rate, gaps, per_ch.len());
                // top 5
                let mut items: Vec<_> = per_ch.iter().map(|(k,v)| (k.clone(), *v)).collect();
                items.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
                for (ch, c) in items.into_iter().take(5) { eprint!("  {}:{}", ch, c); }
                eprintln!();
            }
        }
        Ok(())
    }
}