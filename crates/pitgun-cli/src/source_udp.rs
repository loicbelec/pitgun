use anyhow::Result;
use async_trait::async_trait;
use futures_core::Stream; // <- vient de futures-core
use pitgun_core::{Source, SourceConfig, EventBatch, Event, Telemetry, Quality, SessionMeta};
use tokio::{net::UdpSocket, sync::mpsc};
use std::{
    net::{SocketAddr, Ipv4Addr},
    pin::Pin,
    task::{Context, Poll},
};

pub struct UdpSource {
    bind: SocketAddr,
    mcast: Option<Ipv4Addr>,
    iface: Ipv4Addr,
}

impl UdpSource {
    pub fn new(bind: SocketAddr, mcast: Option<Ipv4Addr>, iface: Ipv4Addr) -> Self {
        Self { bind, mcast, iface }
    }
}

#[async_trait]
impl Source for UdpSource {
    type Error = anyhow::Error;

    async fn stream(&self, cfg: SourceConfig)
      -> Result<Box<dyn Stream<Item = Result<EventBatch, Self::Error>> + Unpin + Send>, Self::Error>
    {
        // bind + join multicast éventuel sur socket std, puis passe en tokio
        let std_sock = std::net::UdpSocket::bind(self.bind)?;
        std_sock.set_nonblocking(true)?;
        if let Some(group) = self.mcast {
            std_sock.join_multicast_v4(&group, &self.iface)?;
        }
        let sock = UdpSocket::from_std(std_sock)?;

        let (tx, rx) = mpsc::channel::<Result<EventBatch, anyhow::Error>>(1024);

        let meta = SessionMeta {
            run_id: "run-udp".into(),
            car_id: "car-A".into(),
            track:  "sim".into(),
            season: 2025,
            rda_filtered: false,
        };

        tokio::spawn(async move {
            let mut buf = vec![0u8; 64 * 1024];
            let mut pending: Vec<Event> = Vec::with_capacity(256);
            let mut last_flush = tokio::time::Instant::now();

            loop {
                let n = match sock.recv_from(&mut buf).await {
                    Ok((n, _)) => n,
                    Err(e) => { let _ = tx.send(Err(e.into())).await; break; }
                };
                if n < 2 + 16 + 8 { continue; }

                match decode_frame(&buf[..n]) {
                    Ok((channel, ts_ns, value)) => {
                        if let Some(filter) = &cfg.channels {
                            if !filter.iter().any(|c| c == &channel) { continue; }
                        }
                        pending.push(Event::Telemetry(Telemetry {
                            channel, ts_ns, value, quality: Quality::RAW
                        }));
                    }
                    Err(e) => { let _ = tx.send(Err(e)).await; continue; }
                }

                let due_len  = cfg.batch_max_len > 0 && pending.len() >= cfg.batch_max_len;
                let due_time = cfg.batch_max_ns > 0
                    && last_flush.elapsed().as_nanos() as u64 >= cfg.batch_max_ns;

                if due_len || due_time {
                    let batch = EventBatch { meta: meta.clone(), events: std::mem::take(&mut pending) };
                    if tx.send(Ok(batch)).await.is_err() { break; }
                    last_flush = tokio::time::Instant::now();
                }
            }
        });

        Ok(Box::new(ReceiverStream(rx)))
    }
}

// Decode wire-format: [len:u16][channel][ts:u128 LE][val:f64 LE]
fn decode_frame(mut bytes: &[u8]) -> Result<(String, u128, f64), anyhow::Error> {
    use std::convert::TryInto;
    if bytes.len() < 2 { anyhow::bail!("short frame (len)"); }
    let len = u16::from_le_bytes(bytes[0..2].try_into().unwrap()) as usize;
    bytes = &bytes[2..];
    if bytes.len() < len { anyhow::bail!("short frame (channel)"); }
    let channel = std::str::from_utf8(&bytes[..len])?.to_string();
    bytes = &bytes[len..];
    if bytes.len() < 16 { anyhow::bail!("short frame (ts)"); }
    let mut t = [0u8; 16]; t.copy_from_slice(&bytes[..16]);
    let ts_ns = u128::from_le_bytes(t);
    bytes = &bytes[16..];
    if bytes.len() < 8 { anyhow::bail!("short frame (val)"); }
    let mut v = [0u8; 8]; v.copy_from_slice(&bytes[..8]);
    let value = f64::from_le_bytes(v);
    Ok((channel, ts_ns, value))
}

// mpsc::Receiver -> impl Stream (corrige l'ambiguïté Self::Item)
struct ReceiverStream<T>(tokio::sync::mpsc::Receiver<T>);
impl<T> Stream for ReceiverStream<T> {
    type Item = T;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        Pin::new(&mut self.get_mut().0).poll_recv(cx)
    }
}