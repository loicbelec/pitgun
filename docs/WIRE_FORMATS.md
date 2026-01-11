# Pitgun Wire Formats

This document summarizes the stable wire formats used by Pitgun. Identifiers here are
stable IDs and must not be repurposed for different layouts.

## udp/pitgun-v1

**Datagram framing**
- One UDP datagram contains exactly one Pitgun v1 frame.
- There is no multi-frame message, segmentation, or reassembly at this layer.
- Message ordering, loss, and duplication follow UDP semantics.

**Field layout (little-endian)**
```
[len_channel:u16][channel:bytes][ts_ns:u128][value:f64]
```
- `len_channel`: length of `channel` in bytes (u16 LE).
- `channel`: UTF-8 bytes, length `len_channel`.
- `ts_ns`: timestamp in nanoseconds as u128 LE.
- `value`: IEEE-754 f64 LE.

**Invariants**
- `channel` must be valid UTF-8.
- Decoders clamp `ts_ns` to `u64::MAX` if it does not fit in u64.

**Versioning strategy**
- v1 has no in-band version tag; it is selected by configuration/port/flag.
- A future v2 should use a distinct wire ID and an explicit selection mechanism
  (for example a new CLI flag or a dedicated port), leaving v1 unchanged.

## session-envelope-json-v1

**Transport framing**
- One WebSocket text message equals one complete SessionEnvelope JSON string.
- The entire message is parsed as JSON; there is no streaming or NDJSON framing.

**Minimal JSON shape**
```
{
  "schema_version": 1,
  "session_id": "...",
  "sent_at_ms": 1710000000000,
  "batch": {
    "events": [ { "channel": "...", "ts_ns": 1710000000000000000, "value": 1.23 } ],
    "end_of_stream": false
  }
}
```

**Invariants**
- `schema_version` must be 1.
- `session_id` must be a non-empty string.
- `batch.events[]` contains event objects.
- `ts_ns` can be a number or a numeric string; decoders treat both as u64.
- `batch.aggregates[]` is ignored on ingestion.

