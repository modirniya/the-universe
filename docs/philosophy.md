# The theory stack

This file holds the framework the code was built to investigate, including the
parts that cannot be coded. The code implements sections 1–6 in miniature;
sections 7–10 are here because they explain why the experiments were worth
running at all, not because the repo can test them.

Every statement here is a **physical hypothesis** in the sense of
`analysis/claims.toml`: an interpretation concerning our universe that nothing
in this repository tests. Each section ends by saying what the model's
experiments found when the theory was tried in the toy, and those findings are
about the toy. Where a v0.9 result has been withdrawn, this file says so rather
than quietly updating the sentence.

The order matters. Each theory is what makes the next one thinkable.

---

## 1. Limits as optimizations

Physical limits are not brute facts about reality. They are resource decisions.

A universe is expensive to run. Anyone running one has an interest in running
it cheaply, and the cheapest correct universe is the one that computes only
what has to be computed, only when it has to be computed, at only the fidelity
that will be noticed. Read that way, the limits we find at the bottom of
physics stop looking like mysteries and start looking like line items:

- **Discrete space and time** — a finite lattice instead of a continuum. You
  cannot store a continuum. Pixelation is what storage looks like from inside.
- **A speed cap** — a bound on how far influence travels per unit of time.
  Without it, every update depends on every cell, and cost per step scales with
  the whole universe rather than with a neighbourhood.
- **Lazy rendering** — full resolution only where something is looking. Detail
  that nothing observes need not be computed, and if it is never computed, it
  costs nothing.

The claim is not that these are the *only* possible optimizations, nor that a
creator would necessarily pick them. It is that they are the ones a competent
engineer would reach for first, and that we appear to live under all three.

**In the code:** `constraints`, `space`, `physics`, `observer`, and the
experiment in `limits`. The four limits are toggles on one automaton; what
stands in for an unobserved region is an assumption with three implementations,
and every lazy-rendering result is reported under each.

**What would falsify it within the model:** if turning the limits on failed to
make the universe meaningfully cheaper, or made it so different that an
observer could trivially tell. The first did not happen and could not have:
the savings are arithmetic on the config. The second did, for three of the four.
In the v1.0 factorial, discrete space, the speed cap and lazy rendering each
move seven macro-scale observables by many seed-to-seed standard deviations —
the speed cap because it changes the law, lazy rendering by an amount that
depends on the closure chosen for unobserved ground. Discrete time is the one
limit within a reseed's distance of the reference; at twenty seeds the
observables cannot tell it from a reseed, which is weaker than v0.9's "free
lunch, below the chaos floor in 20/20 seeds", a result that turned out to rest
on a transient and a metric that rewards low variance. So, in this model: a
competent engineer would take discrete time and pay in visibility for the rest.

**Can an inhabitant tell?** `detector` asks this as a hypothesis test with
calibrated error rates. The reach of influence reads `radius × substeps` by
construction, so the speed cap and discrete time are found and cannot be told
apart. The lattice's anisotropy is √2 at every scale, so its shape is visible
and its scale is not — geometry, not a discovery. And lazy rendering, which
v0.9 said was concealed by the act of looking, is findable: the cells at the
edge of what an inhabitant renders have neighbours that are densities, and the
birth rate there gives it away with power 0.95. What is found is the
approximation at the boundary — a fully rendered universe with equally
approximated ground is flagged just as often — but it is found.

---

## 2. Nesting and degradation

If a universe can be run, a universe can run one.

A **layer** is one universe in the chain. **Layer 0** is the host machine's
process — for this repo, a laptop. Every layer can host children, and every
child necessarily runs on a fraction of its parent's resources, because the
parent has to keep running too.

That fraction is the **degradation rule**, and it has a consequence the
framework cannot avoid: the chain is finite. Each layer is poorer than the one
above, so depth terminates. A child universe is always a smaller universe, and
somewhere down the chain is a layer too poor to host anything.

This cuts against the intuition that a simulation chain could be infinite. It
also means that if we are simulated, we are not simulated *cheaply* — we are
near enough to the top of a chain to still afford complexity.

**In the code:** `budget` (the degradation rule and the closed-form depth
bound) and `layer` (the containment relation, sizing each world to its budget,
the termination map and the size control). Run it with `the-universe nest`.

**What would falsify it within the model:** a chain that runs deeper than the
closed form allows, a layer that outspends its host, or a total cost that fails
to converge. None can happen: all three are consequences of the degradation
rule, derived in `docs/derivations.md`, and the runs confirm the arithmetic.
What the runs add is which definition ends a chain where — budget, the size
floor, or the block partition — mapped over four axes without running physics.

