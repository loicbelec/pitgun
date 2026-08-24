# Racing Model V3 driver-control contract V1

Status: contract implemented by offline candidate Model `0.12.0` under
[`loicbelec/pitgun#311`](https://github.com/loicbelec/pitgun/issues/311).
It does not change any published model or immutable catalog release. Runtime
details are documented in
[`RACING_V3_DRIVER_CONTROL_RUNTIME_V1.md`](RACING_V3_DRIVER_CONTROL_RUNTIME_V1.md).

## Problem

The current physical driver resource exposes one `aggressiveness` scalar. The
Model V3 compatibility resolver maps a larger value to all of the following:

- more cornering-force utilization;
- more braking-force utilization;
- more traction-force utilization;
- less deterministic control error.

Aggressiveness is therefore nearly strictly beneficial. It does not describe a
decision with a cost, and it cannot support understandable driver archetypes.
The player is also hard-coded to `default`, while opponents receive named
drivers.

The catalog still contains `aggressive`, `balanced`, and `conservative` files
with setup and strategy fields. They contain no physical aggressiveness value,
are rejected by the Model V3 driver parser, and do not influence a race. These
files must not be mistaken for active driver profiles.

## Boundary decision

Pitgun separates persistent ability from the decision made for one session:

| Layer | Owns | Does not own |
| --- | --- | --- |
| Driver resource | force-envelope skill, consistency, tire management | session strategy, hidden pace bonus |
| Session contract | selected driver and driving mode | driver ability |
| Model parameter resource | mode targets and physical response coefficients | player or opponent identity |
| Solver | resolved utilization, deterministic error and contact workload | game labels, AI policy |
| Simulator | mode resolution and race evolution | hidden post-solve milliseconds |

The same resource and session contract apply to the player and every opponent.

## Persistent driver traits

The next catalog generation introduces a versioned driver resource with three
bounded traits in `[0, 1]`:

```json
{
  "schema_version": "pitgun.racing-driver/v2",
  "id": "smooth_operator",
  "traits": {
    "limit_exploitation": 0.82,
    "consistency": 0.94,
    "tire_management": 0.91
  }
}
```

- `limit_exploitation` describes how much of the available physical force
  envelope a driver can use. It is not a lap-time multiplier.
- `consistency` reduces the amplitude of deterministic sample-level control
  errors. It does not remove the cost of requesting more commitment.
- `tire_management` reduces correction-induced tire workload. It does not
  alter the tire resource's grip, baseline wear, or thermal coefficients.

A driver may be better than another, but the authored roster must contain
meaningful trade-offs. Catalog validation will reject values outside the
declared interval; it will not silently clamp them.

## Session driving mode

This section documents the static whole-session input used to evaluate the
first driver-control candidates. The governed mode-surface review later showed
that coefficient tuning alone cannot make that static usage sustainable over
race distance. Future timeline-enabled profiles therefore use the common
default and ordered transitions defined in
[`RACING_DETERMINISTIC_DRIVER_INSTRUCTIONS_V1.md`](RACING_DETERMINISTIC_DRIVER_INSTRUCTIONS_V1.md).
Historical model and catalog identities keep the behavior below unchanged.

Every competitor selects one explicit mode:

- `manage` requests low commitment and protects tires;
- `balanced` requests the reviewed nominal commitment;
- `attack` requests high commitment at the cost of error and contact workload.

The mode is deterministic input. It is not sampled during the race and cannot
react invisibly to the player's result. Opponent Policy chooses modes before
execution from the same public contract the player uses.

The mode names are presentation labels. Their numerical commitment targets
belong to an immutable model parameter resource so they can be screened without
recompiling Rust.

## Physical resolution

The accepted equation shape is:

```text
requested_commitment = mode_profile[driving_mode]

maximum_utilization[channel]
  = utilization_floor[channel]
  + utilization_span[channel]
    * driver.limit_exploitation
    * requested_commitment

control_error_amplitude
  = base_error
  + commitment_error_gain
    * requested_commitment^commitment_error_exponent
    * (1 - driver.consistency)

correction_workload_multiplier
  = 1
  + correction_workload_gain
    * requested_commitment
    * control_error_amplitude
    * (1 - driver.tire_management)
```

The existing deterministic signal keyed by execution seed, driver, lap,
sample, and control channel reduces the resolved utilization by the error
amplitude. It remains reproducible and never becomes a distributed time bonus.

The correction workload represents the reduced-order thermal and wear cost of
less smooth steering, braking, and traction corrections. It may add heat and
wear only through the existing contact-patch state equations. It may not reduce
the base physical workload below one.

All floors, spans, mode targets, gains, and exponents are named fields in one
versioned driver-control parameter resource. Numerical values are not accepted
by this document.

## Required diagnostics

Each resolved run must expose enough data to explain the result:

- driver resource identity and traits;
- requested driving mode and resolved commitment;
- cornering, braking, and traction utilization distributions;
- control-error amplitude and realized deterministic error distribution;
- base contact workload and correction-induced workload;
- tire heat and wear attributed to the correction workload;
- lap-level pace and consistency summaries.

These diagnostics are eligible for governed local and Databricks campaigns.
They do not all need to enter the compact verifier evidence, but their model and
resource identities do.

## Compatibility

- Catalogs through Racing `1.6.0` retain the exact legacy aggressiveness
  resolver and their published model identities.
- The new traits and driving mode activate only under a new model identity and
  a catalog that explicitly declares their resources.
- Missing V2 traits fail closed for the new model; they never fall back to a
  silently synthesized perfect driver.
- Historical `aggressive`, `balanced`, and `conservative` files remain in
  immutable releases but are absent from the next catalog index.
- Native Rust, WASM, Authority, and Verifier must resolve the same driver and
  mode identities.

## Validation campaign

The implementation is not promoted from a hand-picked roster. The governed
campaign must cover:

- `manage`, `balanced`, and `attack` for every candidate driver archetype;
- representative low-, medium-, and high-speed circuits;
- short and race-length sessions;
- soft, medium, and hard compounds;
- at least three deterministic seeds;
- equal vehicles and development budgets before opponent-policy composition.

The campaign must demonstrate:

1. `attack` can improve peak pace but increases error or tire workload;
2. `manage` preserves tires but cannot dominate short-run pace;
3. consistency changes dispersion without granting a hidden mean-time bonus;
4. tire management changes physical heat and wear rather than tire identity;
5. no single driver-and-mode pair dominates every reviewed scenario;
6. local and Databricks executions produce the same natural keys and results.

## Delivery sequence

1. add the versioned traits, mode, and parameter-resource contracts;
2. implement physical resolution and diagnostics under a new candidate model;
3. execute the local screening campaign;
4. replay and review the campaign on Databricks;
5. publish the reviewed catalog and model identities;
6. let Pitgun Game select a driver, start every competitor from the common
   catalog-resolved default, and later issue deterministic instructions through
   [`pitgun-game#177`](https://github.com/loicbelec/pitgun-game/issues/177);
7. regenerate Opponent Policy only after the physical surface is accepted.

This order prevents AI balance from hiding an invalid driver model.
