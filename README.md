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
cargo run --release -- pipe --config configs/pipe.toml      # Theory 3: what survives the crossing
cargo run --release -- detect --config configs/detect.toml  # Detection: which limits are findable
cargo run --release -- sweep  --config configs/sweep.toml   # Theory 6: how narrow the band is
```

The `run` experiment takes about 11 seconds on an M1 Air; `nest` and `pipe`
take under a second each. `configs/quick.toml`
is a much smaller world for iterating on code — too short to draw conclusions
from.

```sh
cargo test                          # 184 tests, most of them on the physics
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

## v0.3: the pipe

Theory 3 says a black hole is a one-way serializing channel: content structure
is destroyed, but *timing and magnitude* may survive. Both halves are testable,
and they pull in opposite directions — a channel that preserved everything
would not be a pipe, and one that preserved nothing would carry no signal.

```sh
cargo run --release -- pipe --config configs/pipe.toml
```

```
horizon 48x48 at (48, 48): 2304 bits of content per tick, 128 bits transmitted
the channel carries 5.56% of what a faithful description would need

content structure: 50.2% of digest bits flip when one cell changes
timing and magnitude: correlation 0.7884 between what crossed and what the child was doing

 threshold   registers    events   correlation
      0.00      100.0%       300        0.7884
      0.10       35.3%       106        0.5886
      0.15       16.7%        50        0.2911
      0.20        4.7%        14        0.5536
      0.30        0.7%         2  too few (<5)
      0.50        0.0%         0  too few (<5)
```

**Content did not survive.** Changing one cell of the horizon flips 50.2% of
the digest's bits — the signature of a hash. No amount of comparing what came
out recovers the arrangement that went in.

**Timing and magnitude did.** What crossed tracks the child's global behaviour
at 0.79, through a channel carrying 5.6% of the information. Theory 3's split
survives contact with an implementation: the pipe destroys the *what* while
preserving the *when* and the *how much*.

**Mutual blindness is enforced by the compiler, not by convention.** The child
holds a `WriteEnd`, which has `write` and nothing else — no method returns
anything about the far side, so a universe on that end cannot discover it is
being read, or by what. `WriteEnd::seal` consumes it to produce the `ReadEnd`,
and there is no path back. In a project that chose Rust because a strict
compiler substitutes for human language expertise, this seemed like the right
thing to make unrepresentable rather than merely documented.

**The logging threshold makes a child vanish.** At a threshold of 0.10 barely a
third of the child's history registers; past 0.50, nothing does — not quietly,
not in aggregate, not at all. This is Theory 4's uncomfortable idea made
concrete: a parent watching a dashboard at the wrong resolution is not hostile
or absent, just tuned past you.

**A trap worth naming.** The first version of that table reported a correlation
of **1.0000** at threshold 0.30, which looked like the strongest result in it.
That row had two data points, and Pearson on two points is always exactly ±1.
The sweep now reports the event count beside every row and refuses to print a
correlation below five, because a high threshold admitting a handful of events
is exactly the situation that manufactures perfect correlations out of noise.

## v0.4: detection

The milestone where the model argues against itself. v0.1 established that the
creator's limits are worth having. This asks whether they are *findable* by
something with no access to anything outside its own universe.

```sh
cargo run --release -- detect --config configs/detect.toml
```

An inhabitant is not an agent — it is a measuring apparatus with an honest
access restriction. It reads its own region of its own world, one tick at a
time, through the same `sample` every cell uses. It never sees a `Constraints`,
and cannot look at a second universe for comparison.

```
                            signal        with     without       verdict
discrete_space         min_feature      1.0000      1.0000     invisible
speed_cap          influence_speed      1.0000      3.0000         found
discrete_time      influence_speed      1.0000      2.0000         found
lazy_rendering          smoothness      0.0002      0.0001     invisible
```

**Pixelation leaves no fingerprint.** An inhabitant measures in cells because it
is made of them, so subdividing space leaves its ruler exactly where it was. The
smallest distinguishable separation is one unit in every universe, and always
will be.

