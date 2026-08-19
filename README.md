# the-universe

[![CI](https://github.com/modirniya/the-universe/actions/workflows/ci.yml/badge.svg)](https://github.com/modirniya/the-universe/actions/workflows/ci.yml)

Open-source **executable philosophy**: a runnable model of a
simulation-hypothesis framework developed by Parham Modirniya. The codebase
*is* the argument. Each module implements one theory, and a successful run
demonstrates the framework's internal coherence.

## What this proves, and what it does not

Running this proves the ideas are **coherent** — that they can be made to work
together in a system that actually executes and produces measurable results. It
does **not** prove our universe works this way, and no result here should be
read as evidence that it does. The model makes falsifiable predictions only
about its own behaviour.

Philosophy that cannot be coded lives in [`docs/philosophy.md`](docs/philosophy.md),
not in the code.

## The question v0.1 asks

Theory 1 of the framework says physical limits are not fundamental truths but
**resource optimizations** — decisions a creator made to run a universe
cheaply. Discrete space and time, a speed of light, and detail rendered only
where something is looking are the three an engineer would reach for first.

That is testable inside a toy. Build a small universe, run it with the limits
in force and without them, and ask: *do the creator's limits make a universe
cheaper without changing what it produces?*

## Quick start

```sh
cargo run --release -- run  --config configs/default.toml   # Theory 1: what the limits cost
cargo run --release -- nest --config configs/nesting.toml   # Theory 2: how deep a chain gets
```

The `run` experiment takes about 11 seconds on an M1 Air; `nest` takes under a
second. `configs/quick.toml`
is a much smaller world for iterating on code — too short to draw conclusions
from.

```sh
cargo test                          # 108 tests, most of them on the physics
cargo run --release -- --help
```

## Findings

Reproduce with the quick-start command above. Everything below came from that
command on an Apple M1, seed 42, 128×128 base cells, 200 ticks.

```
reference universe (no limits): 65536 cells, 1258291200 neighbour visits, 3454 ms
control (same universe, different seed): macro floor 0.08065, occupancy floor 0.01299

limit          work     time   memory  macro div   vs floor   occup div
-----------------------------------------------------------------------
space        0.250x   0.246x   0.257x    0.12319      1.53x     0.04505
time         0.500x   0.498x   1.000x    0.07566      0.94x     0.01557
speed        0.167x   0.193x   1.000x    0.16157      2.00x     0.15968
lazy         0.250x   0.259x   0.257x    0.20672      2.56x     0.17935
all_on       0.005x   0.009x   0.071x    0.27727      3.44x     0.09716
```

**Read the divergence column against the floor, not against zero.** This world
is chaotic, so any perturbation decorrelates it. To calibrate, the harness runs
the unconstrained universe a second time changing nothing but the seed. Those
two are unquestionably the same *kind* of universe, and the divergence between
them (`chaos floor`) is what chaos alone produces. Only divergence above the
floor is a limit showing through. Without this control the numbers would be
uninterpretable, and every limit would look damning.

Three things came out of it:

**All limits together make the universe ~190× cheaper in work and ~14× smaller
in memory.** Theory 1's core claim survives contact with an implementation: the
limits are enormously worth having. A creator would take this deal without
thinking about it.

**Discrete time is the free lunch.** It halves the cost and diverges *below*
the chaos floor (0.94×) — that is, refining time changes the universe less than
changing the seed does. If a creator wanted one limit that inhabitants could
never detect, this is it.

**The other three are not free, and the model says so.** Lazy rendering is the
most visible at 2.56× the floor, which stands to reason: mean-field
approximation of unobserved regions is a real loss of fidelity, and it shows up
as a universe that holds substantially less structure (occupancy 0.05 against
the reference's 0.22). Cheapness and invisibility are separate properties, and
the framework does not get to assume they come together.

**One result was not designed in.** Discrete time and the speed cap turn out to
be *coupled*: influence covers `radius × substeps` cells per tick, and a cell is
`1 / subdivision` of a base length, so refining time without refining space
raises the physical speed of influence. A creator cannot relax one of these
limits without paying in another. That falls out of the model rather than
having been assumed by it. See `constraints::Resolved`.

### On the numbers

Work ratios, divergences and cell counts are **reproducible on any machine** —
they are counters, not measurements. Wall time is **not**; it is reported
because it is what a creator would actually pay, and the M1 Air is fanless, so
long runs throttle. Memory is reported twice for the same reason: `peak_live_bytes`
is what a resource-honest implementation would hold, `allocated_bytes` is what
this one really allocates. Claiming the smaller number as measured RSS would be
a lie, and claiming the larger one would hide what the optimization is for.

Full output lands in `out/runs.csv` and `out/report.json`.

## v0.2: nesting and degradation

Theory 2 says a universe can host a child, but only on a fraction of its own
resources — so the chain degrades and has a maximum depth. Reproduce with:

```sh
cargo run --release -- nest --config configs/nesting.toml
```

```
root budget 6630400 work units, each child gets 25% of its host

depth        world       budget          spent      used      churn     state
    1      128x128      6630400        6630400    100.0%    0.10835      live
    2        64x64      1657600        1657600    100.0%    0.01018      live
    3        21x21       414400         414400    100.0%    0.00659      live

the chain terminated at depth 3; the closed form allowed at most 4
every layer stayed inside the budget its host gave it
total cost 8702400 against a geometric bound of 8840533
```

The root is given exactly what its own world costs, so nothing about the depth
is chosen — it falls out of the rule.

**The chain is finite, and cheap.** Budgets form a geometric series, so however
deep a chain runs it costs the host less than `1 / (1 - fraction)` times the
root layer alone — here 1.33×. Nesting is bounded in total spend, not just in
depth, which is the more interesting half: a parent can host a whole chain
without the chain eventually costing more than the parent.

**It died of space, not of money.** The closed form allowed four layers; the
chain stopped at three, because the fourth world would have been smaller than
one block. Two termination conditions exist and the spatial one bit first.

**Degradation is visible as declining activity.** Churn — mean tick-to-tick
change in the macro field — falls by an order of magnitude per layer. It is
worth noting this runs *against* the measurement's own bias: churn is taken on
a fixed 16×16 macro grid, so a smaller world averages over fewer cells and
should look noisier, not calmer. The decline is real, and if anything
understated.

**Cost is not monotonic in world size.** A 48×48 layer costs 3,686,400 while a
64×64 one costs 1,657,600. Lazy rendering charges by the block, and the probe
is rescaled with the world, so a probe landing on block boundaries resolves far
fewer blocks than one of the same area straddling them. Sizing a layer is
therefore a scan, not algebra — walking down from an area estimate would step
straight past larger worlds that also fit.

**Integer truncation costs the chain depth.** Budgets are integers and each
generation is floored, so real chains come up short of the ideal geometric
prediction — with `fraction = 0.75` and a root of 1000 against a viable minimum
of 100, the closed form says 9 and the chain that builds is 8. `max_depth` is
an upper bound, not an equality, and is documented as one.

**What this does not model.** Layers cannot reach each other. The one-way
serializing channel between them is v0.3, so the mutual blindness here is an
omission rather than a claim.

## Theory → module map

Each module's docs state which theory it implements and what would falsify it
*within the model*.

| Module | Implements | Theory |
| --- | --- | --- |
| `constraints` | The four limits, as toggles, and the dials behind them | 1 |
| `space` | Discrete space; two-fidelity storage (cells + block densities) | 1 |
| `physics` | The laws, as pure functions over immutable state | — |
| `observer` | Probes; the render and collapse events | 1 |
| `rng` | The creator's runtime input channel | 9 |
| `experiment` | The ON/OFF benchmark and its control | 1 |
| `budget` | The degradation rule; what a layer may spend | 2 |
| `layer` | Nesting: layers hosting layers, each poorer than its host | 2 |
| `report` | CSV, JSON, and a summary that declines to overstate the result | 4 |

The macro grid that `report` compares runs on is the **logging threshold** from
Theory 4 — deliberately a parent's-eye view, since it is the only fair
comparison between worlds running at different internal resolutions.

## How the toy works

A 2D grid cellular automaton on a torus. The rule is life-like but written as
**density bands** rather than neighbour counts, so the same law survives a
change of resolution; at radius 1 on a Moore neighbourhood the default bands
reduce exactly to Conway's B3/S23, which the tests check with a blinker, a
block and a glider.

The world is stored at two fidelities at once. Cells are authoritative inside
**resolved** blocks; a single density is authoritative inside unresolved ones.
A block is resolved only while the probe observes it, and neighbouring cells
read an unresolved block as a density rather than as detail — so the cost
saving is real, and so is the error it introduces.

Two events matter. A **render** happens when a block enters observation: it has
no cells, only a density, so cells are drawn from that density through the
seeded RNG. Detail that was never computed is committed to at the instant it is
looked at. A **collapse** happens when a block leaves observation: its cells are
summarised to a density and stop being computed.

In v0.1 the probe is a **fixed window** — the least interesting probe on
purpose, because holding observation constant keeps the cost difference
attributable to the optimization rather than to the observer wandering about.

## Determinism

Same seed, same universe. All randomness flows through one seeded RNG
(`src/rng.rs`, SplitMix64, written out in full rather than pulled from a crate
so the value stream stays identical across machines and future targets).

This is philosophically load-bearing, not just hygiene: the seed is the
creator's only necessary intervention, supplied from outside the universe in a
config file. See Theory 9 in the philosophy doc.

Rendering uses *positional* sub-streams derived from block coordinates and
tick, not draws from a shared stream, so a region renders identically no matter
what else was rendered first. Physics never touches a clock, a hash map's
iteration order, or a thread-local RNG.

## Configuration

```sh
the-universe run --config <FILE> [--out <DIR>] [--seed <N>] [--ticks <N>]
```

`configs/default.toml` is commented field by field. The dials worth turning:
`params.subdivision` and `params.substeps` (how much finer the unconstrained
universe is), `params.capped_radius` / `uncapped_radius` (the speed of light),
`params.block_size` (granularity of lazy rendering), and `observer` (what gets
looked at).

## Roadmap

Done: **v0.1** limits as optimizations, **v0.2** nesting and degradation.

Next, in order:

1. **Pipe** — the one-way serializing channel between layers; whether timing
   and magnitude survive when content does not.
2. **Detection** — agents inside a layer trying to determine, from within,
   whether they are running under limits.
3. **Fine-tuning sweep** — how thin the band of complexity-producing constants
   actually is.
4. **Emergence** — bootloader life. The hardest and slowest; emergence cannot
   be scheduled.

Also later, not scoped: a Python notebook shell for analysing experiment
output, visuals, and a WASM build so strangers can run a universe in a browser
tab.

## Building

Rust, single crate, two direct dependencies (`serde` and `toml`, both only for
reading the config file). The RNG is written out in full rather than pulled in. Rust was chosen because a strict compiler substitutes for human language
expertise in an AI-built, AI-consumed codebase; because it supports the
paradigm split the design depends on (physics as pure functions, layers as
things with identity and lifecycle); because it produces one fast binary; and
because WASM is a plausible later target.

```sh
cargo build --release
cargo test
cargo clippy --all-targets -- -D warnings
```

## Licence

Dual licensed under [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE), at your
option. You pick one and comply with that one; you do not have to satisfy both.

This is the Rust ecosystem convention. Apache 2.0 carries an explicit patent
grant, which MIT lacks; MIT stays compatible with GPLv2, which Apache 2.0 is
not. Offering both means neither constituency is locked out.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

See [CONTRIBUTING.md](CONTRIBUTING.md) for the rules that are load-bearing —
determinism, pure physics, and the fact that every performance claim here has
to be reproducible by a command in this repo.
