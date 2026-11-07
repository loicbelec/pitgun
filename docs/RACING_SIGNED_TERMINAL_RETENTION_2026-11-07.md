# Signed Racing — release completed physical histories before evidence

7 November 2026. A small Racing orchestration change reduces memory on the
**current signed Model 0.14 path**, without changing sampling, physics, JSON
contracts or canonical output. It follows the [initial local V9 screen](RACING_PERFORMANCE_ATTRIBUTION_2026-11-07.md).
Tracking: [#430](https://github.com/loicbelec/pitgun/issues/430),
[game #315](https://github.com/loicbelec/pitgun-game/issues/315), draft PR #431.

## One retention change

`AuthorizedDynamicRacingSession` retained its complete physical
`IncrementalRacingSession` while cloning terminal output, deriving evidence,
serializing the terminal batch and waiting for `complete()`. Those Solver histories
were no longer used, but overlapped the terminal allocations.

The wrapper now releases that physical session once the terminal batch owns the
complete output, **before** constructing the application result and evidence.
The result remains available through the existing handle. Intermediate pulls,
stream ordering, cancellation and post-terminal completion/release retain their
public behavior. Failed finalization cannot reopen physical computation.

Histories remain retained during physical calculation. The legacy local V9 facade,
generic runtime/contracts, catalog, coefficients, precision, RNG, driver
instructions, telemetry frequency and JSON schemas are unchanged. No golden
fixture or expected digest is rebased.

## Exact signed workload and trust boundary

The owner's **local QA** Monza race: 53 laps, ten competitors, 5,787 spatial
samples, 5 Hz telemetry, 37,944 frames, Catalogue 1.8.0 and Model 0.14.0. Its
original submission was VERIFIED. The packed replay has SHA-256
`fcdf4319a1202ad8d2bbaa4c81285a578de100fdc29640344e7e0952554b4fb5`.
The signed input/evidence are recovered from that verified archive and retained
privately, outside Git.

There is no new Authority request, Verifier submission, leaderboard claim, model
call, browser/save action or deployment. Offline reconstruction is **not a new
trusted VERIFIED verdict**. The declared historical artifact digest is frozen as
part of identical input for the A/B; it does not attest either rebuilt binary.
Actual baseline/candidate binary hashes are recorded separately. A deployed new
module must declare/register its own real digest, retaining old pinned runtimes
for existing attempts.

## Measurements

Three fresh processes per runtime/variant, alternating native/WASM within each
variant; reference block first, candidate block second. Same input, release
settings, Rust 1.89.0, pinned bindings and machine (M4 Pro / 24 GiB). Compilation,
equivalence dumps and canonical hashing are outside timed processes. No forced GC
or browser. Ordinary OS activity is not fully isolated; this is not a p95 bound.

| Median of three | Reference | Candidate | Change |
| --- | ---: | ---: | ---: |
| WASM/Node process peak RSS | 1,192.9 MiB | 987.5 MiB | −17.2% |
| WASM linear capacity at completion | 829.1 MiB | 572.3 MiB | −31.0% |
| Native process peak RSS | 1,908.9 MiB | 1,629.4 MiB | −14.6% median |
| WASM API total | 10.335 s | 9.924 s | descriptive only |
| Native API total | 5.453 s | 5.194 s | descriptive only |

All three WASM RSS samples separate: reference 1,192.1–1,193.4 MiB, candidate
987.4–988.0 MiB. Linear capacity repeats exactly in each variant. Native ranges
overlap: reference 1,607.8–1,910.0 MiB, candidate 1,626.8–1,638.0 MiB. Do not claim
every native trial improved or a general CPU speedup from this small fixed-order
experiment. API time excludes host parsing and initialization.

RSS, private footprint, JS heap and linear capacity are different metrics. Phase
snapshots are endpoints, not phase maxima. The initial native child-`ps` sampler
pilot is excluded entirely; final native measurement forks no samplers. MaxRSS is
measured externally by the OS. Node phase snapshots remain in-process.

**JSON volume is identical**: WASM stream plus completion emits 299,246,775 bytes
before and after. No smaller archive or lower sampling is claimed. The gain comes
from avoiding dead physical state overlapping terminal work.

## Full identity and regression checks

Canonical application, runtime output and evidence are hashed in separate untimed
processes. All before/after hashes match **within each runtime**, covering every
telemetry field. Both evidence hashes also equal the historical VERIFIED proof:
`sha256:86f5570b1dd9c2a14c483d39fd4433b5efa2793a6d673c8475dce4024a8ca2e8`.
Native/WASM full runtime-output hashes already differ in the reference; arbitrary
cross-runtime floating-point identity is not claimed.

[Compact measurements, runtime/source fingerprints and canonical hashes](qa/signed-terminal-retention-2026-11-07.json).
Reference probe source is `81d08c5`; measured candidate library source matches
commit `0dd875a` exactly. The baseline WASM uses the unchanged pre-fix library.

Passing checks: 136 native tests across Racing policy/Solver/Simulator and the
compatibility facade; 22 actual Node/WASM tests, covering published goldens,
evidence, RNG and thermal profiles (Models 0.14/0.15 in the suite). The lifecycle
regression now tests consumption once and releasing a terminal session without
consuming its result. Formatting, strict Clippy for library/probe and changed
golden target, plus Node/Python syntax checks pass.

## Reproduction and remaining gates

Build release native `signed_racing_performance_probe` and web WASM bindings from
each revision, preserving binaries in separate directories. Input JSON contains
`request_json` (the exact `RacingDynamicExecutionRequestV1`) and
`catalog_bundle_json`. Use a QA export/dummy authorization, never signing keys.

```sh
python3 scripts/signed-racing-trials.py /private/qa/input.json /private/qa/reference /private/qa/reference/trials
python3 scripts/signed-racing-trials.py /private/qa/input.json /private/qa/candidate /private/qa/candidate/trials
```

Each runtime directory contains `native-probe`, `pitgun_racing_simulator.js` and
`pitgun_racing_simulator_bg.wasm`. Drivers refuse to overwrite evidence and check
source/input/runtime hashes. Native `dump` and optional Node dumps run separately;
native `canonical` hashes those complete outputs afterwards. The exact private
input can be reconstructed from the retained QA archive/evidence; no public signed
full-GP fixture is published here.

The measured WASM gain justifies retaining the change. This is **not browser-memory
acceptance**: renderer/GPU, adaptation, archive Workers, hashes/codecs/gzip and
storage are outside the experiment. Node still peaks near 1 GiB. Neither a browser
plateau, mobile acceptance nor resolution of Chrome's warning is established.

The game still uses its original pinned module. Next: integrate the new real
artifact into an isolated lot 1 build, preserve old runtimes, and measure signed
recovery/GP/HQ/replay in Chrome/Safari. Backend 24 h admission, release manifest,
cold/real-clock/mobile and Engineer gates remain. No staging/prod rollout or lot 2
action activation is included.
