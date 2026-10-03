# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

An open-source **computational investigation of a simulation-hypothesis framework** by Parham Modirniya. A toy universe in which each of the framework's theories is a definition or a parameter; experiments with controls and replication; and a strict separation between what the model defines, what follows from its definitions, what had to be run, and what is said about our universe (which the repository never tests).

**Honest framing is a deliverable, not a disclaimer.** Running this establishes facts about a program. Every printed summary ends by declining to overstate the result, and a test enforces the sentence. When a claim has to change, weaken the claim; never strengthen the rhetoric. A negative or inconvenient result is preserved, not smoothed over: v1.0 withdrew five v0.9 findings and the README lists them.

v1.0 is the current state: the six theories (v0.1–v0.6), WebAssembly and the viewer (v0.7), the analysis shell (v0.8), the ensemble pass (v0.9), and the methodological redesign (v1.0: audit, factorial Theory 1 with nulls, detection as hypothesis tests, the pipe in bits, the termination map, the measure problem, bootloader controls, provenance, the claims ledger).

## The four categories

Every claim is one of: **definition** (built in; established by reading the code), **consequence** (follows from the definitions; derived in `docs/derivations.md`), **computational finding** (could have come out otherwise; needs an experiment, controls, replication, an uncertainty), **physical hypothesis** (about our universe; established by nothing here, lives only in `docs/philosophy.md`). The ledger `analysis/claims.toml` records every claim with its category; `analysis/ledger.py` checks the ledger against the README and the artifacts. **Adding or changing a claim means editing the ledger and the README together**, or the check fails. Never describe a consequence as a discovery; never present a definition as a measurement (v0.9's `min_feature: 1.0` was a literal reported as measured).

## Layout

A workspace. `universe-core` at the repository root holds the model, the experiments and the reports (configs are resolved relative to the package root, which is why it stays there). `crates/universe-web` is a thin wasm-bindgen bridge that owns no physics. `web/` is the static viewer. `analysis/` holds the ledger, its checker, a pandas second opinion and the figures. `docs/` holds the audit, the derivations, the architecture and the philosophy.

## Commands

```sh
scripts/reproduce.sh                                          # every published table (~1 h on 4 cores); `quick` for the cheap ones
cargo run --release -- run     --config configs/default.toml  # Theory 1 factorial, 4 nulls, 7 observables, closure ablation; ~7 min
cargo run --release -- run     --config configs/quick.toml    # small world for iterating, 20 s
cargo run --release -- nest    --config configs/nesting.toml  # chain, termination map, size control, churn vs size; ~2 min
cargo run --release -- pipe    --config configs/pipe.toml     # correlation sweep + information analysis; ~1 min
cargo run --release -- detect  --config configs/detect.toml   # 40 seeds: 20 calibrate, 20 evaluate; ~30 s
cargo run --release -- boot    --config configs/boot.toml     # chain, gate ablation, shuffle + area controls; ~5 min
cargo run --release -- boot    --config configs/boot-permissive.toml
cargo run --release -- edge    --config configs/edge.toml     # why chains end, 10 seeds x 98 chains; ~5 min
cargo run --release -- sweep   --config configs/sweep.toml    # the v0.5 grid sweep; ~3 min
cargo run --release -- measure --config configs/measure.toml  # 5 priors x 3 criteria, 6 universes; ~35 min
cargo test --workspace                                        # native suite (~230 tests)
cargo test --test detection                                   # one integration suite
cargo test physics::                                          # one module
cargo clippy --workspace --all-targets -- -D warnings         # kept clean
cargo fmt --all
wasm-pack test --node crates/universe-web                     # the cross-target fingerprint
python3 analysis/ledger.py                                    # ledger vs README vs artifacts (stdlib only)
python3 analysis/ledger.py --stamp                            # after a full reproduce at a clean commit
```

CLI overrides: `--seed <N>`, `--ticks <N>`, `--seeds <N>`, `--out <DIR>`, `--budget <N>` (nest/boot/edge), `--steps <N>` (sweep). Overrides are recorded in `metadata.json`.

Always benchmark with `--release`; debug builds are close to 20× slower.

**Ensembles.** Every config runs 20 seeds (`detect.toml` 40, `edge.toml` 10). The pinned seed (42) runs alone first and prints in full; the rest run in parallel via `experiment::per_seed`. Wall time is never reported for parallel members. A seed selects an initial condition and nothing else; say so when quoting an interval. Report `mean ± sd [95% CI]` (`stats::Summary`) and give counts of seeds a Wilson interval (`stats::wilson`) — "20/20" is [0.84, 1.00]. Never quote seed 42 alone as a finding.

**CI** (`.github/workflows/ci.yml`): fmt, clippy, tests on Linux; tests on macOS ARM; the wasm fingerprint; and a `claims` job that runs `scripts/reproduce.sh`, requires a clean-tree `metadata.json` beside every artifact, runs `analysis/ledger.py --ignore-commit`, and checks a short list of by-construction invariants. If a number in the README changes, change the ledger's range in the same commit.

## Architecture

Module names match theory names on purpose; do not rename a module to something more conventional. `docs/architecture.md` states the MODEL / EXPERIMENTS / RESULTS / INTERPRETATION layering: nothing in the model imports from the experiments; assumptions are parameters and are varied; every finding has a null; consequences are derived, not run; interpretation stays downstream.

| Module | Role | Layer |
| --- | --- | --- |
| `constraints` | The four toggles, `Params`, `CoarseRule` (binomial / indicator / frozen), `Resolved` | model |
| `space` | `Geometry`, `World` (cells + block densities, `sample` as the seam), macro field | model |
| `physics` | Pure `step` and `tick`; density-band `Rules` with an optional radius-1 table; the three closures | model |
| `observer` | `Probe`; render and collapse | model |
| `rng` | SplitMix64, positional `derive` | model |
| `budget`, `layer` | Degradation rule; chain, exact `predict_work`, termination map, size control, churn vs size | model / experiments |
| `pipe` | `Message`, `WriteEnd`/`ReadEnd`, horizon, relay, correlation sweep | model |
| `bootloader` | Cluster tracking as a pure function of frames, the boot chain, `Gate`, shuffle and area controls, the `edge` map | model / experiments |
| `experiment` | `run` (records a `Profile`), `per_seed`, `Spread` | experiments |
| `limits` | Theory 1: the 2⁴ factorial, four nulls, `D`, main effects, interactions, Pareto set | experiments |
| `observables` | Seven macro-scale observables | experiments |
| `detector` | Hypothesis tests: statistics, calibration/evaluation split, negative and resample controls, `Standing` | experiments |
| `information` | Mutual information per task per encoding; window, noise and bit-flip controls | experiments |
| `sweep`, `measure` | The grid sweep; five priors, three criteria, six universes | experiments |
| `stats` | `Summary`, Wilson, Cohen's d, z and tail against a null, deterministic bootstrap | experiments |
| `golden`, `provenance` | The cross-target fingerprint; `metadata.json` | experiments |
| `report` (+ `report::{limits,detect,information,nesting,measure,boot}`) | CSV, JSON, summaries ending with the framing sentence | results |

### Things worth understanding before editing

**Two-fidelity storage and the closure.** `World::sample` returns the cell inside a resolved block and the block's density inside an unresolved one. What advances an unresolved block is `CoarseRule`, an *assumption*. The binomial closure is the expected next density under independent cells (it holds Life near its mean-field fixed point 0.37 while Life decays — a known failure of mean fields, measured here, not a bug). The indicator closure is kept only to reproduce v0.9. Any result about lazy rendering must be reported under all three.

**Same-seed comparisons need a same-seed null.** A limit toggled at the same seed starts correlated with the reference; a reseed does not. Report divergence over the second half and at the final tick, never only the whole-run mean, and read it against `perturb` as well as `seed`. Mean |Δ| rewards a universe with less macro variance (Jensen), which is why fidelity is a vector of observables and `D`, not one number.

**The speed cap changes the law.** Radius 3 with the shipped bands is a larger-than-life rule at occupancy ~0.23; radius 1 is Conway at ~0.06. Say so whenever the speed cap is compared to the reference.

**Cost is quantised and `predict_work` is exact.** Lazy rendering charges by the block; `scale_probe` rescales the probe with the world; `fit_spec` scans because cost is not monotone in size. A test asserts equality between `predict_work` and what a run spends; if that weakens to an inequality the budget check has become a guess. A layer of the chain *is* `experiment::run` on a smaller config — nothing about nesting reaches it, and the size control pins that.

**Detection standing is declared before data.** `detector::standing` says, per (limit, statistic, gaze), whether a result is a consequence, a finding or exploratory. A new statistic gets its standing there first. Calibration and evaluation seeds are disjoint; the negative control (two all-limits universes) must stay quiet; the resample control says whether a lazy detector detects laziness or approximation.

**Inhabitants and probes are in base cells.** The inhabitant window scales with the subdivision like the probe does; v0.9's did not, and it produced a false detection of discrete space (audit §2.5).

## Non-negotiables

**Physics is pure.** `physics::step` takes state and returns state; no mutation, no interior mutability, no logging, no clock, no I/O. `step_does_not_mutate_its_input` checks it.

**Determinism: same seed → same universe, on every target.** All randomness through `rng::Rng`; positional `derive` for anything keyed by place and time; never `rand`, thread-local RNGs, `SystemTime`, or hash-map order. No transcendental functions in anything that feeds the fingerprint (`band_probability` is multiplications for this reason). If a deliberate physics change moves `GOLDEN_FINGERPRINT`, compute it natively, confirm it with `wasm-pack test`, update the constant, record the old value in `golden.rs`, and say so in the commit.

**Every number is a claim with a provenance.** A result lives in an artifact with a `metadata.json`, in the ledger with a range, and in the README with its id. Distinguish counters (reproducible anywhere) from wall time (never asserted). Memory is reported twice on purpose (`peak_live_bytes` vs `allocated_bytes`).

**Every finding has a null and a replication.** No statistic computed over a filtered subset without a sample-size guard (`MIN_CORRELATION_SAMPLES`); no detection without a false-positive rate; no "is this different" test without an absolute floor and a control that should fail.

**Mutual blindness is a type.** `WriteEnd` has `write` and `seal` and nothing else; nothing converts a `ReadEnd` back. Do not add a read method, a receipt, or a `&mut` accessor. `Message` fields are private and only the encoded payload is stored.

**Module docs state their theory and what would falsify it within the model.**

**Resolution-independent rules.** `Rules` is density bands; at radius 1 the defaults are B3/S23 (tested). The optional rule table is radius-1 only and never read from a config.

**Fair comparisons.** `World::seed` draws at base resolution and upsamples. Probes and inhabitants are placed in base cells.

**Dependencies stay minimal.** `serde` + `toml` in the core; `wasm-bindgen` in the bridge; the analysis layer may use pandas, scipy and matplotlib but `ledger.py` uses the standard library only so CI needs no venv.

**Nothing models a black hole.** The channel is a horizon and a pipe; the analogy is a physical hypothesis in `docs/philosophy.md`.

## Findings so far (v1.0)

Recorded because they are results of the model; each has a ledger id.

- Every limit but discrete time moves the seven macro observables far beyond a reseed (`D` 9 / 62 / 21 / 116 vs 1.5); discrete time is within the reseed null (`D` 1.38 vs 1.46, divergence 0.93× a reseed's, above it in 2/20). Not "free": not distinguishable at n = 20. (T1-FND-001/002)
- The speed cap is a change of law (occupancy 0.23 → 0.06). Lazy rendering's visibility is the closure's: frozen matches a reseed's divergence while `D` = 11. Speed × lazy interact (10.9 ± 2.2). (T1-FND-004/005/006)
- Chains end on budget, space or block quantisation; a layer is a standalone universe of its size; churn follows the resolved share of blocks, and "the deepest layer is calmer" holds in 0/20 under the binomial closure. (T2-CON-002/003, T2-FND-001 withdrawn)
- At full width the horizon carries 1.4 bits about itself, 0.5 about the child, 0.3 ten ticks ahead; two uniform bits carry 0.00 at the shipped placement; an adaptive four bits carry 1.9. (T3-FND-001/002)
- Lazy rendering is findable by a rendering inhabitant via `edge_excess` (power 19/20, FPR 1/20, exploratory) — and so is the resample control (20/20): the boundary approximation is what is detected. Negative control ceiling 0.10. (DET-FND-001/002)
- Bootloader tracker false-positive floor 5%; density rises down the chain (7.3 → 10.8 → 13.0 per 1000 cells) and matches standalone same-size worlds. Gate never fires under shipped floors; 16/20 under permissive, sole stop 7/20. (T5-FND-001/002/003)
- In the baseline universe productive laws are a minority under every prior and criterion, from 0.055 to 0.446: a factor of eight that is the prior's and the criterion's. At edge 96 or 40 ticks the growth criterion admits a majority (0.60, 0.63). (T6-FND-001/002/003)
- Rust folds float sums from `-0.0`; normalise with `+ 0.0` before reporting.

**Withdrawn from v0.9** (kept in the ledger as `withdrawn`): the free lunch below the chaos floor; the calmer deepest layer; poorer layers producing less life; looking concealing lazy rendering; 2–6 bits keeping 90%; √2 as a measured finding; 19% productive. See `docs/audit.md` for why.

## Decisions already made — do not relitigate

- **Language: Rust.** Strict compiler, paradigm split (pure physics; layers as structs), one fast binary, WASM.
- **Default closure: binomial.** It is what "mean field" means; the indicator closure stays for reproduction only; results are reported under all three.
- **Dependencies stay minimal.** The RNG is hand-written because `StdRng` is not guaranteed stable across `rand` versions.
- **Target hardware:** MacBook Air M1, 2D toy scale. Three interesting layers beat ten dead ones.
- **Distribution:** public repo, dual `MIT OR Apache-2.0`.
- **Ledger ids are stable.** A withdrawn claim keeps its id and its status; it is not deleted.

## Roadmap

The original roadmap (v0.1–v0.6) and phase 2 (v0.7–v0.9) are complete; v1.0 is the methodological redesign. Open items are in the README's "What remains unresolved": discrete time at higher seed counts, a closure that tracks Life's decay, a preregistered replication of `edge_excess`, a like-for-like window control for the pipe. The research track — a child whose dynamics depend on what its parent's bootloaders did — stays deliberately unscheduled. Finish a milestone before starting the next.

## Vocabulary — use consistently

- **Layer** — one universe in the chain. **Layer 0** is the host process.
- **Horizon / pipe** — the one-way serializing channel between layers. Never "black hole" in code or configs.
- **Closure** — what stands in for an unobserved block: binomial, indicator or frozen.
- **Null** — a reference universe altered in a way that is not a limit: reseed, perturb, density, shuffle.
- **D** — root mean square of the seven observables' z-scores against seed-to-seed spread.
- **Standing** — consequence / finding / exploratory, declared per detection statistic before data.
- **Logging threshold** — minimum aggregate scale at which a parent notices; `report.macro_grid`.
- **Degradation rule** — each child's budget is a strict fraction of its parent's.
- **Bootloader** — a cluster that persists and travels; a precondition, not an achievement.
- **Probe / observation** — the event that forces full-resolution computation of a region.
