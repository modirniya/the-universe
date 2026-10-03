# Architecture: model, experiments, results, interpretation

The repository is arranged so that an assumption of the model cannot quietly
become a fact about the world. Four layers, each depending only on the ones
above it:

```
MODEL            what the toy universe is
  |
  +-- dynamics     physics, space, constraints      pure functions over immutable state
  +-- resources    budget                           the degradation rule
  +-- observer     observer                         probes; render and collapse
  +-- nesting      layer                            layers hosting layers
  +-- channel      pipe                             the one-way serializing horizon
  +-- agents       bootloader (detection half)      clusters that persist and travel
  +-- randomness   rng                              SplitMix64, positional sub-streams
       |
       v
EXPERIMENTS      what is done to the model to learn something
  |
  +-- controls     limits (four nulls), detector (negative and resample controls),
  |                information (window and noise controls), bootloader (shuffle and
  |                area controls), layer (size control)
  +-- measurements observables, detector statistics, information measures,
  |                sweep criteria, measure priors
  +-- statistics   stats                            intervals, effect sizes, Wilson counts
  +-- design       limits (factorial), detector (calibration/evaluation split),
  |                measure (priors and settings), layer (termination map)
  +-- reproduction experiment::per_seed, golden, provenance
       |
       v
RESULTS          what the experiments produced
  |
  +-- raw          out/**/*.csv                     every seed, every cell, every test
  +-- summaries    out/**/*.json, the CLI's printed tables
  +-- provenance   out/**/metadata.json             commit, toolchain, target, config, seeds
  +-- figures      analysis/figures.py              drawn from the artifacts, computing nothing new
       |
       v
INTERPRETATION   what the results are taken to mean
  |
  +-- computational conclusions   README.md, analysis/claims.toml (definition / consequence / finding)
  +-- philosophical hypotheses    docs/philosophy.md (untested, and labelled so)
```

## Rules the layering enforces

**Physics knows nothing about experiments.** `physics::step` takes a world,
a rule and a resolved set of dials and returns a world and a work count. It
does not know it is being measured, which seed it is, or what the report will
say. The detector's inhabitant reads the world through the same `sample` every
cell uses, and the pipe's horizon through the same. Nothing in the MODEL layer
imports from EXPERIMENTS.

**An assumption of the model is a parameter, not a constant.** The closure for
unobserved ground (`CoarseRule`), the rule (`Rules`), the degradation
fraction, the block size and the probe are all configuration. Where an
experiment's result depends on one of them, the experiment varies it
(`limits` runs the closures; `measure` runs the settings; `layer` maps the
floors) and the result is reported against the variation rather than at one
value.

**Every finding has a null.** A number is reported against what the same
statistic reads when nothing is a limit: a reseed, a one-cell perturbation, a
density nudge, a shuffled field (Theory 1); two all-limits universes
(detection); a window elsewhere and random symbols (the pipe); shuffled frames
(bootloaders); a standalone universe of the same size (nesting and
bootloaders). A finding that does not clear its null is reported as not
clearing it.

**Consequences are derived, not run.** What follows from the definitions is in
`docs/derivations.md` and is labelled `consequence` in the ledger and the
README. Running the model confirms the arithmetic and adds nothing; the
experiments spend their seeds on what could have come out otherwise.

**Interpretation is downstream of results and stays there.** The README's
claims table is generated from the artifacts and the ledger; it does not
contain a number the artifacts do not. `docs/philosophy.md` holds the
framework the model was built to test, including the parts no run can bear on,
and says which those are. The code never names a physical object it does not
model: the channel is a horizon and a pipe, not a black hole.

**Determinism is a cross-target invariant.** `golden` pins one universe to a
`u64`; the native and WebAssembly suites each assert it. Every run writes its
commit, toolchain, target and configuration beside its numbers. A result is
reproducible from `scripts/reproduce.sh` at the recorded commit, or it is not
a result.

## Where to add things

- A new **observable** goes in `observables`; the factorial picks it up.
- A new **inhabitant statistic** goes in `detector::STATISTICS` with its
  standing declared in `detector::standing` before any data is seen.
- A new **encoding** or **task** for the pipe goes in `information`.
- A new **criterion** or **prior** for fine-tuning goes in `sweep` or
  `measure`.
- A new **claim** goes in `analysis/claims.toml` with its category, and in the
  README with its id, or `analysis/ledger.py` fails.
- A new **physical hypothesis** goes in `docs/philosophy.md` and nowhere else.
