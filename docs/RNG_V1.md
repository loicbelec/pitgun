# Deterministic RNG V1

This document specifies the executable algorithms named by
`DeterministicRunContractV1`:

- `pitgun-splitmix64-v1`
- `sha256-label-v1`

Both algorithms are domain-neutral compatibility contracts. They are suitable
for deterministic simulation, not for keys, tokens, secrets, or any other
cryptographic randomness.

## `pitgun-splitmix64-v1`

The generator stores one unsigned 64-bit state. Construction sets `state` to
the supplied seed. Each call performs wrapping 64-bit arithmetic exactly:

```text
state = state + 0x9E3779B97F4A7C15
z = state
z = (z xor (z >> 30)) * 0xBF58476D1CE4E5B9
z = (z xor (z >> 27)) * 0x94D049BB133111EB
return z xor (z >> 31)
```

Addition and multiplication wrap modulo 2^64. Right shifts are logical. The
first call increments before mixing.

Seed zero begins with:

```text
e220a8397b1dcdaf
6e789e6aa1b965f4
06c45d188009454f
f88bb8a8724c81ec
1b39896a51a8749b
```

Seed `18446744073709551615` begins with:

```text
e4d971771b652c20
e99ff867dbf682c9
```

## `sha256-label-v1`

Independent random streams are derived without consuming a parent generator.
The four inputs are encoded as strings in this exact JSON array order:

```json
["<root-seed>","<component-id>","<entity-id>","<logical-index>"]
```

The seed and logical index use canonical unsigned decimal strings. Component
and entity labels are non-empty NFC Unicode strings of at most 256 UTF-8 bytes
without control characters.

The array is serialized with RFC 8785. SHA-256 hashes these bytes:

```text
UTF-8("pitgun.sha256-label-v1") || 0x00 || JCS(label_array)
```

The derived `u64` is the first eight digest bytes interpreted in big-endian
order. Changing any label dimension creates a different stream. Deriving one
stream never advances or otherwise affects another stream.

Published derivation vectors:

| Root seed | Component | Entity | Index | Derived seed (hex) |
|---|---|---|---:|---|
| `0` | `solver` | `entity-0` | `0` | `d34dc81fe421a5ad` |
| `7` | `racing.lap` | `player` | `1` | `29e0a03058dc9787` |
| `18446744073709551615` | `grid.node` | `poste-électrique` | `18446744073709551615` | `34b6b94dd89b109d` |

## Racing migration

The current Racing model keeps its existing `StdRng` and historical seed
derivation until an explicit compatibility migration. This V1 implementation
must not silently change existing golden output.

Adopting these algorithms in Racing requires:

1. a new Racing model version;
2. an explicit mapping from each random effect to component, entity, and logical
   index labels;
3. new native and WASM golden vectors;
4. release notes declaring that old and new run identities are not compatible.
