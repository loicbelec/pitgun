# pitgun-racing-policy

`pitgun-racing-policy` owns Racing-specific setup canonicalization and input
validation.

It adapts `RaceInput`, `CompetitorSpec`, and `TuningSpec` from
`pitgun-racing-contract` to the domain-neutral loading, canonicalization, and
constraint primitives provided by `pitgun-policy`.

The crate intentionally owns the embedded `gametuning.v1.yaml` interpretation
and its Racing validation errors. It must not implement physics or race
orchestration.

