use crate::{Event, EventBatch};
use std::collections::{HashMap, HashSet};
use std::io;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
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

pub struct UdpSource {
    socket: UdpSocket,
    buf: Vec<u8>,
    pending: Vec<Event>,
    batch_max_len: usize,
    batch_max_ns: u64,
    last_flush: Instant,
}

impl UdpSource {
    pub fn new(
        bind: SocketAddr,
        mcast: Option<Ipv4Addr>,
        iface: Ipv4Addr,
        batch_max_len: usize,
        batch_max_ns: u64,
    ) -> io::Result<Self> {
        let socket = UdpSocket::bind(bind)?;
        socket.set_nonblocking(false)?;
        if let Some(group) = mcast {
            socket.join_multicast_v4(&group, &iface)?;
        }
        Ok(Self {
            socket,
            buf: vec![0u8; 64 * 1024],
            pending: Vec::with_capacity(256),
            batch_max_len,
            batch_max_ns,
            last_flush: Instant::now(),
        })
    }

    fn should_flush(&self) -> bool {
        let len_due = self.batch_max_len > 0 && self.pending.len() >= self.batch_max_len;
        let time_due = self.batch_max_ns > 0
            && self.last_flush.elapsed().as_nanos() as u64 >= self.batch_max_ns;
        len_due || time_due
    }
}

impl Source for UdpSource {
    fn next_batch(&mut self) -> Option<EventBatch> {
        loop {
            match self.socket.recv(&mut self.buf) {
                Ok(n) => {
                    if n < 2 + 16 + 8 {
                        continue;
                    }
                    match decode_frame(&self.buf[..n]) {
                        Ok(event) => self.pending.push(event),
                        Err(err) => {
                            eprintln!("pitgun-core: failed to decode frame: {err}");
                            continue;
                        }
                    }

                    if self.should_flush() && !self.pending.is_empty() {
                        self.last_flush = Instant::now();
                        let events = std::mem::take(&mut self.pending);
                        return Some(EventBatch { events });
                    }
                }
                Err(err) => {
                    eprintln!("pitgun-core: UDP receive error: {err}");
                    return None;
                }
            }
        }
    }
}

fn decode_frame(mut bytes: &[u8]) -> io::Result<Event> {
    use std::convert::TryInto;

    if bytes.len() < 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "short frame (len)",
        ));
    }
    let len = u16::from_le_bytes(bytes[0..2].try_into().unwrap()) as usize;
    bytes = &bytes[2..];
    if bytes.len() < len {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "short frame (channel)",
        ));
    }
    let channel = std::str::from_utf8(&bytes[..len])
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid channel utf8"))?
        .to_string();
    bytes = &bytes[len..];
    if bytes.len() < 16 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "short frame (ts)",
        ));
    }
    let mut t = [0u8; 16];
    t.copy_from_slice(&bytes[..16]);
    let ts_raw = u128::from_le_bytes(t);
    bytes = &bytes[16..];
    if bytes.len() < 8 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "short frame (value)",
        ));
    }
    let mut v = [0u8; 8];
    v.copy_from_slice(&bytes[..8]);
    let value = f64::from_le_bytes(v);

    let ts_ns = if ts_raw > u128::from(u64::MAX) {
        u64::MAX
    } else {
        ts_raw as u64
    };

    Ok(Event {
        channel,
        ts_ns,
        value,
    })
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
            if let Some(prev) = 
                self.last_ts.insert(event.channel.clone(), event.ts_ns)
                && event.ts_ns <= prev
            {
                self.gaps += 1;
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
