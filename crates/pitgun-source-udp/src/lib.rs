use pitgun_codec_udp::{UdpDecoded, UdpDecoder};
use pitgun_core::{Event, EventBatch, Source};
use std::collections::VecDeque;
use std::io;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::time::Instant;

pub struct UdpSource<D> {
    socket: UdpSocket,
    buf: Vec<u8>,
    pending_events: Vec<Event>,
    pending_batches: VecDeque<EventBatch>,
    batch_max_len: usize,
    batch_max_ns: u64,
    last_flush: Instant,
    decoder: D,
}

impl<D> UdpSource<D>
where
    D: UdpDecoder,
{
    pub fn new(
        bind: SocketAddr,
        mcast: Option<Ipv4Addr>,
        iface: Ipv4Addr,
        batch_max_len: usize,
        batch_max_ns: u64,
        decoder: D,
    ) -> io::Result<Self> {
        let socket = UdpSocket::bind(bind)?;
        socket.set_nonblocking(false)?;
        if let Some(group) = mcast {
            socket.join_multicast_v4(&group, &iface)?;
        }
        Ok(Self {
            socket,
            buf: vec![0u8; 64 * 1024],
            pending_events: Vec::with_capacity(256),
            pending_batches: VecDeque::new(),
            batch_max_len,
            batch_max_ns,
            last_flush: Instant::now(),
            decoder,
        })
    }

    fn should_flush(&self) -> bool {
        let len_due = self.batch_max_len > 0 && self.pending_events.len() >= self.batch_max_len;
        let time_due = self.batch_max_ns > 0
            && self.last_flush.elapsed().as_nanos() as u64 >= self.batch_max_ns;
        len_due || time_due
    }

    fn flush_events(&mut self) -> Option<EventBatch> {
        if self.pending_events.is_empty() {
            return None;
        }
        self.last_flush = Instant::now();
        let events = std::mem::take(&mut self.pending_events);
        Some(EventBatch {
            events,
            aggregates: Vec::new(),
            end_of_stream: false,
        })
    }
}

impl<D> Source for UdpSource<D>
where
    D: UdpDecoder,
{
    fn next_batch(&mut self) -> Option<EventBatch> {
        if let Some(batch) = self.pending_batches.pop_front() {
            return Some(batch);
        }

        loop {
            match self.socket.recv(&mut self.buf) {
                Ok(n) => {
                    if n < self.decoder.min_datagram_len() {
                        continue;
                    }

                    match self.decoder.decode(&self.buf[..n]) {
                        Ok(UdpDecoded::Events(mut events)) => {
                            self.pending_events.append(&mut events);
                        }
                        Ok(UdpDecoded::Batches(batches)) => {
                            self.pending_batches.extend(batches);
                        }
                        Err(err) => {
                            eprintln!("pitgun-core: failed to decode frame: {err}");
                            continue;
                        }
                    }

                    if let Some(batch) = self.pending_batches.pop_front() {
                        return Some(batch);
                    }

                    if self.should_flush()
                        && let Some(batch) = self.flush_events()
                    {
                        return Some(batch);
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
