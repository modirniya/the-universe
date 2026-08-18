## What this changes

<!-- One or two sentences. If it closes an issue, say "Closes #N". -->

## Why

<!-- What problem this solves, or which theory it implements. -->

## Checks

<!-- CI runs the first three. The rest are things CI cannot see. -->

- [ ] `cargo fmt` clean
- [ ] `cargo clippy --all-targets -- -D warnings` clean
- [ ] `cargo test` passes

The five rules in [CONTRIBUTING.md](../blob/main/CONTRIBUTING.md):

- [ ] **Determinism holds.** No new randomness outside `rng::Rng`, no clock, no
      reliance on hash-map iteration order. Anything position-dependent uses
      `Rng::derive` rather than a shared stream.
- [ ] **Physics is still pure.** No mutation of inputs, no interior mutability,
      no logging or I/O in `physics`.
- [ ] **Benchmarks still match their claims.** If the numbers moved, the README
      and the `claims` CI job were updated in the same commit.
- [ ] **New modules name their theory** and what would falsify it within the
      model.
- [ ] **Rules stay resolution-independent** — density bands, still reducing to
      B3/S23 at radius 1.

## Dependencies

- [ ] No new dependency.
- [ ] Adds one, and the description below says what it buys and why the standard
      library will not do.

## Numbers

<!--
Only if this touches physics, the cost model, or the report. Paste the summary
from:

    cargo run --release -- run --config configs/default.toml

Wall time is expected to differ from the README's — it is a measurement, not a
counter. The work ratios and divergences are the ones that should be stable.
-->
