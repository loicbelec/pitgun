//! Domain-neutral contracts for deterministic incremental execution.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, de};

use crate::run::{ArtifactIdentity, ExecutionId, LogicalClockV1};

const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;

/// Maximum number of ordered records carried by one V1 transport-neutral batch.
pub const MAX_INCREMENTAL_STREAM_BATCH_RECORDS: usize = 256;

/// Exact wire semantics of the deterministic incremental execution stream.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum IncrementalExecutionStreamVersion {
    /// First transport-neutral incremental execution semantics.
    #[serde(rename = "pitgun.incremental-execution-stream/v1")]
    V1,
}

/// Immutable facts shared by every batch of one concrete execution stream.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IncrementalExecutionStreamDescriptorV1 {
    schema_version: IncrementalExecutionStreamVersion,
    execution_id: ExecutionId,
    model: ArtifactIdentity,
    clock: LogicalClockV1,
}

impl IncrementalExecutionStreamDescriptorV1 {
    /// Creates the immutable descriptor before the first progress record.
    #[must_use]
    pub const fn new(
        execution_id: ExecutionId,
        model: ArtifactIdentity,
        clock: LogicalClockV1,
    ) -> Self {
        Self {
            schema_version: IncrementalExecutionStreamVersion::V1,
            execution_id,
            model,
            clock,
        }
    }

    /// Returns the exact descriptor and batch wire semantics.
    #[must_use]
    pub const fn schema_version(&self) -> IncrementalExecutionStreamVersion {
        self.schema_version
    }

    /// Returns the concrete attempt identity known before final completion.
    #[must_use]
    pub const fn execution_id(&self) -> ExecutionId {
        self.execution_id
    }

    /// Returns the exact resolved model identity used by this execution.
    #[must_use]
    pub const fn model(&self) -> &ArtifactIdentity {
        &self.model
    }

    /// Returns the rational logical clock used to interpret record ticks.
    #[must_use]
    pub const fn clock(&self) -> &LogicalClockV1 {
        &self.clock
    }
}

/// Domain-owned payload carried by one ordered stream record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    deny_unknown_fields,
    tag = "kind",
    content = "payload",
    rename_all = "snake_case",
    bound(
        serialize = "Progress: Serialize, Completion: Serialize",
        deserialize = "Progress: Deserialize<'de>, Completion: Deserialize<'de>"
    )
)]
pub enum IncrementalExecutionStreamEventV1<Progress, Completion> {
    /// Non-terminal workload progress.
    Progress(Progress),
    /// The unique terminal workload result.
    Complete(Completion),
}

impl<Progress, Completion> IncrementalExecutionStreamEventV1<Progress, Completion> {
    /// Returns whether this is the unique terminal record.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        matches!(self, Self::Complete(_))
    }
}

/// One deterministic record independent of transport or rendering cadence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(bound(serialize = "Progress: Serialize, Completion: Serialize"))]
pub struct IncrementalExecutionStreamRecordV1<Progress, Completion> {
    sequence: u64,
    logical_tick: u64,
    event: IncrementalExecutionStreamEventV1<Progress, Completion>,
}

#[derive(Deserialize)]
#[serde(
    deny_unknown_fields,
    bound(deserialize = "Progress: Deserialize<'de>, Completion: Deserialize<'de>")
)]
struct IncrementalExecutionStreamRecordWire<Progress, Completion> {
    sequence: u64,
    logical_tick: u64,
    event: IncrementalExecutionStreamEventV1<Progress, Completion>,
}

impl<Progress, Completion> IncrementalExecutionStreamRecordV1<Progress, Completion> {
    /// Creates one record after validating its portable integer fields.
    pub fn new(
        sequence: u64,
        logical_tick: u64,
        event: IncrementalExecutionStreamEventV1<Progress, Completion>,
    ) -> Result<Self, IncrementalExecutionStreamError> {
        validate_portable_integer("sequence", sequence)?;
        validate_portable_integer("logical_tick", logical_tick)?;
        Ok(Self {
            sequence,
            logical_tick,
            event,
        })
    }

    /// Returns the zero-based total-order sequence.
    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Returns the integer tick on the descriptor's logical clock.
    #[must_use]
    pub const fn logical_tick(&self) -> u64 {
        self.logical_tick
    }

    /// Returns the domain-owned progress or completion payload.
    #[must_use]
    pub const fn event(&self) -> &IncrementalExecutionStreamEventV1<Progress, Completion> {
        &self.event
    }
}

impl<'de, Progress, Completion> Deserialize<'de>
    for IncrementalExecutionStreamRecordV1<Progress, Completion>
