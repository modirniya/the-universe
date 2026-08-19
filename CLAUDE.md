# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Open-source **executable philosophy**: a runnable model of a simulation-hypothesis framework by Parham Modirniya. The codebase *is* the argument — each module implements one theory, and a successful run demonstrates the framework's internal coherence.

**Honest framing, which is a deliverable and not a disclaimer:** running this proves the ideas are *coherent*, not that our universe works this way. The model makes falsifiable predictions only about its own behaviour. Any summary the code prints must decline to overstate the result (`report::verdict` ends with this and a test enforces it). Philosophy that can't be coded lives in `docs/philosophy.md`, not in the code.

v0.1 and v0.2 are complete: one layer with four constraint toggles and a benchmark (Theory 1), plus chains of layers under the degradation rule (Theory 2).

## Commands

```sh
cargo run --release -- run  --config configs/default.toml  # Theory 1 experiment, ~11s on M1 Air
cargo run --release -- run  --config configs/quick.toml    # small world for iterating
cargo run --release -- nest --config configs/nesting.toml  # Theory 2 chain, <1s
cargo test                                                 # 108 tests
cargo test physics::                                       # one module
cargo test blinker_oscillates_with_period_two              # one test by name
cargo test --test determinism                              # the same-seed-same-universe suite
cargo test --test nesting                                  # Theory 2 end to end
cargo test -- --nocapture                                  # see println! from tests
cargo clippy --all-targets -- -D warnings                  # kept clean
cargo fmt
```

CLI overrides: `--seed <N>`, `--ticks <N>`, `--out <DIR>`.

CI (`.github/workflows/ci.yml`) runs fmt, clippy and tests on Linux, tests on
macOS ARM, and a `claims` job that runs the documented experiment and asserts
the *reproducible* parts of what the README says about it. If you change the
README's headline numbers, update that job's thresholds too — it exists to stop
the README and the code drifting apart. It never asserts on wall time.

Always benchmark with `--release`; debug builds are close to 20× slower (18.7× on `configs/quick.toml`, M1) and the numbers become meaningless.

## Architecture

Single crate. Module names match theory names — this is deliberate and load-bearing, since the code is meant to self-document the philosophy. Do not rename a module to something more conventional.

| Module | Role |
| --- | --- |
| `constraints` | The four toggles + `Params` dials; `Resolved` computes what they work out to |
| `space` | `Geometry` (fine grid + block partition), `World` (two-fidelity state), macro field |
| `physics` | Pure update rules; `step` and `tick` |
| `observer` | `Probe`, and the render/collapse events |
| `rng` | SplitMix64, hand-written; the creator's input channel |
| `experiment` | Runs each constraint setting, computes cost ratios and divergences |
| `budget` | Degradation rule; closed-form depth bound |
| `layer` | Nesting: sizes each layer to its budget, runs the chain |
| `report` | CSV, JSON, printed summary and verdict |
| `config` | TOML loading and validation |

**The loop** (`experiment::run`) is the whole model in three lines: `observe` forces detail into existence where something is looking → `tick` applies the laws → record what an outside observer could have seen.

### Three things worth understanding before editing

**Two-fidelity storage.** `World` holds `cells` (one byte each) *and* `coarse` (one density per block), with `resolved[b]` deciding which is authoritative. `World::sample` is the seam: inside a resolved block it reads the cell, inside an unresolved one it returns the block's density. That single function is where lazy rendering's cost saving *and* its error both come from. Physics reads neighbours only through it.

**Cost is quantized, and not monotonic in world size.** Lazy rendering charges by the block, and `layer::scale_probe` rescales the probe with the world — so a probe landing on block boundaries resolves far fewer blocks than one of the same area straddling them. A 48×48 layer costs more than a 64×64 one. This is why `layer::fit_spec` scans the whole range instead of walking down from an area estimate, and why `layer::predict_work` builds the real `Geometry` rather than estimating from coverage. `predict_work` is *exact*, and a test asserts equality with what the run spends; if that ever weakens to an inequality, the budget check has quietly become a guess.

**The chaos floor.** The world is chaotic, so any perturbation decorrelates the macro field and a raw divergence number means nothing. `experiment::run_all` therefore runs the unconstrained reference a *second* time with only the seed changed; that divergence is the floor chaos alone produces. Every verdict compares against the floor rather than against zero. If you change how divergence is measured, keep the control — without it the report is uninterpretable and every limit looks damning.

## Non-negotiables

