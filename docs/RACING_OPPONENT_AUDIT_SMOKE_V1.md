# Racing Opponent Audit — Local Smoke V1

Status: completed locally as the bounded preflight for game issue
`loicbelec/pitgun-game#156` and framework issue #209.

## Outcome

The exact current game opponents were executed through `pitgun run racing`
with Racing Catalog 1.2.0 and physical model 2.0.0. The controlled player uses
the same base budget, equal development allocation, neutral sliders and the
authored balanced one-stop strategy. It is not a copied player setup.

- Completed runs: 15/15
- Player wins: 2 (13%)
- Player podiums: 2 (13%)
- Player position range: P1–P10
- Median gap to leader: 82,627 ms
- Largest gap to leader: 158,082 ms
- Byte-identical retries: true
- Result artifact: `sha256:7e255e39fa91fe1ad1d1c24fa7ce3268dcba8bdae32ce7275ca869470e049c14`

## Matrix

| Circuit | Progression | Player | Gap to leader (ms) | Field spread (ms) | Distinct setups | Distinct strategies |
|---|---:|---:|---:|---:|---:|---:|
| BUDAPEST | early | 8 | 75,170 | 84,843 | 9/9 | 8/9 |
| BUDAPEST | mid | 10 | 148,243 | 148,243 | 9/9 | 8/9 |
| BUDAPEST | late | 10 | 138,458 | 138,458 | 9/9 | 7/9 |
| MONACO | early | 6 | 55,502 | 82,617 | 9/9 | 9/9 |
| MONACO | mid | 10 | 134,640 | 134,640 | 9/9 | 7/9 |
| MONACO | late | 10 | 101,723 | 101,723 | 9/9 | 9/9 |
| MONZA | early | 7 | 44,390 | 71,263 | 9/9 | 7/9 |
| MONZA | mid | 1 | 0 | 97,081 | 9/9 | 8/9 |
| MONZA | late | 1 | 0 | 94,222 | 9/9 | 6/9 |
| SINGAPORE | early | 7 | 39,399 | 41,215 | 9/9 | 6/9 |
| SINGAPORE | mid | 10 | 114,067 | 114,067 | 9/9 | 9/9 |
| SINGAPORE | late | 10 | 158,082 | 158,082 | 9/9 | 8/9 |
| SUZUKA | early | 4 | 63,239 | 81,824 | 9/9 | 8/9 |
| SUZUKA | mid | 10 | 82,627 | 82,627 | 9/9 | 8/9 |
| SUZUKA | late | 10 | 127,076 | 127,076 | 9/9 | 8/9 |

## Interpretation boundary

This smoke proves the cross-repository contract and execution path and provides
an initial diagnosis. It does not decide game balance: one neutral reference,
one seed and one strategy cannot represent player skill. The governed
Databricks campaign will add reviewed player references and seeds, persist Delta
lineage and make the policy decision auditable.

No career, leaderboard, private setup or observed telemetry data was consumed.