where
    Progress: Deserialize<'de>,
    Completion: Deserialize<'de>,
{
    fn deserialize<DeserializerT>(deserializer: DeserializerT) -> Result<Self, DeserializerT::Error>
    where
        DeserializerT: Deserializer<'de>,
    {
        let wire = IncrementalExecutionStreamRecordWire::deserialize(deserializer)?;
        Self::new(wire.sequence, wire.logical_tick, wire.event).map_err(de::Error::custom)
    }
}

/// One bounded non-empty group of deterministic records.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(bound(serialize = "Progress: Serialize, Completion: Serialize"))]
pub struct IncrementalExecutionStreamBatchV1<Progress, Completion> {
    schema_version: IncrementalExecutionStreamVersion,
    records: Vec<IncrementalExecutionStreamRecordV1<Progress, Completion>>,
}

#[derive(Deserialize)]
#[serde(
    deny_unknown_fields,
    bound(deserialize = "Progress: Deserialize<'de>, Completion: Deserialize<'de>")
)]
struct IncrementalExecutionStreamBatchWire<Progress, Completion> {
    schema_version: IncrementalExecutionStreamVersion,
    records: Vec<IncrementalExecutionStreamRecordV1<Progress, Completion>>,
}

impl<Progress, Completion> IncrementalExecutionStreamBatchV1<Progress, Completion> {
    /// Builds a batch whose records are already in their deterministic order.
    pub fn new(
        records: Vec<IncrementalExecutionStreamRecordV1<Progress, Completion>>,
    ) -> Result<Self, IncrementalExecutionStreamError> {
        validate_batch_structure(&records)?;
        Ok(Self {
            schema_version: IncrementalExecutionStreamVersion::V1,
            records,
        })
    }

    /// Returns the exact descriptor and batch wire semantics.
    #[must_use]
    pub const fn schema_version(&self) -> IncrementalExecutionStreamVersion {
        self.schema_version
    }

    /// Returns the ordered records without copying their payloads.
    #[must_use]
    pub fn records(&self) -> &[IncrementalExecutionStreamRecordV1<Progress, Completion>] {
        &self.records
    }

    /// Consumes the batch and returns its ordered records.
    #[must_use]
    pub fn into_records(self) -> Vec<IncrementalExecutionStreamRecordV1<Progress, Completion>> {
        self.records
    }
}

impl<'de, Progress, Completion> Deserialize<'de>
    for IncrementalExecutionStreamBatchV1<Progress, Completion>
where
    Progress: Deserialize<'de>,
    Completion: Deserialize<'de>,
{
    fn deserialize<DeserializerT>(deserializer: DeserializerT) -> Result<Self, DeserializerT::Error>
    where
        DeserializerT: Deserializer<'de>,
    {
        let wire = IncrementalExecutionStreamBatchWire::deserialize(deserializer)?;
        let batch = Self::new(wire.records).map_err(de::Error::custom)?;
        debug_assert_eq!(wire.schema_version, batch.schema_version);
        Ok(batch)
    }
}

/// Stateful validator for one sequence of independently transported batches.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IncrementalExecutionStreamCursorV1 {
    next_sequence: u64,
    last_logical_tick: Option<u64>,
    completed: bool,
}

impl Default for IncrementalExecutionStreamCursorV1 {
    fn default() -> Self {
        Self::new()
    }
}

impl IncrementalExecutionStreamCursorV1 {
    /// Starts before sequence zero with no observed logical tick.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            next_sequence: 0,
            last_logical_tick: None,
            completed: false,
        }
    }

    /// Validates one complete batch and advances only after every record passes.
    pub fn validate_next<Progress, Completion>(
        &mut self,
        batch: &IncrementalExecutionStreamBatchV1<Progress, Completion>,
    ) -> Result<(), IncrementalExecutionStreamError> {
        if self.completed {
            return Err(IncrementalExecutionStreamError::BatchAfterCompletion);
        }
        validate_batch_structure(&batch.records)?;

        let first = batch
            .records
            .first()
            .expect("validated non-empty stream batch");
        if first.sequence != self.next_sequence {
            return Err(IncrementalExecutionStreamError::UnexpectedSequence {
                expected: self.next_sequence,
                actual: first.sequence,
            });
        }
        if self
            .last_logical_tick
            .is_some_and(|previous| first.logical_tick < previous)
        {
            return Err(IncrementalExecutionStreamError::DecreasingLogicalTick {
                previous: self.last_logical_tick.expect("checked Some"),
                actual: first.logical_tick,
            });
        }

        let last = batch
            .records
            .last()
            .expect("validated non-empty stream batch");
        let completed = last.event.is_complete();
        let next_sequence = if completed {
            last.sequence
        } else {
            let next = last
                .sequence
                .checked_add(1)
                .ok_or(IncrementalExecutionStreamError::SequenceExhausted)?;
            validate_portable_integer("sequence", next)?;
            next
        };

        self.next_sequence = next_sequence;
        self.last_logical_tick = Some(last.logical_tick);
        self.completed = completed;
        Ok(())
    }

    /// Returns the sequence expected at the start of the next batch while incomplete.
    #[must_use]
    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    /// Returns the last accepted logical tick.
    #[must_use]
    pub const fn last_logical_tick(&self) -> Option<u64> {
        self.last_logical_tick
    }

    /// Returns whether the unique terminal completion has been accepted.
    #[must_use]
    pub const fn is_completed(&self) -> bool {
        self.completed
    }
}

