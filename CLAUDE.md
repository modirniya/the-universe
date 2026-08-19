# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Open-source **executable philosophy**: a runnable model of a simulation-hypothesis framework by Parham Modirniya. The codebase *is* the argument — each module implements one theory, and a successful run demonstrates the framework's internal coherence.

**Honest framing, which is a deliverable and not a disclaimer:** running this proves the ideas are *coherent*, not that our universe works this way. The model makes falsifiable predictions only about its own behaviour. Any summary the code prints must decline to overstate the result (`report::verdict` ends with this and a test enforces it). Philosophy that can't be coded lives in `docs/philosophy.md`, not in the code.

v0.1 through v0.7 are complete: the six theories, plus a WebAssembly build and a browser viewer.

## Layout

A workspace. `universe-core` is the crate at the repository root — the six milestones, two dependencies, unchanged by the split. `crates/universe-web` is a thin wasm-bindgen bridge that owns no physics. `web/` is the static viewer. The core stays at the root because tests and the CLI resolve `configs/...` relative to the package root; moving it would change every documented command.

## Commands

```sh
cargo run --release -- run  --config configs/default.toml  # Theory 1 experiment, ~11s on M1 Air
cargo run --release -- run  --config configs/quick.toml    # small world for iterating
cargo run --release -- nest --config configs/nesting.toml  # Theory 2 chain, <1s
cargo run --release -- pipe --config configs/pipe.toml     # Theory 3 relay, <1s
cargo run --release -- detect --config configs/detect.toml # Detection survey, ~10s
cargo run --release -- sweep  --config configs/sweep.toml  # Theory 6 sweep, ~6s at 21 steps
cargo run --release -- boot   --config configs/boot.toml   # Theory 5 boot chain, ~5s
cargo test --workspace                                     # native suite
cargo test physics::                                       # one module
cargo test blinker_oscillates_with_period_two              # one test by name
cargo test --test determinism                              # the same-seed-same-universe suite
cargo test --test nesting                                  # Theory 2 end to end
cargo test --test pipe                                     # Theory 3 end to end
cargo test --test detection                                # Detection end to end
cargo test --test sweep                                    # Theory 6 end to end
cargo test --test bootloader                               # Theory 5 end to end
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
| `pipe` | Horizon, serialization, `WriteEnd`/`ReadEnd`, logging threshold |
| `detector` | Inhabitant measurements; which limits are findable from inside |
| `sweep` | Fine-tuning: scores each rule setting, counts distinct laws |
| `bootloader` | Cluster tracking, bootloader detection, the boot chain |
| `golden` | The pinned reference universe and its fingerprint |
| `report` | CSV, JSON, printed summary and verdict |
| `config` | TOML loading and validation |

**The loop** (`experiment::run`) is the whole model in three lines: `observe` forces detail into existence where something is looking → `tick` applies the laws → record what an outside observer could have seen.

### Four things worth understanding before editing

**Two-fidelity storage.** `World` holds `cells` (one byte each) *and* `coarse` (one density per block), with `resolved[b]` deciding which is authoritative. `World::sample` is the seam: inside a resolved block it reads the cell, inside an unresolved one it returns the block's density. That single function is where lazy rendering's cost saving *and* its error both come from. Physics reads neighbours only through it.

**Mutual blindness is a type-system invariant, not a convention.** `pipe::WriteEnd` has `write` and `seal` and nothing else — no method returns anything about the far side. `ReadEnd` cannot write, and nothing converts it back. Do not add a read method, a receipt, an acknowledgement, or a `&mut` accessor that hands out both halves: the whole point is that a child cannot discover it is being read. If a future milestone needs bidirectional flow, that is a new type, not a loosened one.

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

From v0.6:

- The boot chain closes the loop: each layer is seeded from what crossed its parent's horizon, using all six theories at once.
- **A chain can die of sterility rather than poverty** — Theory 5 supplies a depth limit independent of the budget. Which limit binds depends on the floors in `[nesting]`.
- Poorer layers produce less life: bootloaders fall 128 → 32 → 6 down the chain.
- Rust folds float sums from `-0.0` (the true additive identity), so an empty `sum::<f64>()` prints as `-0.0`. Normalise with `+ 0.0` before reporting.

From v0.5:

- 19% of reachable laws are productive: fine-tuning holds, but weakly.
- **Complexity criteria must be bands, not floors.** An activity floor admitted chaotic rules churning at 20× Conway. Class 3 is not class 4.
- **Count distinct laws, not grid area.** 441 settings denote 42 laws because only `k/8` densities occur. An area fraction reports the sweep's resolution, not the universe's. `sweep::rule_signature` canonicalises.
- Raw macro variance nearly tracks density; normalise by the i.i.d. baseline `p(1-p)/cells_per_macro` before calling anything "structure".

From v0.4:

- Pixelation is undetectable from inside: the cell is the ruler.
- `influence_speed` measures `radius × substeps` and cannot factor it — the v0.1 coupling reappears as a limit on knowledge.
- Lazy rendering is concealed by the act of measuring it. `Gaze::Rendering` vs `Gaze::Passive` shows this is a consequence of the framework's definition of a probe, not an artefact of where the inhabitant stands.
- **Detections need an absolute floor, not just a relative one.** 0.0002 vs 0.0001 is a 50% relative gap and pure noise; it was reported as a finding until `MIN_ABSOLUTE` existed. Any new "is this different" test needs both.
- Whether a speed bound is *reached* is region-dependent and not a stable invariant. Assert the ceiling, report saturation as an observation.

From v0.3:

- Theory 3's split holds: content is destroyed (50.2% digest avalanche on a one-cell change) while timing and magnitude survive (0.79 correlation) through a channel carrying 5.6% of the information.
- **Threshold sweeps manufacture perfect correlations.** The first version of the report showed 1.0000 at a high threshold — from two data points, where Pearson is always ±1. `MIN_CORRELATION_SAMPLES` refuses to print a correlation below five events, and the sweep shows the event count beside every row. Any future statistic computed over a filtered subset needs the same guard.

## Decisions already made — do not relitigate

- **Language: Rust.** Strict compiler substitutes for human language expertise in an AI-built, AI-consumed codebase; supports the paradigm split; single fast binary; WASM later.
- **Paradigm split:** physics = pure functions over immutable state; entities/layers = traits + structs, composition over inheritance.
- **Dependencies stay minimal.** Currently `serde` + `toml`, both only for reading the config. The RNG is hand-written rather than pulled in, because `StdRng` is not guaranteed reproducible across `rand` versions and determinism is the project's first rule. Resist adding more.
- **Target hardware:** MacBook Air M1, fanless — long runs throttle. 2D toy scale only. Three interesting nested layers beat ten dead ones.
- **v0.1 world:** grid CA. **v0.1 probe:** fixed window. **Repo:** Parham's personal GitHub.
- **Distribution:** public repo, dual `MIT OR Apache-2.0`.

## Roadmap

**Determinism is now a cross-target invariant.** `universe_core::golden` pins one reference universe and reduces it to a `u64`. The native suite and the WebAssembly suite each assert `GOLDEN_FINGERPRINT`; neither sees the other's answer. If a change to physics moves that number legitimately, update the constant *and say so in the commit* — a silent update makes the check meaningless. If it moves without a deliberate change, that is a finding about the first rule, not a flaky test.

**The viewer contains no physics.** `web/app.js` draws and wires controls; every rule comes from the core through wasm. Do not reimplement a fast approximation in JS — the page would then be showing a different universe from the one the findings describe.

**The original roadmap is complete**: v0.1 (limits as optimizations), v0.2 (nesting and degradation), v0.3 (the pipe), v0.4 (detection), v0.5 (fine-tuning sweep), v0.6 (bootloader life).

Done in phase 2: v0.7 (workspace split, WebAssembly, the viewer, Pages deploy). The Python notebook shell is v0.8. The research track — seeded replicators, evolution — stays deliberately unscheduled. Also later: Python notebook shell for analysing output, visuals, WASM build.

This order is firm. Finish a milestone before starting the next, and do not widen the current one to include the next even where they touch — layers currently cannot reach each other, and that omission belongs to the pipe milestone, not this one.

## Vocabulary — use consistently in code and docs

- **Layer** — one universe in the chain. **Layer 0** is the host machine's process.
- **Horizon / pipe** — the one-way serializing channel between layers.
- **Logging threshold** — minimum aggregate scale at which a parent's observer notices child activity. Implemented as `report.macro_grid`.
- **Degradation rule** — each child's resource budget is a strict fraction of its parent's.
- **Bootloader** — an emergent agent/pattern whose effect is to instantiate computation one layer down.
- **Probe / observation** — the event that forces full-resolution computation of a region.
