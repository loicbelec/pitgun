#[cfg(feature = "protobuf")]
use pitgun_core::Event;
use pitgun_core::EventBatch;
use thiserror::Error;

#[cfg(feature = "protobuf")]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/telemetry.rs"));
}

#[derive(Debug, Error)]
#[cfg_attr(feature = "protobuf", allow(dead_code))]
pub enum ProtoDecodeError {
    #[error("protobuf support not enabled")]
    NotEnabled,
    #[cfg(feature = "protobuf")]
    #[error("failed to decode protobuf payload: {0}")]
    Decode(#[from] prost::DecodeError),
}

pub fn decode_event_batch(bytes: &[u8]) -> Result<EventBatch, ProtoDecodeError> {
    #[cfg(feature = "protobuf")]
    {
        use prost::Message;

        let batch = generated::EventBatch::decode(bytes)?;
        let events = batch
            .events
            .into_iter()
            .map(|event| Event {
                channel: event.channel,
                ts_ns: event.ts_ns,
                value: event.value,
            })
            .collect();

        Ok(EventBatch {
            events,
            aggregates: Vec::new(),
            end_of_stream: batch.end_of_stream,
        })
    }

    #[cfg(not(feature = "protobuf"))]
    {
        let _ = bytes;
        Err(ProtoDecodeError::NotEnabled)
    }
}
