# Audit of the model and its experiments (pre-v1.0)

This is the record of an audit of the repository as it stood at v0.9.0
(commit `7839c97`), made before any experiment was redesigned. It is kept
because the weaknesses it found are part of the project's history, and because
a reader deciding how much to trust the current results should be able to see
what was wrong before and what was done about it.

Everything below was checked by reading the code and, where a claim could be
tested cheaply, by running it. Each weakness names the file it lives in. The
section at the end says what the redesign did about each one; items marked
*open* were not fixed and are stated as limitations.

## Vocabulary used throughout

Every claim the repository makes is placed in one of four categories, and the
rest of this document, the README, and `analysis/claims.toml` use the same
four:

| Category | Meaning | What establishes it |
| --- | --- | --- |
| **Definition** | Something the model deliberately builds in. | Reading the code. |
| **Consequence** | Follows necessarily from the definitions. | A derivation. Running it only confirms the arithmetic. |
| **Computational finding** | Could have come out otherwise when the model ran. | An experiment, with controls, replication and an uncertainty. |
| **Physical hypothesis** | An interpretation concerning our universe. | Nothing in this repository. |

The v0.9 README already made a two-way split ("follows from rules" versus "had
to be run"). The audit found that split was applied generously: several rows
marked "had to be run" are consequences, or are findings about an
implementation artifact rather than about the model.

## 1. Theory 1 — limits as optimizations (`experiment`, `physics`, `space`)

### 1.1 The coarse-block update is not a mean field. It is a pathology.

`physics::step` advances an unresolved block by

```
nd  = mean density of the 8 neighbouring blocks   (the block's own density is ignored)
new = (born(nd) ? 1 - d : 0) + (survives(nd) ? d : 0)
```

The doc comment calls this "the expected outcome if the block's occupants were
spread evenly". It is not. The expected next occupancy of a cell whose eight
neighbours are independently alive with probability `p` is a *binomial*
average of the rule over `k = 0..8`, which is smooth in `p`. The code applies
the rule's *indicator* to the mean, so a block whose neighbours sit in the
birth band jumps to `(1 - d) + d = 1.0` — fully saturated — in one substep,
and a saturated block then pushes its neighbours' mean above the survival band
and kills them. Measured on `configs/default.toml` at seed 42 with lazy
rendering alone, the mean density of unobserved ground went
0.45 → 0.30 → 0.25 → 0.40 → 0.35 → 0.33 → 0.19 → 0.17 → 0.17 → 0.23 → 0.27 → 0.08
over the first twelve ticks while the observed ground stayed near 0.29. That
is not coarse graining losing detail; it is a different and badly behaved
dynamical system occupying three quarters of the world.

Consequences for the published results:

- "Lazy rendering is the most visible limit, 2.65× the floor" (README,
  Findings) is mostly a measurement of this artifact. The occupancy collapse to
  0.05 that the README explains as "mean-field approximation is a real loss of
  fidelity" is the coarse ground oscillating and dying.
- Every chain in `layer` and `bootloader` runs with lazy rendering on and
  three quarters of each world unobserved, so the churn, sterility and
  bootloader counts of every layer were taken on worlds whose unobserved parts
  were driven by this rule.
- The detection result "a passive reader finds lazy rendering" is a finding
  about *this* approximation, not about lazy rendering as such. The mission's
  question — is the detector detecting lazy rendering or the approximation? —
  cannot be answered without a second approximation to compare against.

**Classification:** the saturating behaviour is an implementation artifact.
Any finding that depended on unobserved ground is affected.

### 1.2 The chaos floor is the wrong null for a same-seed comparison

`experiment::run_all` compares each constrained run to the reference run *at
the same seed*, and calibrates against a control run *at a different seed*.
The two kinds of pair start in different states: a same-seed pair shares its
initial condition and decorrelates over time; the different-seed pair is
uncorrelated from tick 0. `Comparison::mean_divergence` averages over every
tick, including the transient, so a same-seed pair is handed a head start.

Measured at seed 42: `time` versus the reference reads 0.04–0.06 at ticks
1–40 against a floor of 0.07 at the same ticks, then 0.087–0.090 at ticks
120–199 against a floor of 0.072–0.084. Averaged over the whole run it is
0.94× the floor; averaged over the second half it is 0.98×; at the final tick
it is 1.2×. The README's "discrete time diverges below the chaos floor in
20/20 seeds" and "the free lunch, in every seed" rests on the whole-run
average, and the whole-run average is dominated by the transient.

Two further problems compound it:

- The floor is **one pair per seed**, not a distribution. "Below the floor" is
  a comparison of one number against one other number. No dispersion of the
  null is ever estimated, so there is no sense in which 0.94× is far from 1.
- `space::macro_divergence` is a mean absolute difference, and for two
  independent fields with equal means it is *smaller* when one field has less
  variance (Jensen). The `time` universe's second-half macro variance is
  0.0050 against the reference's 0.0064. A universe with less structure scores
  *below* the floor on this metric while being measurably different.

**Classification:** "discrete time is below the chaos floor" is at best a
transient-dominated observation on one metric. It needs a same-seed null, a
null *distribution*, a late-time window, and a second observable that is
sensitive to variance.

### 1.3 The speed cap changes the law, not only the speed

`Constraints::speed_cap` switches the neighbourhood radius between 1 and 3
while the density bands stay fixed. At radius 1 the bands are Conway's
B3/S23. At radius 3 they define a 48-neighbour "larger than life" rule with a
different stationary density: the reference universe sits at occupancy 0.22
and the radius-1 universe at 0.05, in every seed. So the reference against
which every limit is judged is a different automaton from the one the
nesting, pipe, detection, fine-tuning and bootloader experiments run, and
"turning the speed cap on" is "replacing the law". The README reports the
speed cap as "visible above the floor" without noting that its occupancy
differs from the reference's by a factor of four; `live_delta` for `speed` is
0.16, the largest of any single limit.

Nothing is wrong with a model in which the speed of light and the rule are
entangled — the framework's own coupling finding says they are — but the
experiment's framing ("do the limits change what the universe produces?")
presumes the limits are interventions on *one* universe, and this one is not.

**Classification:** a definition presented as if it were an intervention.
The redesign reports the speed cap's effect on occupancy alongside its
divergence and says plainly that it is a change of law.

### 1.4 Work and memory ratios are arithmetic

`all_on` costs 1/(4 · 2 · 6 · 4) ≈ 0.0052 of the reference: four times fewer
cells, two times fewer substeps, six times fewer neighbours, four times fewer
resolved blocks. The "~190× cheaper" headline is a product of four config
values. The README classifies the per-limit ratios as "follows from rules"
but still presents "~190× less work" in the findings as if a run were needed.

**Classification:** consequence.

### 1.5 No factorial, one metric, no cost–fidelity analysis

Only five of the sixteen constraint settings are run (four singles and
`all_on`), so no interaction can be estimated, and "fidelity" is one number
(mean |Δ| of a 16×16 density field) plus an occupancy delta. There is no
observable sensitive to spatial correlation, temporal structure or cluster
statistics, and no attempt to put cost and fidelity on one chart.

### 1.6 Fine points

- `World::seed` starts every block resolved, and `run` samples peak memory
  only after the first `observe`. Fair, and documented.
- The macro field is built from block densities, so with `block_size = 16` on
  a 128-cell world the 16×16 macro grid of a `discrete_space` run is 8×8
  blocks upsampled. Runs at different subdivisions are compared on grids with
  different effective resolution. Documented nowhere.
- Wall time is reported but never asserted on. Correct.

## 2. Detection (`detector`)

### 2.1 `min_feature` is hard-coded

```rust
// Measured, not assumed: see `min_feature_is_always_one`.
min_feature: 1.0,
```

The field is a literal. The test it cites asserts `1.0 == 1.0`. The README's
"pixelation's scale is invisible from inside" is true and is correctly
classified as "follows from rules", but the code presents a constant as a
measurement and a test as a check.

**Classification:** definition, mis-presented as a measurement.

### 2.2 The √2 anisotropy is geometry

`Evidence::anisotropy` is the ratio of the largest diagonal reach to the
largest axis reach among births. At radius 1 a diagonal neighbour is at
distance √2 and an axis neighbour at distance 1, and nothing else is
reachable, so the ratio is √2 whenever one birth of each kind occurs — which
in a Conway soup of 10⁴ cells is certain. The README classifies "the
lattice's shape is visible from inside" as "had to be run, 20/20 seeds". What
had to be run is only that natural births reach the corner at all; the value
√2 is the Moore neighbourhood's geometry. The "1 for a continuum" it is
compared against is not a run.

**Classification:** consequence, with a trivial existence condition.

### 2.3 Verdicts are ad hoc thresholds, not tests

`separated(a, b)` calls a detection when the relative gap is ≥ 10% *and* the
absolute gap is ≥ 0.01. Both constants were chosen after seeing the data (the
README says so for `MIN_ABSOLUTE`). There is no null hypothesis, no
false-positive rate, no calibration on seeds other than the evaluation seeds,
and no negative control (two unconstrained universes should not be told apart).
The "passive reader finds lazy rendering in 12/20 seeds" is the number of
seeds on which a post-hoc threshold happened to be crossed; the eight misses
are ones where the signal was 80× the control but below 0.01 absolute.

### 2.4 Influence speed is the radius

Under the speed cap a birth needs three live cells within Chebyshev distance
1, so the nearest ancestor is at distance 1 by construction; `influence_speed`
reads `radius × substeps` because that is the only distance the rule can
produce. The README classifies this as "follows from rules". Agreed — but it
is then listed under "found" in the detection table as if the detector
established something.

### 2.5 The inhabitant did not scale with the lattice

`Inhabitant::cells` placed the inhabitant's window in *fine* cells while the
probe is placed in base cells. In a universe with `discrete_space` relaxed
(two fine cells per base cell) the inhabitant therefore covered a different
and smaller physical region than the same inhabitant in the coarse universe —
one lying wholly inside the observed ground, where there is no coarse ground
to read. Found during the redesign, when the new hypothesis test reported the
passive gaze "detecting" discrete space through smoothness with power 20/20:
the two universes being compared were being read in two different places.
Fixed in v1.0; the inhabitant is now placed in base cells like the probe.

## 3. Theory 3 — the pipe (`pipe`)

### 3.1 Correlation of a slowly varying scalar with itself

The channel carries the occupancy of a 48×48 window; the "truth" it is
correlated with is the occupancy of the whole 128×128 world, of which the
window is 14%. Both are slowly varying aggregates of the same field. That a
few bits of a slowly varying scalar retain most of its correlation with a
second slowly varying scalar is a property of quantising smooth time series,
not of horizons. The "2–6 bits keep 90%" result is the only measured claim in
the section, and it does not test anything specific to the mechanism.

The mission's question is what *information* survives for which *task*, and
whether the result survives a change of encoding. Neither is measured.

### 3.2 The horizon does not straddle coarse ground

`configs/pipe.toml` places the horizon at (48, 48)–(96, 96) and the observer
at (32, 32)–(96, 96); the horizon lies wholly inside the observed region. The
config comment says it straddles the two. It does not, so the pipe experiment
never exercised the child's own optimizations as claimed.

### 3.3 Naming

Module and config docs call this "black holes as serializing pipes". The
model has no gravity, no horizon in the physical sense, and no singularity;
`docs/philosophy.md` already says so. The code's naming does not.

## 4. Theory 2 — nesting (`budget`, `layer`)

### 4.1 Depth and total cost are consequences

`max_depth` and `total_cost_bound` are closed forms of a geometric series.
The README classifies both as "follows from rules". Agreed. The integer
truncation result (`max_depth` is a bound, not an equality) is also a
consequence, and is correctly stated as one.

### 4.2 "The deepest layer is calmer than the root" is a finite-size effect

A layer in `run_chain` is an ordinary `experiment::run` of a smaller world.
Nothing about *nesting* reaches it: not the parent's state, not its history,
not its seed (the plain chain reseeds every layer from the same `world.seed`).
So "churn falls down the chain" is "churn falls with world size under lazy
rendering", and the proposed control — a standalone universe of the same
size — reproduces every layer exactly, by construction. The README calls the
decline a "degradation signature". It is a statement about world size, and
about the coarse-ground artifact of §1.1, since smaller worlds under block
quantisation have a different unobserved fraction.

### 4.3 The termination map is a one-plane slice

`edge` sweeps the degradation fraction against the size floor at one block
size and one root size. The mission asks under what conditions nesting
terminates on budget, on spatial resolution, on block quantisation, or on
sterility; two of the four axes are missing, and the first three endings can
be mapped without running any physics at all, since `predict_work` is exact.

## 5. Theory 6 — fine-tuning (`sweep`)

### 5.1 The measure is a choice, and only one was tried

The "productive fraction" is counted over distinct laws reached by sweeping
two band centres over [0.05, 0.65] with Conway's half-widths, later widened to
three half-widths each. The range, the step and the family are all choices of
*prior*; the fraction over all 46 × 46 = 2116 contiguous-band laws, or over
the 2¹⁸ outer-totalistic Life-like rules the model can also express, was never
measured. The README correctly warns that "area is the resolution of the
sweep", then quotes a range of 9–33% as if the law space were given.

### 5.2 Criteria were chosen after seeing results

The compressibility band [0.2, 0.8] and growth band [0, 0.5] are described as
"fixed in advance"; the activity criterion was changed from a floor to a band
after a chaotic rule passed. This is exploratory work and should be labelled
as such.

### 5.3 Finite size

Every law is scored on one 64×64 world for 80 ticks from density 0.30.
Sensitivity to size, duration and initial density was not measured.

## 6. Theory 5 — bootloaders (`bootloader`)

### 6.1 The tracker has no false-positive estimate

A "bootloader" is a chain of nearest-centroid matches within 3 cells across
≥ 20 ticks that accumulates ≥ 4 cells of displacement. In a decaying Conway
soup, debris and oscillators lie within 3 cells of one another constantly, so
a track can hop between unrelated clusters and accumulate displacement without
anything travelling. Seed 42 reports 128 bootloaders in a 128² soup over 200
ticks — far more than the handful of gliders such a soup produces. No run on
a surrogate without real transport (e.g. the same frames in shuffled order)
was made, so the false-positive rate is unknown.

### 6.2 "Poorer layers produce less life" is area scaling

128 → 32 → 6 bootloaders across layers of 128², 55² and 24² cells is 7.8, 10.6
and 10.4 per thousand cells. The count falls because the world shrinks. The
README reads it as "degradation thins out what a universe can grow".

### 6.3 The gate is a definition

Correctly labelled already. The ablation confirms the code does what it says.

## 7. Statistics, reproducibility, provenance

- Every ensemble statistic is `mean [min, max]` over 20 seeds. No SD, no CI,
  no median, no effect size. Counts like "20/20" are reported without the
  caveat that twenty deterministic runs of one configuration from an
  arithmetic progression of seeds are not twenty independent experiments.
- Artifacts under `out/` carry no metadata: no commit, toolchain, target,
  config hash or experiment version. Provenance lives in README prose.
- There is no machine-readable list of claims; CI asserts a hand-picked
  subset of README numbers.
- The viewer exposes seed, limits and fingerprint but not the configuration
  or experiment identity, and nothing separates what it shows from what the
  findings rest on.

## 8. What was not found wrong

- `rng` is a correct SplitMix64; `derive` is positional. No `rand`, no
  `HashMap` iteration, no clock in physics.
- `physics::step` is pure and tested for it.
- `pipe::WriteEnd`/`ReadEnd` enforce one-way flow at the type level.
- `layer::predict_work` is exact and tested for equality.
- The golden fingerprint is asserted on two targets.
- Every documented command reproduces its artifacts byte for byte apart from
  wall time.

## 9. Disposition

| § | Weakness | What the redesign did |
| --- | --- | --- |
| 1.1 | Indicator mean field | `CoarseRule` is now an explicit parameter with `Indicator`, `Binomial` and `Frozen` variants; every lazy-rendering result is reported under each, and the published default is stated. |
| 1.2 | Wrong null, one pair, whole-run mean | `limits` runs four null models (different seed, one-cell perturbation of the same seed, initial-density perturbation, shuffled surrogate), reports divergence over the second half and at the final tick, and standardises every distance by the null's dispersion. |
| 1.3 | Speed cap changes the law | Reported as a change of law; occupancy difference printed beside divergence. |
| 1.4 | Ratios are arithmetic | Classified as consequences in the ledger; no longer listed as findings. |
| 1.5 | No factorial, one metric | Full 2⁴ factorial; six observables; cost–fidelity table and Pareto set. |
| 2.1 | Hard-coded `min_feature` | Removed from `Evidence`; the claim is a definition in the ledger. |
| 2.2 | √2 is geometry | Reclassified as a consequence; the measured part (births reach the corner) is stated as such. |
| 2.3 | Ad hoc verdicts | `detector::survey`: calibration seeds set a threshold at a stated false-positive rate, evaluation seeds measure power; negative control of two all-limits universes; resample control for lazy rendering. |
| 2.5 | Inhabitant in fine cells | Placed in base cells, like the probe. |
| 3.1 | Correlation only | `pipe::information`: mutual information with a shuffle null, five tasks, four encodings, and a same-bits control through a plain quantiser of a random window. |
| 3.2 | Horizon inside the observed region | Config fixed and the comment corrected. |
| 3.3 | Black-hole naming | Renamed to the horizon/pipe abstraction throughout; the analogy stays in `docs/philosophy.md` as a physical hypothesis. |
| 4.2 | Nesting = size | Size control added and its tautology stated; churn-against-size curve for standalone universes. |
| 4.3 | One-plane termination map | `terminate` maps budget/space/quantisation endings over four axes analytically. |
| 5.1 | One prior | Four priors, including the full band family and random Life-like rules. |
| 5.3 | Finite size | Size, duration and density sensitivity. |
| 6.1 | No false-positive rate | Frame-shuffled surrogate control. |
| 6.2 | Area scaling | Bootloaders reported per thousand cells. |
| 7 | Statistics, provenance, ledger | `stats` module; `metadata.json` on every run; `analysis/claims.toml` checked in CI. |

Items left open are listed in the README under "What remains unresolved".
