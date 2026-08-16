# Racing Era Capability Matrix V1

Status: target contract for issue #242. The matrix distinguishes the current
implementation from the physical capabilities Pitgun intends to earn. It does
not enable a game era or claim physics that the Solver does not implement.

Capability identifiers are defined in the
[Racing Model Constitution V1](RACING_MODEL_CONSTITUTION_V1.md).

## Status vocabulary

- `implemented`: executed by the current versioned Solver and protected by its
  deterministic contract;
- `simplified`: executed, but with an intentional approximation that limits the
  physical claim;
- `planned`: required for the era but not implemented;
- `absent`: intentionally outside the current era or first delivery.

## Current consumer and catalog mapping

The game currently selects the latest unlocked vehicle name, while the
catalog resolves the physical vehicle. These are not yet a complete one-to-one
model of seven eras.

| Era | Game name | Current game vehicle intent | Catalog 1.3.0 reality |
| ---: | --- | --- | --- |
| 1 | Garage Days | `classic_v8_1960` | resolved |
| 2 | Industrialization | `classic_v8_1970` after its upgrade | resolved |
| 3 | Aero Era | `classic_v6t_1980` after its upgrade | vehicle resource absent |
| 4 | Composite Age | `classic_v10_1990`, then `classic_v8_2000` | vehicle resources absent |
| 5 | Digital Twin | `modern_v6t`, then `f1_2026` | both resolved; hybrid and active-aero names exceed current physical state |
| 6 | Hybrid Synthesis | late vehicle intent | gameplay and Opponent Policy V2 disabled |
| 7 | Theoretical Limit | late vehicle placeholder | gameplay and Opponent Policy V2 disabled; no Pod/Drone model |

The missing Era 3 and Era 4 resources are a contract gap, not permission to
silently reuse an unrelated vehicle. A later ticket must either publish the
named physical resources or simplify and document the game unlock mapping.

## Era 1 — Garage Days

Physical intent: a mechanically dominated combustion car whose behavior is
legible through mass, grip, torque, gearing, braking, fuel mass, and tires.

| Capability | Current | Target before calling the era complete |
| --- | --- | --- |
| `racing.vehicle.longitudinal` | implemented | keep and validate against representative historical ranges |
| `racing.powertrain.ice` | simplified | preserve a simple torque curve and gearbox; document fuel and thermal approximations |
| `racing.tire.state` | simplified | retain simple compound, temperature, and wear state |
| `racing.aero.fixed` | absent by vehicle resource | remain absent or minimal |
| energy and active-aero capabilities | absent | remain absent |

Gameplay emphasis should come from mechanical understanding, not a modern
energy system hidden under an old vehicle label.

## Era 2 — Industrialization

Physical intent: the Era 1 system becomes more repeatable and better engineered
without pretending that industrial process is itself a lap-time equation.

| Capability | Current | Target before calling the era complete |
| --- | --- | --- |
| Era 1 capabilities | implemented or simplified | preserve |
| `racing.aero.fixed` | simplified | introduce bounded fixed aerodynamic response |
| manufacturing consistency | absent from Solver | keep in gameplay or stochastic contracts unless a physical tolerance model is justified |
| energy and active-aero capabilities | absent | remain absent |

Reliability and manufacturing variance may later affect sessions, but they
belong in the Simulator unless they alter a resolved physical component.

## Era 3 — Aero Era

Physical intent: circuit-dependent drag/downforce trade-offs and ground-effect
behavior become material decisions.

| Capability | Current | Target before calling the era complete |
| --- | --- | --- |
| `racing.track.curvature` | implemented by model V2 | preserve continuous response |
| `racing.aero.fixed` | simplified | distinguish drag and downforce development, setup, and operating regimes |
| historical vehicle resource | absent | publish and validate the selected Era 3 vehicle identity |
| `racing.track.elevation` | simplified | use when circuit elevation provenance passes data checks |
| energy and active-aero capabilities | absent | remain absent |

An aerodynamic control must not be universally harmful or universally optimal.
Representative circuit classes must exhibit different reviewed optima.

## Era 4 — Composite Age

Physical intent: mass, braking, tire load, materials, and mature aerodynamics
interact without creating a fictional material-science submodel.