**Physics is pure.** `physics::step` takes state and returns state. No mutation of inputs, no interior mutability, no logging, no clock, no I/O. `step_does_not_mutate_its_input` checks this rather than trusting a comment. This is what lets one law run at four resolutions and two fidelities and still be the same law.

**Determinism: same seed → same universe.** All randomness goes through `rng::Rng`. Never introduce `rand`, thread-local RNGs, `SystemTime`, or anything that depends on hash-map iteration order. Rendering uses `Rng::derive` for *positional* sub-streams keyed by block and tick — never draws from a shared stream, because that would make the result depend on visit order.

**Benchmarks are claims.** Every performance number in the README must be reproducible by a command in the repo. Distinguish reproducible counters (`Work`, divergences, cell counts) from machine-dependent measurements (wall time). Memory is reported twice on purpose — `peak_live_bytes` is what a resource-honest implementation would hold, `allocated_bytes` is what this one really allocates; reporting only one would be dishonest in one direction or the other.

**Module docs state their theory and what would falsify it within the model.** Keep this up when adding modules.

**Resolution-independent rules.** `physics::Rules` is stated as density bands, not neighbour counts, so the same law survives a change of resolution. At radius 1 the defaults reduce exactly to Conway B3/S23 (tested with blinker, block, glider). Any new rule must keep that property or the cross-resolution comparison is meaningless.

**Fair comparisons.** `World::seed` draws the pattern at base resolution and upsamples, so a coarse and a subdivided universe start from the same macro configuration. Without this the resolution comparison would compare two different initial conditions.

## Findings so far

Recorded because they are results of the model, not assumptions fed into it:

- All limits together: ~190× less work, ~14× less memory.
- **Discrete time is the only free lunch** — halves the cost and diverges *below* the chaos floor (0.94×).
- Space, speed cap and lazy rendering are all cheap but visible above the floor. Cheapness and invisibility are separate properties.
- **Discrete time and the speed cap are coupled**: influence covers `radius × substeps` cells per tick over cells of size `1/subdivision`, so refining time without refining space raises the physical speed of influence. See `constraints::Resolved`.

From v0.2:

- A chain's total cost is bounded by `root / (1 - fraction)` — 1.33× the root layer at the default fraction. Nesting is bounded in total spend, not just depth.
- The shipped chain **dies of the spatial floor, not the budget floor**: the closed form allowed 4 layers, the chain built 3.
- Churn falls ~10× per layer. The measure is biased *against* finding this (a fixed 16×16 macro grid makes small worlds look noisier), so the decline is if anything understated.
- `Degradation::max_depth` is an **upper bound**, not an equality: integer flooring at each generation costs real chains depth.
- A child with budget slack legitimately keeps its host's size — shrinkage is derived from scarcity, never imposed. Pinned by `a_child_with_slack_may_keep_its_hosts_size`.

## Decisions already made — do not relitigate

- **Language: Rust.** Strict compiler substitutes for human language expertise in an AI-built, AI-consumed codebase; supports the paradigm split; single fast binary; WASM later.
- **Paradigm split:** physics = pure functions over immutable state; entities/layers = traits + structs, composition over inheritance.
- **Dependencies stay minimal.** Currently `serde` + `toml`, both only for reading the config. The RNG is hand-written rather than pulled in, because `StdRng` is not guaranteed reproducible across `rand` versions and determinism is the project's first rule. Resist adding more.
- **Target hardware:** MacBook Air M1, fanless — long runs throttle. 2D toy scale only. Three interesting nested layers beat ten dead ones.
- **v0.1 world:** grid CA. **v0.1 probe:** fixed window. **Repo:** Parham's personal GitHub.
- **Distribution:** public repo, dual `MIT OR Apache-2.0`.

## Roadmap

Done: v0.1 (limits as optimizations), v0.2 (nesting and degradation).

Next, in order: **pipe** → **detection** → **fine-tuning sweep** → **emergence**. Also later: Python notebook shell for analysing output, visuals, WASM build.

This order is firm. Finish a milestone before starting the next, and do not widen the current one to include the next even where they touch — layers currently cannot reach each other, and that omission belongs to the pipe milestone, not this one.

## Vocabulary — use consistently in code and docs

- **Layer** — one universe in the chain. **Layer 0** is the host machine's process.
- **Horizon / pipe** — the one-way serializing channel between layers.
- **Logging threshold** — minimum aggregate scale at which a parent's observer notices child activity. Implemented as `report.macro_grid`.
- **Degradation rule** — each child's resource budget is a strict fraction of its parent's.
- **Bootloader** — an emergent agent/pattern whose effect is to instantiate computation one layer down.
- **Probe / observation** — the event that forces full-resolution computation of a region.