/// Structural failure in an incremental execution stream.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IncrementalExecutionStreamError {
    /// Transport batches cannot be empty.
    EmptyBatch,
    /// One transport batch exceeded the fixed V1 bound.
    BatchTooLarge { actual: usize, maximum: usize },
    /// A record integer cannot be represented exactly by the V1 I-JSON profile.
    UnsafeInteger { field: &'static str, value: u64 },
    /// Adjacent records or adjacent batches did not use contiguous sequences.
    UnexpectedSequence { expected: u64, actual: u64 },
    /// Logical time moved backwards.
    DecreasingLogicalTick { previous: u64, actual: u64 },
    /// A completion appeared before the last record of its batch.
    CompletionNotLast { sequence: u64 },
    /// A batch was supplied after the unique terminal completion.
    BatchAfterCompletion,
    /// A non-terminal record consumed the last portable sequence value.
    SequenceExhausted,
}

impl fmt::Display for IncrementalExecutionStreamError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyBatch => {
                formatter.write_str("incremental execution batches cannot be empty")
            }
            Self::BatchTooLarge { actual, maximum } => write!(
                formatter,
                "incremental execution batch contains {actual} records; maximum is {maximum}"
            ),
            Self::UnsafeInteger { field, value } => write!(
                formatter,
                "incremental execution {field} is outside the exact I-JSON range: {value}"
            ),
            Self::UnexpectedSequence { expected, actual } => write!(
                formatter,
                "incremental execution expected sequence {expected}, got {actual}"
            ),
            Self::DecreasingLogicalTick { previous, actual } => write!(
                formatter,
                "incremental execution logical tick decreased from {previous} to {actual}"
            ),
            Self::CompletionNotLast { sequence } => write!(
                formatter,
                "incremental execution completion at sequence {sequence} must be the last record"
            ),
            Self::BatchAfterCompletion => {
                formatter.write_str("incremental execution cannot accept a batch after completion")
            }
            Self::SequenceExhausted => {
                formatter.write_str("incremental execution exhausted the portable sequence range")
            }
        }
    }
}

impl std::error::Error for IncrementalExecutionStreamError {}

fn validate_batch_structure<Progress, Completion>(
    records: &[IncrementalExecutionStreamRecordV1<Progress, Completion>],
) -> Result<(), IncrementalExecutionStreamError> {
    if records.is_empty() {
        return Err(IncrementalExecutionStreamError::EmptyBatch);
    }
    if records.len() > MAX_INCREMENTAL_STREAM_BATCH_RECORDS {
        return Err(IncrementalExecutionStreamError::BatchTooLarge {
            actual: records.len(),
            maximum: MAX_INCREMENTAL_STREAM_BATCH_RECORDS,
        });
    }

    for record in records {
        validate_portable_integer("sequence", record.sequence)?;
        validate_portable_integer("logical_tick", record.logical_tick)?;
    }
    for pair in records.windows(2) {
        let expected = pair[0]
            .sequence
            .checked_add(1)
            .ok_or(IncrementalExecutionStreamError::SequenceExhausted)?;
        if pair[1].sequence != expected {
            return Err(IncrementalExecutionStreamError::UnexpectedSequence {
                expected,
                actual: pair[1].sequence,
            });
        }
        if pair[1].logical_tick < pair[0].logical_tick {
            return Err(IncrementalExecutionStreamError::DecreasingLogicalTick {
                previous: pair[0].logical_tick,
                actual: pair[1].logical_tick,
            });
        }
        if pair[0].event.is_complete() {
            return Err(IncrementalExecutionStreamError::CompletionNotLast {
                sequence: pair[0].sequence,
            });
        }
    }
    Ok(())
}