| Capability | Current | Target before calling the era complete |
| --- | --- | --- |
| Era 3 capabilities | implemented or simplified | preserve and recalibrate |
| Era-specific vehicle resources | absent | publish validated vehicle identities |
| mass and load response | simplified | make weight, normal load, braking, and tire effects inspectable |
| advanced materials | absent as physics | express through resolved mass/thermal parameters, not a hidden bonus |
| energy and active-aero capabilities | absent | remain absent |

## Era 5 — Digital Twin

Physical intent: a modern hybrid car whose energy behavior is real but whose
deployment is initially automatic, keeping the game approachable.

| Capability | Current | Target before calling the era complete |
| --- | --- | --- |
| modern combustion and thermal behavior | simplified | retain with calibrated limits and diagnostics |
| `racing.energy.accounting` | absent | implement fuel energy, electrical storage, delivered work, recovery, and losses |
| `racing.energy.automatic-control` | absent | add one deterministic, model-owned controller with reserve protection |
| `racing.energy.strategic-control` | absent | remain absent from the first Era 5 delivery |
| `racing.aero.active` | absent despite resource name | remain disabled until a commanded state exists |

The first hybrid delivery must expose state of charge, deployment, recovery,
loss, reserve, and thermal telemetry before it changes gameplay strategy.

## Era 6 — Hybrid Synthesis

Physical intent: energy and active systems become strategic decisions suitable
for a player or a bounded external agent.

| Capability | Current | Target before enabling the era |
| --- | --- | --- |
| Era 5 energy foundation | absent | required and validated first |
| `racing.energy.strategic-control` | absent | bounded deployment, harvest, reserve, and thermal objectives |
| `racing.aero.active` | absent | deterministic state and transition constraints |
| agent decision boundary | absent | versioned observations, actions, budgets, and deterministic fallback |
| late-era propulsion bridge | absent | introduce reusable source/storage/load diagnostics without flight physics |

Era 6 remains disabled until the energy balance and automatic controller are
stable. Agent control must use the same public action boundary as a human or
authored controller; it receives no private pace control.

## Era 7 — Theoretical Limit / Pod-Drone bridge

Physical intent: evolve the late Racing energy system into a fictional
Pod/Drone mission that creates professionally relevant energy-management
questions without initially solving trajectories.

The first Era 7 model operates on an already resolved, ordered mission path.
Segments may describe launch, acceleration, climb, cruise, maneuver, descent,
and reserve. The Solver computes energy feasibility and state evolution; it
does not choose a three-dimensional path or implement a flight controller.

| Capability | Current | First valid Era 7 target |
| --- | --- | --- |
| `racing.energy.accounting` | absent | reuse the governed balance semantics proven in Eras 5 and 6 |
| `racing.energy.strategic-control` | absent | allocate bounded energy across mission segments |
| `racing.mission.fixed-path` | absent | resolve deterministic segment duration, demand, and environment |
| `racing.propulsion.lift` | absent | model lift/hover/propulsion and auxiliary electrical loads at energy level |
| battery thermal and reserve state | absent | enforce safe operating and terminal-reserve invariants |
| trajectory optimization | absent | intentionally defer |
| attitude and flight control | absent | intentionally defer |

The bridge should prefer reusable concepts such as energy source, storage,
load, conversion efficiency, thermal state, reserve, and mission segment. It
must not force Racing and Drone code into a generic crate before two working
domains demonstrate a stable abstraction.

## Cross-era acceptance gates

An era can be enabled only when:

1. every active capability has an explicit status and deterministic tests;
2. its vehicle and parameter resources exist in an immutable catalog release;
3. development controls have reviewed marginal and interaction effects;
4. no required physical feature exists only in its display name;
5. native, WASM, Authority, and Verifier agree on model and resource identity;
6. a staging campaign demonstrates replay, telemetry, and rollback;
7. the game explains the era's new decision without requiring expert knowledge
   merely to complete a race.

This matrix is intentionally asymmetric: earlier eras stay simple, Era 5 adds
automatic energy, Era 6 exposes strategy, and Era 7 changes the mission domain.
Complexity is earned by the era rather than applied to every vehicle at once.