**What was withdrawn.** v0.9 reported a "degradation signature": the deepest
layer calmer than the root in 20/20 seeds. A layer of the chain is a standalone
universe of its size, by construction — nothing about its host reaches it — so
the signature was a statement about world size, and under a binomial closure
for unobserved ground it inverts: 0/20. Degradation here is a budget rule and a
size rule and has no trace in the dynamics beyond what size and the partition
produce.

---

## 3. The horizon as a serializing pipe (the black-hole analogy)

A **pipe** is a one-way channel between layers.

The framework's candidate for a real one is the black hole: what goes in does
not come back, and what comes out bears no resemblance to what went in. That is
the behaviour of a serializing write, in which structure is destroyed,
compressed, scrambled. The **horizon** is the write surface.

That is an analogy, and nothing in this repository tests it. The model has no
gravity, no horizon in the physical sense, and no singularity. An earlier
version of this section said the singularity lies outside the child's address
space, which is why physics reports it as a division by zero; that is a claim
about our universe, the code cannot reach it, and it has been withdrawn.

What might survive serialization is not content but *timing and magnitude*: how
much went in, and when. A parent reading the far end of the pipe would receive
something closer to a log line than a message.

**In the code:** `pipe` and `information`. The horizon is a region of the
child universe folded into one message per tick, whatever its area. The code
never calls it a black hole: the module models a channel, and the analogy is
this paragraph.

**What is designed in.** The message has a field for magnitude and a digest
that is a hash, so "timing and magnitude survive, arrangement does not" is how
the pipe was built rather than something the run discovered. That the hash
carries no occupancy is a property of hashing.

**What would falsify it within the model:** a parent's view uncorrelated with
the child's behaviour at every width and encoding, meaning the pipe carries
nothing; or the horizon carrying information that a window elsewhere would not,
meaning the mechanism did something unaccounted for. Neither happened. At full
width the horizon carries 1.4 bits a tick about its own occupancy, 0.5 about
the whole child now, 0.3 about the child ten ticks ahead and 0.2 about which
quadrant is densest. What a narrow channel keeps is the encoding's: two
uniform bits carry nothing at the horizon's placement, an adaptive four bits
carry 1.9. v0.9's "two to six bits keep 90%" was one encoding at one placement
and is withdrawn as a general statement. The pipe is a quantised channel and
nothing more; what survives it is what any channel of that width carries about
a slowly varying aggregate.

---

## 4. Mutual blindness

Neither side of a pipe can see through it.

A child cannot inspect its parent, because everything the child can measure is
made of the parent's implementation and therefore cannot reach past it. A
parent cannot inspect a child's interior either, except by reading what the
pipe delivers.

The **logging threshold** is the aggregate scale at which a parent's observer
notices child activity at all. Below it, nothing registers. This is where the
framework's least comfortable idea lives: the creator may be **unaware or
indifferent**. Not hostile, not absent — just watching a dashboard whose
resolution does not include us. A civilization is not a log line. A collapsing
galaxy might be.

The experiment borrows this idea as a measurement tool. Two universes running
at different internal resolutions cannot be compared cell by cell, so the code
compares them at a coarse macro grid — deliberately the parent's-eye view.
Divergence is measured where an outside observer would actually be looking.

**In the code:** `space::macro_field` and `report` for the experiment's own use
of the idea; `pipe` for the real thing. The threshold is a parameter, not a
metaphor: `ReadEnd::above` is the entire extent of a parent's access to a child,
and below it nothing registers.

The blindness itself is enforced by the type system. A child holds a `WriteEnd`,
which exposes no method returning anything about the far side, so it cannot
learn that it is read or by what; a parent holds a `ReadEnd`, which cannot write.
There is no conversion back. Making this unrepresentable rather than merely
documented is the clearest use the project has found for choosing a language
with a strict compiler.

---

## 5. Bootloader life

The cosmic role of emergent agents is to boot the next layer.

Complexity emerges. Some of it becomes agentic. Agents build computers, and
computers eventually run universes. On this reading, life's function in the
chain is not to persist, flourish, or understand — it is to be the mechanism by
which a layer instantiates the layer below it. A **bootloader** is any emergent
pattern whose effect is to start computation one layer down.

This reframes the search for purpose unsentimentally. The question is not
whether life means anything, but what life *does* structurally, and what it
does is boot the next BIOS.

**In the code:** `bootloader`. A bootloader is detected as a pattern that
persists, stays localized, and *travels* — structure moved to somewhere it was
not, which is the transport any real bootloader would need first. Conway's
glider is the canonical case and the detector is checked against one.
`run_boot_chain` then closes the framework's loop: each layer is seeded from
what crossed its parent's horizon, so the child is booted from inside the
parent, through a channel neither end can see through.

**What would falsify it within the model:** bootloaders appearing everywhere,
which would make them unremarkable and disconnect this from Theory 6; or
nowhere, which would mean the chain is inert by construction. Neither happened:
about seven per thousand cells in a Conway soup, of which the tracker's
false-positive floor (the same frames in shuffled order) accounts for 5%.

