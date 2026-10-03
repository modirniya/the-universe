# the-universe

[![CI](https://github.com/modirniya/the-universe/actions/workflows/ci.yml/badge.svg)](https://github.com/modirniya/the-universe/actions/workflows/ci.yml)

**[▶ Watch it run](https://modirniya.github.io/the-universe/)** — one universe
of the model, in a browser tab, with its optimizations switched on and off by
hand. A visualisation, not an experiment: no number on that page is a result.

An open-source **computational investigation of a simulation-hypothesis
framework**, by Parham Modirniya. The framework (in
[`docs/philosophy.md`](docs/philosophy.md)) says physical limits are resource
optimizations, universes nest on degrading budgets, horizons are one-way
channels, inhabitants cannot see out, life boots the next layer, and only a
narrow band of laws is productive. This repository builds a toy universe in
which each of those is a definition or a parameter, runs experiments with
controls and replication, and reports what follows from the definitions, what
had to be run, what came out, and what did not.

## What this is, and is not

It is not evidence that our universe is simulated, and nothing in it should be
read that way. Every number here is a fact about a program. The model makes
falsifiable predictions only about its own behaviour, and several of the
predictions its earlier versions made about itself turned out to be wrong (see
[What v1.0 withdrew](#what-v10-withdrew)).

It is a model whose assumptions are explicit, whose experiments are
reproducible, whose statistics carry their uncertainty, and whose conclusions
are sorted into four kinds that are never mixed:

| Kind | Meaning | Established by |
| --- | --- | --- |
| **Definition** | Deliberately built into the model. | Reading the code. |
| **Consequence** | Follows necessarily from the definitions. | A derivation ([`docs/derivations.md`](docs/derivations.md)). |
| **Computational finding** | Could have come out otherwise when the model ran. | An experiment with controls, replication and an uncertainty. |
| **Physical hypothesis** | An interpretation concerning our universe. | Nothing in this repository. |

Every claim is listed in [`analysis/claims.toml`](analysis/claims.toml) with its
kind, the module and experiment it rests on, its seeds, its controls, the commit
it was last verified at, and the artifact ranges that must hold.
`analysis/ledger.py` checks the ledger against the README and the artifacts, and
CI runs it on a fresh reproduction. Claim ids below (`T1-FND-002`, …) point into
that ledger.

## Reproduce everything

```sh
scripts/reproduce.sh            # every published table; about an hour on four cores
scripts/reproduce.sh quick      # the cheap commands only
python3 analysis/ledger.py      # the ledger against what was just produced
```

Each output directory under `out/` gets a `metadata.json` naming the commit,
whether the tree was clean, the compiler, the target, the exact configuration,
its fingerprint, the seeds and the experiment's design version. Counters —
work, divergences, bits, counts — are identical on any machine. Wall-clock
columns are not and are never asserted on.

Individual commands:

```sh
cargo run --release -- run     --config configs/default.toml   # Theory 1: 2^4 factorial, 4 nulls, 7 observables; ~7 min
cargo run --release -- nest    --config configs/nesting.toml   # Theory 2: chain, termination map, size control; ~2 min
cargo run --release -- pipe    --config configs/pipe.toml      # Theory 3: bits per task per encoding; ~1 min
cargo run --release -- detect  --config configs/detect.toml    # Detection: calibrated tests, two controls; ~30 s
cargo run --release -- boot    --config configs/boot.toml      # Theory 5: chain, gate ablation, shuffle and area controls; ~5 min
cargo run --release -- boot    --config configs/boot-permissive.toml
cargo run --release -- edge    --config configs/edge.toml      # Theories 2 and 5: why chains end; ~5 min
cargo run --release -- sweep   --config configs/sweep.toml     # Theory 6: the original grid sweep; ~3 min
cargo run --release -- measure --config configs/measure.toml   # Theory 6: five priors, six universes; ~35 min
cargo test --workspace                                        # 230 tests
```

Times are for four cores of a 2026 x86_64 container; the pinned seed runs
alone and the rest in parallel. Every config runs 20 seeds (detection 40: 20 to
calibrate, 20 to evaluate; the edge map 10). A seed selects an initial
condition and nothing else (`GEN-DEF-001`), so every interval below is an
interval over initial conditions of one configuration — not over sizes, probes,
rules or closures, which the experiments vary separately. Twenty is small; the intervals are
printed so that it stays visible, and "20/20" is written with its Wilson
interval, [0.84, 1.00].

## Results in brief

What had to be run, and what came out. Full tables follow in the sections
below; `verified` and `exploratory` are ledger statuses.

| Finding | Status | Result |
| --- | --- | --- |
| Every limit but discrete time moves the macro observables far beyond a reseed (`T1-FND-001`) | verified | D = 9 (space), 62 (speed cap), 21 (lazy), 116 (all four) against 1.5 for a reseed; above the reseed's divergence in 20/20 seeds. |
| Discrete time is within the reseed null (`T1-FND-002`) | verified | D 1.38 ± 0.60 vs 1.46 ± 0.78; divergence 0.93 ± 0.05 of a reseed's. Not distinguishable at n = 20 — which is not "free". |
| The speed cap is a change of law (`T1-FND-004`) | verified | Occupancy 0.23 → 0.06; components +146 sd. The reference is a radius-3 automaton, the capped universe is Life. |
| Lazy rendering's visibility is the closure's (`T1-FND-005`) | verified | Frozen: divergence equals a reseed's yet D = 11. Indicator: D = 24. Binomial: D = 21. |
| The limits interact (`T1-FND-006`) | verified | Speed cap × lazy on D: 10.9 ± 2.2. |
| Chains end on budget, space, or block quantisation (`T2-CON-002`) | consequence | Mapped over fractions, floors, block sizes and root sizes with no physics run. |
| A layer is a standalone universe of its size (`T2-CON-003`) | consequence | Nothing about nesting reaches it; churn follows the resolved share of blocks, not depth. |
| At full width the horizon carries 1.4 bits about itself, 0.5 about the child (`T3-FND-001`) | verified | 0.32 bits ten ticks ahead; 0.22 about which quadrant is densest. |
| What a narrow channel keeps depends on the encoding and the placement (`T3-FND-002`) | verified | Two uniform bits carry 0.00; an adaptive four bits carry 1.9. |
| Lazy rendering is findable by an inhabitant whose looking renders (`DET-FND-001`) | exploratory | Power 19/20 at FPR 1/20 by the edge-birth statistic — but the resample control fires 20/20: it is the boundary approximation that is detected. |
| The negative control is quiet (`DET-FND-002`) | verified | Highest power on two all-limits universes: 0.10. |
| The tracker's false-positive floor is 5% (`T5-FND-001`) | verified | 0.35 vs 7.3 bootloaders per thousand cells; real above shuffled in 20/20. |
| Bootloader density rises down the chain (`T5-FND-002`) | withdrawn | 7.3 → 10.8 → 13.0 per thousand cells, matched by standalone worlds of the same size. The count fell because the world shrank. |
| The gate never fires under the shipped floors; fires in 16/20 under permissive ones (`T5-FND-003`) | verified | Sole stop in 7/20; in the edge map sterility alone binds in at most 2 of 10 seeds per cell. |
| Productive laws are a minority under every prior and criterion (`T6-FND-001`) | verified | From 0.055 ± 0.035 to 0.446 ± 0.037. |
| The share is the prior's and the criterion's as much as the laws' (`T6-FND-002`) | verified | A factor of eight. "Fine-tuning is X%" means nothing here without both named. |
| Same seed, same universe, on three targets (`GEN-FND-001`) | verified | Fingerprint `8151296893090507384`, native and WebAssembly. |

### What v1.0 withdrew

An audit of v0.9 ([`docs/audit.md`](docs/audit.md)) found that several of its
findings were artifacts of how the model was implemented or measured. They are
kept in the ledger as `withdrawn`, with what replaced them:

- **"Discrete time diverges below the chaos floor in 20/20 seeds — the free
  lunch."** (`T1-FND-003`) The floor was a different-seed control; a same-seed
  comparison starts correlated, and the whole-run mean was dominated by that
  transient. Over the second half discrete time reads 0.93× a reseed, and its
  divergence is low partly because its macro variance is lower, which the
  metric rewards. Replaced by `T1-FND-002`.
- **"The deepest layer is calmer than the root in 20/20 seeds."**
  (`T2-FND-001`) Under the mean-field closure v0.9 shipped, unobserved ground
  oscillated between near-empty and near-full. Under a binomial closure the
  claim holds in 0/20 seeds, and churn is a function of how many blocks the
  rescaled probe resolves at each size.
- **"Poorer layers produce less life: 128 → 32 → 6."** (`T5-FND-002`) Per
  thousand cells the density rises; the count fell with the area.
- **"Looking conceals lazy rendering."** (`DET-FND-001`) The cells at the edge
  of what an inhabitant renders have neighbours that are densities, and the
  birth rate there gives it away with power 0.95. What is detected is the
  approximation, as the resample control shows, but it is detected.
- **"Two to six bits a tick keep 90% of what crosses."** (`T3-FND-002`) True of
  one encoding at one horizon placement; at the placement the config describes,
  two uniform bits carry nothing.
- **"The lattice's shape is visible from inside, measured in 20/20 seeds."**
  (`DET-CON-002`) √2 is the geometry of a Moore neighbourhood at every scale; it
  is a consequence, and the only measured part is that births reach the corner.
- **"19% of laws are productive."** (`T6-FND-001`) One prior, one criterion, one
  seed. The range across priors and criteria is 5.5% to 45%.

## How the toy works

A two-dimensional cellular automaton on a torus. The rule is life-like but
written as **density bands** rather than neighbour counts so that it survives a
change of resolution; at radius 1 the shipped bands are Conway's B3/S23, which
tests check with a blinker, a block and a glider. The four limits are toggles
on this one automaton (`T1-DEF-001`): discrete space (one or two fine cells per
base cell), discrete time (one or two substeps per tick), the speed cap (radius
1 or 3 — which changes the neighbourhood and therefore the law), and lazy
rendering (unobserved blocks held as a density).

The world is stored at two fidelities. Cells are authoritative inside
**resolved** blocks; a density is authoritative inside unresolved ones, and
physics reads neighbours through one `sample` function that returns whichever
applies. A block is resolved only while a probe observes it. **What stands in
for an unobserved block is an assumption** (`T1-DEF-002`): three closures are
implemented — *binomial* (the rule averaged over a binomial neighbour count,
the default), *indicator* (what v0.1–v0.9 shipped, which saturates a block whose
neighbours sit in the birth band) and *frozen* (the density is held) — and
every lazy-rendering result is reported under each.

Two events matter. A **render** happens when a block enters observation: cells
are drawn from its density through a positionally seeded generator, so a region
renders identically whatever was rendered first. A **collapse** happens when it
leaves: its cells are summarised and stop being computed. The probe is a fixed
window, held constant so that cost differences are the optimization's and not
the observer's.

## Theory 1: limits as optimizations

The question is whether a creator's limits make a universe cheaper *without
changing what it produces*. The experiment runs all sixteen settings of the
four toggles, measures seven macro-scale observables over the second half of
each run (occupancy, churn, dispersion, spatial correlation, compressibility,
component density, entropy), and judges every setting against four **null
models** — a reseed, one base cell flipped at tick 0, the initial density
nudged by 0.01, and the reference's own macro field shuffled — plus the 190
pairwise divergences between the twenty references. Fidelity is a distance
`D`: the root mean square of the seven observables' z-scores against their
seed-to-seed spread. A reseed scores about √2 by construction; `D` is read
against that.

```
null       div half: mean ± sd [95% ci]        D: mean ± sd [95% ci]
seed       0.08817 ± 0.00495 [0.08586, 0.09049]  1.46 ± 0.78 [1.10, 1.83]
perturb    0.08890 ± 0.00538 [0.08638, 0.09142]  1.59 ± 0.86 [1.18, 1.99]
density    0.08714 ± 0.00443 [0.08507, 0.08922]  1.43 ± 0.82 [1.04, 1.81]
pairwise across 190 reference pairs: div half 0.08777 ± 0.00398

setting        work  memory  D: mean ± sd [95% ci]        div half / reseed      > reseed
space        0.250x  0.257x    9.15 ± 2.56 [7.95, 10.35]  1.41 ± 0.15           20/20 [0.84, 1.00]
time         0.500x  1.000x    1.38 ± 0.60 [1.10, 1.66]   0.93 ± 0.05            2/20 [0.03, 0.30]
speed        0.167x  1.000x   62.39 ± 1.92 [61.49, 63.29] 1.95 ± 0.17           20/20 [0.84, 1.00]
lazy         0.250x  0.257x   21.35 ± 1.09 [20.84, 21.86] 2.33 ± 0.18           20/20 [0.84, 1.00]
all_on       0.005x  0.071x  115.92 ± 7.97 [112.19, 119.65] 1.53 ± 0.11         20/20 [0.84, 1.00]

lazy@frozen  0.250x  0.257x   11.33 ± 0.44 [11.12, 11.53] 1.00 ± 0.07           10/20 [0.30, 0.70]
lazy@indicator 0.250x 0.257x  24.19 ± 6.46 [21.16, 27.21] 2.31 ± 0.17           20/20 [0.84, 1.00]
```

**The work and memory columns are arithmetic** (`T1-CON-001`,
[derivation §1](docs/derivations.md)): fewer cells, substeps, neighbours or
resolved blocks, multiplying to 1/192 for all four. Their main effects on log
work are exactly additive. So is the coupling between discrete time and the
speed cap (`T1-CON-002`): influence covers `radius × substeps` cells per tick
over cells of size `1/subdivision`, so refining time without refining space
raises the physical speed of influence — a consequence of the definitions,
noticed rather than discovered. Only the fidelity columns had to be run.

**Every limit but one is visible many standard deviations away**
(`T1-FND-001`). The speed cap is the furthest, because it is a different law
(`T1-FND-004`): the reference at radius 3 holds occupancy near 0.23, Conway
decays to 0.06, and the component count per cell rises 146 seed-to-seed
standard deviations. Lazy rendering under the binomial closure sits 21 sd away;
the mean-field fixed point of Life is 0.37 while Life itself decays, and
unobserved ground is held at the one while rendered ground does the other.

**Discrete time is the one setting within the reseed null** (`T1-FND-002`):
`D` 1.38 against 1.46, late-time divergence 0.93 of a reseed's, above the reseed
in 2 of 20 seeds. Its per-observable shifts — churn +1.7 sd, dispersion −1.0,
occupancy +0.9 — are real but unresolvable at twenty seeds. The honest statement
is that these seven observables cannot tell it from a reseed, not that it
changes nothing. That it reads systematically *below* a reseed is itself a sign
of lower macro variance, which the divergence metric rewards (`T1-FND-003`).

**Which closure stands in for unobserved ground decides what lazy rendering
looks like** (`T1-FND-005`). Under the frozen closure its macro divergence
equals a reseed's — the v0.9 metric would have called it invisible — while its
`D` is 11: seven observables see what one did not. The indicator closure gives
24 and the binomial 21. A result about lazy rendering is a result about a
closure until the closure is varied, and this one was.

**The limits interact** (`T1-FND-006`): the speed cap × lazy rendering term on
`D` is 10.9 ± 2.2, because the binomial closure behaves oppositely under the
two laws. No one-at-a-time design could have shown it.

**Cost against fidelity** (`T1-FND-007`). The Pareto set on (work, `D`) is
{all_off, time, space+time, space+time+speed, space+time+lazy,
space+speed+lazy, all_on}. Work saved per unit of `D` is 0.36 for discrete time
and under 0.1 for everything else. A creator who wanted one limit that these
observables would not notice has one; the other three are cheap and seen.

Artifacts: `out/factorial.csv`, `out/divergence_trace.csv`, `out/ensemble.csv`,
`out/ensemble.json`.

## Theory 2: nesting and degradation

That a chain of universes on degrading budgets is finite, how deep it can go,
and what it costs in total are **consequences** (`T2-CON-001`,
[derivation §2](docs/derivations.md)): depth is
`1 + ⌊log(root/floor) / log(1/f)⌋`, a bound because budgets are floored, and
the whole chain costs under `root/(1−f)`, 4/3 of the root at `f = 1/4`. The
shipped chain builds three layers against a closed form of four.

What is not evident from the rules is **which definition ends a chain where**.
Because lazy rendering charges by the block and the rescaled probe lands on
block boundaries differently at every size, cost is not monotone in world size,
and a chain can end on *block quantisation* — a world above the size floor would
fit by area, but none fits by block (`T2-CON-002`). `nest` maps the ending over
degradation fractions, size floors, block sizes and root sizes, running no
physics; each cell is the ending and the depth built:

```
root 128x128, blocks of 16:
  floor\frac  0.10 0.15 0.20 0.25 0.30 0.40 0.50
  edge    2     $2   $3   $3   $4   $4   $5   $7
  edge    8     $2   $3   $3   $4   $4   $5   $7
  edge   16     $2   q3   q3   #4   q4   q5   q6
  edge   24     q1   #2   q2   q2   q2   q3   q3
  $ budget   # space   q block quantisation
```

**A layer is a standalone universe of its size** (`T2-CON-003`). The size
control runs each layer's size, seed and probe as a universe of its own and
reproduces its churn and work exactly — by construction, since nothing about a
layer's host reaches it. So the v0.9 "degradation signature", churn falling
down the chain, was a statement about world size. Under the binomial closure it
inverts (`T2-FND-001`): churn is 0.0053 at depth 1, 0.0062 at depth 2 and
0.0162 at depth 3, and the deepest layer is calmer than the root in 0/20 seeds.
The churn-against-size curve for standalone universes shows why:

```
  edge   resolved      churn: mean ± sd [95% ci]
    16       1.00  0.01219 ± 0.00651
    21       0.25  0.01616 ± 0.00171
    32       1.00  0.01460 ± 0.00352
    64       0.25  0.00618 ± 0.00040
   128       0.25  0.00529 ± 0.00023
```

`resolved` is the share of blocks the rescaled probe resolves: the quantity that
churn actually follows. Degradation in this model is a budget rule and a size
rule; it has no signature in the dynamics beyond what size and the partition
produce.

Artifacts: `out/nesting/chain.csv`, `terminations.csv`, `size_control.json`,
`size_curve.csv`, `ensemble.csv`.

## Theory 3: the horizon as a serializing pipe

A region of the child — the **horizon** — is folded into one message per tick:
a magnitude (its occupancy) and a position-sensitive hash, in a channel of a
stated width. The child holds a `WriteEnd` with `write` and `seal` and nothing
else; nothing converts a `ReadEnd` back. That magnitude and timing cross and
arrangement does not is **how the message was built** (`T3-DEF-001`), and that
the hash carries no occupancy is a property of hashing (`T3-CON-001`). The
module models no black hole: it has no gravity, no physical horizon and no
singularity, and the analogy that inspired it is a physical hypothesis
(`T3-HYP-001`) that lives in `docs/philosophy.md`.

What had to be run is **what information survives, for which task, under which
encoding**. `pipe` measures the mutual information (plug-in, less its shuffle
null) and a leave-one-out predictive information between the parent's symbol
and five targets, for thirteen encodings:

```
encoding        bits   horizon_now   global_now   global_future   densest_quadrant   quadrant_pattern
uniform:1          1    0.00 ± 0.00   0.00 ± 0.00   0.00 ± 0.00      0.00 ± 0.00        0.00 ± 0.00
uniform:2          2    0.00 ± 0.00   0.00 ± 0.00   0.00 ± 0.00      0.00 ± 0.00        0.00 ± 0.00
uniform:3          3    0.64 ± 0.17   0.31 ± 0.14   0.24 ± 0.16      0.12 ± 0.09        0.38 ± 0.11
uniform:4          4    0.19 ± 0.22   0.13 ± 0.11   0.11 ± 0.07      0.05 ± 0.08        0.13 ± 0.15
uniform:6          6    1.42 ± 0.22   0.48 ± 0.23   0.34 ± 0.21      0.22 ± 0.19        0.72 ± 0.18
uniform:128      128    1.44 ± 0.19   0.48 ± 0.23   0.32 ± 0.20      0.22 ± 0.17        0.75 ± 0.16
adaptive:4         4    1.89 ± 0.14   0.50 ± 0.24   0.33 ± 0.20      0.22 ± 0.18        0.78 ± 0.14
hash:6             6    0.00 ± 0.05   0.01 ± 0.03   0.01 ± 0.04      0.00 ± 0.03       -0.00 ± 0.04
projection:6       6    0.09 ± 0.06   0.08 ± 0.06   0.07 ± 0.06      0.06 ± 0.05        0.10 ± 0.06
quadrants:4        4    0.89 ± 0.15   0.53 ± 0.18   0.42 ± 0.16      0.54 ± 0.24        3.22 ± 0.07
```

Corrected mutual information in bits per tick, mean ± sd over 20 seeds; the
horizon's occupancy has 2.0 ± 0.2 bits of entropy at 64 levels and each target
up to 3. The `quadrants` row against `quadrant_pattern` is the encoding reading
itself and is a check on the estimator, not a result.

**At full width the horizon carries 1.4 bits about what it sent, 0.5 about the
whole child now, 0.3 about the child ten ticks ahead and 0.2 about which of its
quadrants is densest** (`T3-FND-001`). **What a narrow channel keeps is the
encoding's and the placement's** (`T3-FND-002`): with the horizon straddling
rendered and coarse ground, as the config has always said it should, one and
two uniform bits carry nothing because their levels never trip; three bits
carry more than four; an adaptive four-bit quantiser carries more about the
horizon than the exact magnitude read at 64 levels. v0.9's "two to six bits
keep 90%" was measured with the horizon wholly inside rendered ground and is
not a general statement.

Theory 4's half of this is a type (`T4-DEF-001`): a child holds a `WriteEnd`
and cannot learn that it is read; a parent sees only what clears the logging
threshold. That a creator might be unaware of or indifferent to its inhabitants
— watching a dashboard whose resolution does not include them — is a reading of
the framework (`T4-HYP-001`), not an output of any run.

A window half a world away carries 0.17 ± 0.09 bits about the child against
the horizon's 0.48 ± 0.23 (`T3-FND-003`, exploratory). The far window lies in
coarse ground, whose density is a closure's output; the difference is between
rendered and unrendered ground, not a mechanism, and a like-for-like control
has not been run. Random symbols carry −0.00 ± 0.04; bit flips at 2%, 10% and
30% take the four-bit channel from 0.13 to 0.11, 0.06 and 0.02 bits.

Artifacts: `out/pipe/information.csv`, `information_ensemble.csv`,
`information.json`, plus the v0.9 correlation sweep in `pipe.csv` and
`widths.csv`.

## Detection

Can an inhabitant — a measuring apparatus reading its own region through the
same `sample` every cell uses, with no access to a config or a second
universe — tell that it is running under limits? v1.0 asks it as a **hypothesis
test**. For each limit, H0 is the universe with that limit relaxed and H1 the
universe with every limit in force; a statistic the inhabitant can compute is
thresholded at the most extreme H0 value over twenty calibration seeds
(expected false-positive rate 1/21), and the false-positive rate and power are
measured on twenty seeds the rule never saw. Two controls run alongside: two
all-limits universes at neighbouring seeds (any power there is power to detect
a seed), and a *resample* universe — fully rendered, with the blocks lazy
rendering would leave unobserved redrawn from their own density every tick.

```
== looking renders ==
limit            statistic          mean H0   mean H1   FPR k/n [95%]      power k/n [95%]    acc  standing
discrete_space   anisotropy          1.4142    1.4142   0/20 [0.00, 0.16]   0/20 [0.00, 0.16]  0.50 consequence
discrete_space   edge_excess         2.1220    1.8111   1/20 [0.01, 0.24]  11/20 [0.34, 0.74]  0.75 exploratory
discrete_time    influence_speed     2.0000    1.0000   0/20 [0.00, 0.16]  20/20 [0.84, 1.00]  1.00 consequence
speed_cap        influence_speed     3.0000    1.0000   0/20 [0.00, 0.16]  20/20 [0.84, 1.00]  1.00 consequence
speed_cap        smoothness          0.0160    0.0001   0/20 [0.00, 0.16]  20/20 [0.84, 1.00]  1.00 finding
lazy_rendering   smoothness          0.0001    0.0001   3/20 [0.05, 0.36]  16/20 [0.58, 0.92]  0.83 finding
lazy_rendering   edge_excess         1.0331    1.8111   1/20 [0.01, 0.24]  19/20 [0.76, 0.99]  0.95 exploratory

negative control (two seeds): highest power 2/20
resample control, lazy rule applied: edge_excess flags lazy 19/20, flags resample 20/20
```

**What is a consequence.** The reach of influence is `radius × substeps` by
construction, so the speed cap and discrete time are found with full power by
reading it and cannot be told apart by it (`DET-CON-001`). The lattice's
anisotropy is √2 at every scale, so its shape is visible and its scale is not
(`DET-CON-002`); the only measured part is that births reach the corner. A
reader that could see without rendering finds lazy rendering by smoothness with
power 20/20 because coarse ground is one repeated number (`DET-CON-003`). The
inhabitant's unit is the cell, so the lattice's scale cannot be read
(`DET-DEF-001`); v0.9 reported a literal `1.0` as a measurement of this.

**What had to be run.** Lazy rendering *is* findable by an inhabitant whose
looking renders (`DET-FND-001`): the cells at the edge of what it renders have
neighbours that are densities, and the birth rate there, relative to the
interior, separates the lazy universe with power 19/20 at a false-positive rate
of 1/20. The statistic was designed in v1.0 from the mechanism after the audit
and evaluated on held-out seeds, so it is **exploratory** until replicated on a
preregistered seed range. And the same rule fires on the resample control in
20/20 seeds: what it detects is the approximation at the boundary, not
non-computation as such. The speed cap is also found by texture (`DET-FND-004`):
the radius-3 rule leaves solid patches Life does not. Discrete space is not
reliably found with the honest gaze (`DET-FND-003`: power 0.55 at FPR 0.05, and
that in the presence of lazy rendering). The negative control's highest power
is 0.10 (`DET-FND-002`).

None of this tells an inhabitant whether it is simulated. It tells it which of
its own laws have the shape of an optimization, with what error rate, and that
a limit not found by these four statistics may be found by a fifth.

Artifacts: `out/detect/detection.csv`, `detection.json`, `evidence.csv` (every
raw measurement, so the tests can be re-run with another split or rule).

## Theory 6: fine-tuning, and the measure problem

Fine-tuning says only a narrow band of a universe's constants produces
complexity. In this model the constants are the rule's density bands, and
"complex" is one of three criteria (`T6-DEF-001`): resemblance to Conway on
activity and dispersion, a compressibility band, and a perturbation-growth
band. All three are choices; the first is calibrated against the one law
already believed interesting.

A "productive fraction" is a number only after someone says what counts as
one law and how laws are weighted. v0.5 took the grid of band centres; v0.9
corrected to the distinct laws it reaches (441 points denote 42 laws,
`T6-CON-001`). `measure` runs five priors — the grid points, the laws the grid
reaches, the laws the widened sweep reaches, all 2116 laws the band form can
state, and a fixed sample of 500 of the 2¹⁸ outer-totalistic rules it cannot —
under all three criteria:

```
prior               resembles conway         compressibility          perturbation growth
grid points         0.096 ± 0.059            0.260 ± 0.040            0.318 ± 0.064
reached laws        0.089 ± 0.063            0.294 ± 0.032            0.365 ± 0.062
widened laws        0.094 ± 0.059            0.327 ± 0.010            0.282 ± 0.023
all band laws       0.062 ± 0.033            0.328 ± 0.005            0.443 ± 0.037
life-like sample    0.092 ± 0.079            0.100 ± 0.006            0.172 ± 0.010

lowest share: 0.055 ± 0.035    highest: 0.446 ± 0.037
```

**Productive laws are a minority under every prior and criterion in every
seed** (`T6-FND-001`). **How small a minority is the prior's and the criterion's
as much as the laws'** (`T6-FND-002`): a factor of eight separates the lowest
share from the highest, and nothing in the model privileges one way of counting.
Under all band laws with the growth criterion the share is 44%, close to a
majority; under the life-like sample with the Conway bar it is 9%. Fine-tuning
as a sentence with one number in it is not something this model can say.

SENSITIVITY_PLACEHOLDER

Nothing here bears on the constants of our universe (`T6-HYP-001`).

Artifacts: `out/measure/measure.csv`, `measure.json`, `laws.csv` (every law of
the pinned seed with its profile and verdicts, so the priors can be re-weighted
without re-running), `measure_sensitivity.csv`; the v0.5 grid sweep in
`out/sweep/`.

## Theory 5: bootloader life

A **bootloader** is a cluster of 3 to 40 live cells that persists for at least
20 ticks and travels at least 4 cells — a glider is one, a block is not — and
the thresholds are choices (`T5-DEF-001`). A child's seed is hashed from what
crossed its parent's horizon and nothing else; bootloaders reach the next
layer only through a **gate** that forbids a sterile layer to seed a child, and
the gate is the framework's rule (`T5-DEF-002`). The ablation confirms that a
gated and an ungated chain never differ in a layer both built.

**The tracker is sound** (`T5-FND-001`). The same frames in a shuffled order,
where nothing travels, yield 0.35 ± 0.18 bootloaders per thousand cells against
7.3 ± 0.6 in the real order: a false-positive floor of 5% ± 3%, cleared in
20/20 seeds.

**The density of bootloaders rises down the chain** (`T5-FND-002`): 7.3 → 10.8
→ 13.0 per thousand cells at depths 1, 2 and 3, and standalone universes of the
same sizes at the root's seed give 7.3, 10.6 and 12.6. v0.9's "128 → 32 → 6"
was a count falling with area. The deepest layer's density is below the root's
in 1/20 seeds. Degradation thins out nothing here; it shrinks the world.

**The gate** (`T5-FND-003`). Under the shipped floors it never fires: gated and
ungated chains are identical in 20/20 seeds and every chain ends on space at
depth 3. Under permissive floors (edge 4, 2000 work units) it fires in 16/20
seeds and is the sole stop in 7/20. The `edge` map, over fractions and size
floors with and without the gate, finds sterility alone ending a chain only at
fractions of 0.3 or more with a floor of 8 or less, in at most 2 of 10 seeds
per cell, and never the commonest ending anywhere. A bootloader here is a
precondition, not an achievement: nothing in this model builds a computer, and
bootloaders decide whether a child exists, never what it is. That life's cosmic
role is to boot the next layer (`T5-HYP-001`) is the framework's reading, and
nothing here tests it.

Artifacts: `out/boot/boot.csv`, `gate.csv`, `shuffle_control.csv`,
`area_control.csv`, `controls.json`, `ensemble.csv`; `out/boot-permissive/`;
`out/edge/edge.csv`.

## Determinism and provenance

Same seed, same universe, on three targets (`GEN-FND-001`): a reference
universe with every parameter pinned reduces to `8151296893090507384`, asserted
by the native suite on x86_64 Linux and ARM macOS and by the WebAssembly suite,
neither seeing the other's answer. All randomness goes through one hand-written
SplitMix64 with positional sub-streams; no `rand`, no clock, no hash-map order.
The constant moved from `6900610681785451805` at v1.0 when the binomial closure
became the reference universe's; the change was deliberate, computed natively
and confirmed on wasm32 before being committed, and is recorded in
`golden.rs`.

Every run writes `metadata.json` with the commit it was built from, whether the
tree was clean, the compiler, the target, the configuration's text and
fingerprint, the seeds, and the experiment's design version. The ledger records
the commit each claim was verified at; CI reproduces everything at `HEAD` and
checks the ranges.

## What remains unresolved

- **Discrete time at higher power.** Twenty seeds cannot resolve shifts of one
  to two standard deviations in churn and dispersion. A forty- or eighty-seed
  run would say whether discrete time is invisible to these observables or
  merely faint.
- **A fourth closure.** Binomial, indicator and frozen are three points; a
  closure that tracks Life's decay (a fitted or table-driven one) would say
  whether any cheap stand-in for unobserved ground can be invisible at the
  macro scale.
- **`edge_excess` is exploratory.** It needs a preregistered replication on a
  fresh seed range before it is confirmatory, and a variant that an inhabitant
  without a straight window edge could compute.
- **A like-for-like window control for the pipe.** Two windows over the same
  kind of ground, so the horizon's advantage over the far window can be
  attributed.
- **The measure problem has no resolution in the model.** Nothing weights one
  law above another; the result is the dependence, and it stays.
- **Nothing here builds a computer.** Theory 5's bootloaders decide whether a
  child exists, never what it is. A child whose dynamics depended on what its
  parent's bootloaders did is the research track, and unscheduled.

## Layout and architecture

A workspace. `universe-core` at the root holds the model, the experiments and
the reports; `crates/universe-web` is a thin wasm-bindgen bridge that owns no
physics; `web/` is the viewer. [`docs/architecture.md`](docs/architecture.md)
states the MODEL / EXPERIMENTS / RESULTS / INTERPRETATION layering and the
rules it enforces; the table below maps modules to it.

| Module | Role | Layer |
| --- | --- | --- |
| `constraints`, `space`, `physics`, `observer`, `rng` | The four limits, two-fidelity storage, pure update rules, probes, the generator | model |
| `budget`, `layer`, `pipe`, `bootloader` | The degradation rule, nesting, the horizon, cluster tracking | model |
| `limits`, `observables` | The factorial, its nulls and its observables | experiments |
| `detector`, `information`, `measure`, `sweep` | Hypothesis tests, bits per task, priors, the grid sweep | experiments |
| `stats`, `experiment`, `golden`, `provenance` | Intervals, ensembles, the fingerprint, `metadata.json` | experiments |
| `report` | CSV, JSON and summaries that end by declining to overstate | results |
| `analysis/claims.toml`, `README.md`, `docs/philosophy.md` | The ledger, the computational conclusions, the hypotheses | interpretation |

## Building

Rust stable; two direct dependencies in the core (`serde`, `toml`, for reading
the config). Always benchmark with `--release`; debug builds are close to 20×
slower.

```sh
cargo build --release
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

For the browser build (needs `wasm-pack` and the `wasm32-unknown-unknown`
target):

```sh
wasm-pack test  --node crates/universe-web          # the cross-target check
wasm-pack build --target web --out-dir ../../web/pkg --release crates/universe-web
cd web && python3 -m http.server 8000               # then open localhost:8000
```

The analysis scripts (`analysis/`) are a second opinion that recomputes the
summaries with pandas and scipy and draws the figures; see
[`analysis/README.md`](analysis/README.md).

## Licence

Dual licensed under [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE), at your
option.
