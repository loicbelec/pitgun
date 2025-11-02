use crate::EventBatch;
use async_trait::async_trait;
use futures_core::Stream;

#[derive(Clone, Debug, Default)]
pub struct SourceConfig {
    pub channels: Option<Vec<String>>,
    pub batch_max_len: usize,
    pub batch_max_ns:  u64,
}

#[async_trait]
pub trait Source {
    type Error: Send + Sync + 'static;
    async fn stream(&self, cfg: SourceConfig)
      -> Result<Box<dyn Stream<Item = Result<EventBatch, Self::Error>> + Unpin + Send>, Self::Error>;
}

#[async_trait]
pub trait Sink {
    type Error: Send + Sync + 'static;
    async fn write(&self, batch: EventBatch) -> Result<(), Self::Error>;
}