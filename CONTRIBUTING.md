# Contributing

Thanks for looking. This is a small, opinionated project and it is worth
knowing what it is before you spend time on it.

## What this project is

It is an argument written as a program. Each module implements one theory from
[`docs/philosophy.md`](docs/philosophy.md), and running the code demonstrates
that those theories are **coherent** — that they can be made to work together
in a system that executes and produces measurable results.

It does **not** claim our universe works this way, and no contribution should
imply that it does. The model makes falsifiable predictions only about its own
behaviour. If a change would make the README or the printed summary sound more
confident than that, it is the wrong change no matter how good the code is.

Philosophy that cannot be coded belongs in `docs/philosophy.md`, not in the
source.

## Getting set up

Rust stable. No other tooling.

```sh
git clone https://github.com/modirniya/the-universe
cd the-universe
cargo test
cargo run --release -- run --config configs/quick.toml
```

`configs/quick.toml` is a small world for iterating. `configs/default.toml` is
the real experiment and takes about 11 seconds on an M1 Air. Always benchmark
with `--release`; debug builds are close to 20× slower (18.7× on
`configs/quick.toml`, M1) and the numbers become meaningless.

Before opening a PR:

```sh
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

CI runs exactly these, plus the tests on macOS ARM64, plus a job that runs the
documented experiment and checks the README's reproducible claims still hold.

## The five rules

These are not style preferences. Each one is load-bearing for either the
argument or the reproducibility of the results, and a PR that breaks one will
be asked to change even if it is faster or shorter.

**1. Determinism. Same seed, same universe.**

All randomness goes through `rng::Rng`. Do not introduce `rand`, thread-local
generators, `SystemTime`, or anything that depends on hash-map iteration order.
Where a value depends on position, derive a sub-stream with `Rng::derive`
rather than drawing from a shared one — otherwise the result depends on visit
order, and it stops being reproducible.

This is also why `rand` is not a dependency: `StdRng` is not guaranteed
reproducible across `rand` versions.

**2. Physics is pure.**

`physics::step` takes state and returns state. No mutation of inputs, no
interior mutability, no logging, no clock, no I/O. `step_does_not_mutate_its_input`
checks this rather than trusting a comment.

This is what lets one law run at four resolutions and two fidelities and still
be the same law. It is also why the physics is the part of the codebase that is
thoroughly tested — pure functions are trivially testable, which is partly the
point of making them pure.

**3. Benchmarks are claims.**

Every performance statement in the README must be reproducible by a command in
the repo. Distinguish reproducible counters (`Work`, divergences, cell counts)
from machine-dependent measurements (wall time), and never present the second
kind as though it were the first.

If you change the physics or the cost model, the headline numbers will move.
Update the README **and** the thresholds in the `claims` CI job together; that
job exists to stop the two drifting apart.

**4. Module names match theory names, and module docs name their theory.**

The code is meant to self-document the philosophy. Do not rename a module to
something more conventional. New modules need a `//!` doc saying which theory
they implement and **what would falsify it within the model** — that last part
is the one people skip, and it is the one that keeps this honest.

**5. Rules stay resolution-independent.**

`physics::Rules` is stated as density bands rather than neighbour counts, so
the same law survives a change of resolution. At radius 1 the defaults reduce
exactly to Conway B3/S23, checked with a blinker, a block and a glider. Any new
rule must keep that property, or the cross-resolution comparison the whole
experiment rests on becomes meaningless.

Related: `World::seed` draws the initial pattern at base resolution and
upsamples it, so a coarse universe and a subdivided one start from the same
macro configuration. Without that, the resolution comparison would be comparing
two different initial conditions.

## A note on measuring divergence

The world is chaotic. Any perturbation decorrelates the macro field, so a raw
divergence number on its own says nothing about whether a limit changed the
universe.

This is why `experiment::run_all` runs the unconstrained reference a second
time with only the seed changed. Those two universes are unquestionably the
same *kind* of universe, and the divergence between them is the floor that
chaos alone produces. Every verdict is judged against that floor rather than
against zero.

If you change how divergence is measured, keep a control of some kind. Without
one the report is uninterpretable and every limit looks damning.

## Dependencies

Currently `serde` and `toml`, both only for reading the config file. Please
resist adding more. A PR that adds a dependency should say what it buys and why
the standard library will not do.

## Where help is most useful

v0.1 is one layer and stops there. The roadmap, in order:

1. **Nesting** — layers hosting child layers under the degradation rule
2. **Pipe** — the one-way serializing channel between layers
3. **Detection** — agents trying to determine from within whether they are
   running under limits
4. **Fine-tuning sweep** — how thin the band of complexity-producing constants
   actually is
5. **Emergence** — bootloader life

Nesting will want a `layer` module and a budget type implementing the
degradation rule, not a fifth constraint toggle.

Smaller things that are genuinely welcome: more physics tests, clearer module
docs, performance work that does not compromise determinism, and corrections to
the philosophy doc where it overstates something.

Please open an issue before starting anything large, so you do not spend a
weekend on a design that conflicts with where the model is going.

## Bugs

A bug report is most useful with the config that produced it and the seed. Both
are enough to reproduce any run exactly — that is the whole point of rule 1.

## Disagreeing with the philosophy

Reasonable, and welcome as an issue. Arguments that a theory is incoherent,
that a module does not implement what it claims, or that a result is
overstated, are the most valuable feedback this project can get.

Arguments about whether *we* are simulated are out of scope for the issue
tracker — the model cannot settle that, which is itself one of its claims
(see mutual blindness in the philosophy doc).

## Getting in touch

Issues are preferred, since the answer usually helps someone else too. For
anything that does not belong in public: **modirniya@gmail.com**.

## Licence

By contributing, you agree that your contributions are dual licensed under
[MIT](LICENSE-MIT) and [Apache 2.0](LICENSE-APACHE), matching the project,
without any additional terms or conditions.