fn validate_portable_integer(
    field: &'static str,
    value: u64,
) -> Result<(), IncrementalExecutionStreamError> {
    if value <= MAX_SAFE_JSON_INTEGER {
        Ok(())
    } else {
        Err(IncrementalExecutionStreamError::UnsafeInteger { field, value })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        IncrementalExecutionStreamBatchV1, IncrementalExecutionStreamCursorV1,
        IncrementalExecutionStreamError, IncrementalExecutionStreamEventV1,
        IncrementalExecutionStreamRecordV1, MAX_INCREMENTAL_STREAM_BATCH_RECORDS,
        MAX_SAFE_JSON_INTEGER,
    };

    type Record = IncrementalExecutionStreamRecordV1<String, String>;
    type Batch = IncrementalExecutionStreamBatchV1<String, String>;

    fn progress(sequence: u64, tick: u64) -> Record {
        Record::new(
            sequence,
            tick,
            IncrementalExecutionStreamEventV1::Progress(format!("p{sequence}")),
        )
        .expect("progress record")
    }

    fn completion(sequence: u64, tick: u64) -> Record {
        Record::new(
            sequence,
            tick,
            IncrementalExecutionStreamEventV1::Complete("done".to_string()),
        )
        .expect("completion record")
    }

    #[test]
    fn cursor_accepts_contiguous_batches_and_one_completion() {
        let first = Batch::new(vec![progress(0, 0), progress(1, 4)]).unwrap();
        let last = Batch::new(vec![progress(2, 4), completion(3, 8)]).unwrap();
        let mut cursor = IncrementalExecutionStreamCursorV1::new();

        cursor.validate_next(&first).unwrap();
        assert_eq!(cursor.next_sequence(), 2);
        assert_eq!(cursor.last_logical_tick(), Some(4));
        assert!(!cursor.is_completed());
        cursor.validate_next(&last).unwrap();
        assert!(cursor.is_completed());
        assert_eq!(
            cursor.validate_next(&last),
            Err(IncrementalExecutionStreamError::BatchAfterCompletion)
        );
    }

    #[test]
    fn batches_reject_gaps_time_regressions_and_early_completion() {
        assert!(matches!(
            Batch::new(vec![progress(4, 0), progress(6, 1)]),
            Err(IncrementalExecutionStreamError::UnexpectedSequence { .. })
        ));
        assert!(matches!(
            Batch::new(vec![progress(4, 2), progress(5, 1)]),
            Err(IncrementalExecutionStreamError::DecreasingLogicalTick { .. })
        ));
        assert_eq!(
            Batch::new(vec![completion(0, 0), progress(1, 1)]),
            Err(IncrementalExecutionStreamError::CompletionNotLast { sequence: 0 })
        );
    }

    #[test]
    fn cursor_validates_batch_boundaries_transactionally() {
        let first = Batch::new(vec![progress(0, 5)]).unwrap();
        let gap = Batch::new(vec![progress(2, 6)]).unwrap();
        let regression = Batch::new(vec![progress(1, 4)]).unwrap();
        let mut cursor = IncrementalExecutionStreamCursorV1::new();

        cursor.validate_next(&first).unwrap();
        assert!(matches!(
            cursor.validate_next(&gap),
            Err(IncrementalExecutionStreamError::UnexpectedSequence { .. })
        ));
        assert_eq!(cursor.next_sequence(), 1);
        assert!(matches!(
            cursor.validate_next(&regression),
            Err(IncrementalExecutionStreamError::DecreasingLogicalTick { .. })
        ));
        assert_eq!(cursor.next_sequence(), 1);
        assert_eq!(cursor.last_logical_tick(), Some(5));
    }

    #[test]
    fn wire_validation_is_strict_and_bounded() {
        let empty = json!({
            "schema_version": "pitgun.incremental-execution-stream/v1",
            "records": []
        });
        let unsafe_record = json!({
            "sequence": MAX_SAFE_JSON_INTEGER + 1,
            "logical_tick": 0,
            "event": {"kind": "progress", "payload": "unsafe"}
        });
        let mut unknown = serde_json::to_value(progress(0, 0)).unwrap();
        unknown["rendered_at_ms"] = json!(12);
        let mut unknown_event = serde_json::to_value(progress(0, 0)).unwrap();
        unknown_event["event"]["rendered_at_ms"] = json!(12);

        assert!(serde_json::from_value::<Batch>(empty).is_err());
        assert!(serde_json::from_value::<Record>(unsafe_record).is_err());
        assert!(serde_json::from_value::<Record>(unknown).is_err());
        assert!(serde_json::from_value::<Record>(unknown_event).is_err());
        assert!(matches!(
            Batch::new(
                (0..=MAX_INCREMENTAL_STREAM_BATCH_RECORDS)
                    .map(|sequence| progress(sequence as u64, 0))
                    .collect()
            ),
            Err(IncrementalExecutionStreamError::BatchTooLarge { .. })
        ));
    }
}