**What the model does not show.** The child's seed is derived from what crossed
the horizon, and bootloaders never enter it. They reach the next layer only
through a gate the framework imposes: a layer with no bootloader may not seed a
child. Removing the gate changes nothing under the shipped floors, and under
permissive ones it only lets chains run further; in no case does it change what
a child is. So in this model life decides *whether* the next layer exists, by
rule, and never *what* it is.

**What was withdrawn.** v0.9 read "128 → 32 → 6 bootloaders down the chain" as
degradation thinning out life. Per thousand cells the density *rises* down the
chain, and standalone universes of the same sizes give the same densities. The
count fell because the world shrank.

**What it is not.** Nothing here builds a computer. The model shows the
precondition is available, not that the achievement follows. "The cosmic role
of emergent agents is to boot the next layer" is an interpretation of the
framework, not an output of any experiment here.

---

## 6. Fine-tuning

Only narrow bands of a universe's constants produce complexity.

Most parameter settings give you heat death or immediate collapse. The
interesting band is thin. This is usually deployed as an argument for design;
here it is treated as something to measure. Sweep the constants of a toy
universe and see how thin the band actually is, and whether complexity appears
where the framework predicts.

The toy already exhibits a small version of this: the rule is stated as density
bands, and moving those bands even slightly turns a world that produces
structure into one that dies or saturates.

**In the code:** `sweep` and `measure`. The rule's density bands are the
constants. Every law the band form can state (2116) and a sample of the
outer-totalistic family it cannot (500 of 2¹⁸) are scored under three
criteria, and the productive share is reported under five ways of weighting
laws.

**What would falsify it within the model:** complexity turning out to be
common. Productive laws are a minority under every prior and criterion in every
seed, so that much survives. How small a minority does not survive as a number:
from 5.5% (all band laws, the Conway-resemblance bar) to 45% (all band laws,
perturbation growth), a factor of eight that is the prior's and the criterion's
as much as the laws'. v0.5's "19%" was one prior, one criterion, one seed, and
is withdrawn. Nothing in the model privileges one way of counting laws, and
that absence is the honest content of the fine-tuning argument as it stands
here: the measure problem is not resolved by running a toy, it is exhibited.

---

## The parts that cannot be coded

Sections 1 to 6 are tested, but only as models: each has a module that
implements it and a command whose output can contradict it, and every such test
is a test of the toy, not of the world. Sections 7 to 10 are not tested at all.
Nothing in the repository bears on them, and no run of it is evidence for or
against them. They are here to say why the experiments were worth building.

Everything above can be modelled. What follows cannot, and lives here so that
it stays out of the code.

### 7. Are *we* simulated?

The model cannot answer this and neither can anything inside it — that is what
mutual blindness means. A coherent model of a simulated universe is evidence
that the idea is *thinkable*, not that it is true. Confusing those two is the
main way this kind of project goes wrong, which is why the README says so
before it says anything else.

### 8. God plus peers

If our layer has a creator, that creator is likely not unique to us. A process
that can run one universe can run several, and probably exists alongside other
processes doing the same. The theological picture this suggests is not a
singular god but something closer to an operator among operators — with
colleagues, budgets, and other things running. This is not offered as
consolation.

### 9. The Big Bang as the input event

A universe that starts needs an input. The Big Bang, in this framing, is the
moment the seed was supplied: a single write from outside, after which
everything follows from the rules. The seed is the creator's only necessary
intervention.

The code takes this literally. All randomness flows through one seeded RNG, and
the seed is supplied from outside the universe, in a config file. That is why
`rng` is described as the creator's runtime input channel rather than as a
utility module — it is load-bearing philosophy that also happens to be good
engineering, since it is what makes runs reproducible.

### 10. Consciousness

The framework has no account of consciousness and does not pretend to. It can
say what an observer *does* — force resolution, collapse superposition of
detail, cost the creator money — without saying what an observer *is*. The
`observer` module implements the function, not the phenomenon. A fixed window
that triggers rendering is a probe; nobody is claiming it experiences anything.

Whether the thing that collapses detail must be conscious, or merely
interacting, is the question this framework most conspicuously does not settle.

---

## Vocabulary

Used consistently in code, docs and commit messages.

| Term | Meaning |
| --- | --- |
| **Layer** | One universe in the chain. Layer 0 is the host machine's process. |
| **Horizon / pipe** | The one-way serializing channel between layers. |
| **Logging threshold** | Minimum aggregate scale at which a parent's observer notices child activity. |
| **Degradation rule** | Each child's resource budget is a strict fraction of its parent's. |
| **Bootloader** | An emergent agent or pattern whose effect is to instantiate computation one layer down. |
| **Probe / observation** | The event that forces full-resolution computation of a region. |
