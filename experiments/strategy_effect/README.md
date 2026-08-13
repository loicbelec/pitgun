# Controlled Racing strategy effect

This experiment isolates the controlled player's one-stop strategy before any
opponent policy is designed. It derives 45 pairs from the frozen public game
opponent audit: five circuits, three progression bands, and three seeds.

Within each pair, the vehicle, era, lap count, player budget, player tuning,
opponents, opponent tuning, opponent strategies, and seed are identical. Only
`player.stint_strategy` differs between `balanced-one-stop` and
`late-one-stop`. The player setup is the neutral `0.5 / 0.5` control so the
known Monza circuit-baseline anomaly cannot contaminate this experiment.

Regenerate the immutable scenarios and checksummed manifest with:

```bash
python3 experiments/strategy_effect/build_campaign.py
```

The generated assets contain no observed player, career, leaderboard, or
telemetry data. A successful campaign remains evidence only: it cannot select
or publish a game or catalog policy.