**The speed of influence is measurable** — count how far new life appears from
anything that was alive the tick before. But **that number is a product**,
`radius × substeps`, and no amount of measuring it more carefully will factor
it. Three substeps of radius one and one substep of radius three read
identically. The v0.1 coupling comes back here as a limit on what can be
*known*, not merely on what can be built.

**Looking is what conceals lazy rendering.** The framework defines a probe as
the event that forces full-resolution computation of a region — so an inhabitant
examining its surroundings *is* a probe, and renders what it looks at. Run the
same inhabitant on the same coarse frontier both ways and the contrast is exact:
smoothness 0.0002 when its looking renders, 0.0108 when it can somehow read
without rendering. What hides this limit is not distance or subtlety. It is that
observing without observing is a contradiction, so the limit is hidden in
principle.

**Two guards this needed.** A relative-difference test alone called 0.0002
against 0.0001 a fifty percent difference and reported a detection built
entirely from noise; detections now need an absolute floor as well. And an
earlier test asserted that a generous speed bound goes unreached — true at one
inhabitant placement, false at another. The ceiling is the invariant; saturation
is a local observation and is reported as one.

**What none of this shows.** An inhabitant cannot learn from any of this that it
is simulated. It learns which of its own laws have the shape of an optimization,
which is the most the model allows anyone on the inside to know.

## v0.5: the fine-tuning sweep

Theory 6 says only narrow bands of a universe's constants produce complexity.
That is usually deployed as an argument for design; here it is something to
measure. Sweep the rule's density bands, score what each setting produces, and
report what share of the space is worth inhabiting.

```sh
cargo run --release -- sweep --config configs/sweep.toml --steps 21
```

```
  survive
   0.050 |~########............
   0.140 |#::::####............
   0.260 |:::::::::####........
   0.320 |:::::::::####........
   0.380 |#::::::::............
   0.530 |~####::::
   0.650 |~####::::
         +---------------------
          birth
          0.05             0.65

  # complex   : near   ~ chaotic   . frozen   @ saturated   (blank) empty

19.0% of the swept area produced a complex universe (84 of 441 settings)
but those 441 settings denote only 42 distinct laws, of which 8 were productive
```

**The productive band is a minority, not a sliver.** 19% of the laws this sweep
can reach produce something complex. Fine-tuning holds here, but in a weaker
form than the argument usually assumes — a creator picking blindly would find an
interesting universe about one time in five.

**Area is the resolution of the sweep; laws are the resolution of the
universe.** 441 grid settings denote only 42 distinct rules, because a
neighbourhood of eight cells only ever has densities `k/8` and nudging a band
centre usually changes nothing. Reporting the area fraction alone would have
described the sweep's own granularity and called it a property of physics.

**Chaos is not complexity, and the first bar could not tell.** Complexity sits
*between* order and chaos, so every criterion has to be a band rather than a
floor. The first version required activity above a minimum — which admitted the
rules that churn hardest, one of them at twenty times Conway's activity, a world
rewriting itself completely every tick. Wolfram's class 3 sailed in as class 4.

**Raw variance is nearly a function of density.** The first structure measure
ranked a regular blinking tiling above Conway. Dividing by what uncorrelated
noise of the same density would give removes the density dependence: 1 is
chance, above is clumped, below is more even than chance.

**What this does not show.** The bar is calibrated from Conway, so "productive"
means *resembling the one setting already believed interesting*. It is a measure
of resemblance, not of worth. Two constants were swept out of the many a
universe has, and the band widths were held fixed.

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
| `pipe` | The one-way serializing channel; the horizon and the logging threshold | 3, 4 |
| `detector` | Whether an inhabitant can find the limits from inside | 1, 4 |
| `sweep` | Fine-tuning: how narrow the productive band of constants is | 6 |
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

Done: **v0.1** limits as optimizations, **v0.2** nesting and degradation,
**v0.3** the pipe, **v0.4** detection, **v0.5** the fine-tuning sweep.

Next:

1. **Emergence** — bootloader life. The hardest and slowest; emergence cannot
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
