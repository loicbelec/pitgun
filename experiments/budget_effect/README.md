# Controlled Racing development-budget effect

This experiment measures the causal response to one treatment: the controlled
player development budget. It contains 45 frozen triplets across five circuits,
three progression bands, and three seeds.

Each triplet uses one audited balanced opponent field, a neutral `0.5 / 0.5`
player setup, and the balanced one-stop player strategy. Player development is
set to 90%, 100%, and 110% of the nine-opponent median budget. The budget cap
and deterministic balanced allocation across aero, chassis, cooling, and engine
move together as the single treatment. Every other resolved input is identical.

Regenerate the immutable scenarios and checksummed manifest with:

```bash
python3 experiments/budget_effect/build_campaign.py
```

The assets contain no observed player, career, leaderboard, or telemetry data.
They freeze evidence inputs only and cannot choose or publish a difficulty rule.
