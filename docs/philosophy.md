# The theory stack

This file holds the framework the code argues for, including the parts that
cannot be coded. The code implements sections 1–6 in miniature; sections 7–10
are here because they explain why the experiments are worth running at all,
not because the repo can test them.

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

**In the code:** `constraints`, `space`, `physics`, `observer`. This is the
whole of v0.1.

**What would falsify it within the model:** if turning the limits on failed to
make the universe meaningfully cheaper, or made it so different that an
observer could trivially tell. See "Findings" in the README for what actually
happened, including the one limit that turned out not to be free.

**Can an inhabitant tell?** `detector` asks this, and the answer is partial.
Pixelation is invisible because the cell is the ruler. The speed of influence is
measurable but arrives as a product that cannot be factored, so an inhabitant
can know it is constrained without learning how. Lazy rendering is concealed by
the act of measuring it, since looking is what forces a region into full
resolution — hidden not by subtlety but by contradiction.

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
bound) and `layer` (the containment relation, and sizing each world to its
budget). Run it with `the-universe nest`.

**What would falsify it within the model:** a chain that runs deeper than the
closed form allows, a layer that outspends its host, or a total cost that fails
to converge. All three are checked. What actually happened is in the README:
the chain is finite and costs less than 1.33x its root layer, and it died of
the spatial floor before it ran out of money.

---

## 3. Black holes as serializing pipes

A **pipe** is a one-way channel between layers.

The framework's candidate for a real one is the black hole. What goes in does
not come back, and what comes out bears no resemblance to what went in. That is
exactly the behaviour of a serializing write: structure is destroyed,
compressed, scrambled. The **horizon** is the write surface. The singularity is
not a place inside the child universe at all — it is outside the child's
address space, which is why the child's physics reports it as a division by
zero rather than as a location.

What might survive serialization is not content but *timing and magnitude*: how
much went in, and when. A parent reading the far end of the pipe would receive
something closer to a log line than a message.

**In the code:** `pipe`. The horizon is a region of the child universe folded
into one 128-bit message per tick, whatever its area. Serialization is
position-sensitive, so the digest depends on the arrangement — and avalanching,
so it cannot be read back as one.

**What would falsify it within the model:** content structure surviving the
crossing (the digest tracking the arrangement instead of scattering), or timing
and magnitude *not* surviving (a parent's view uncorrelated with the child's
behaviour, meaning the pipe carries nothing at all). Neither happened: a
one-cell change flips half the digest, while what crossed still tracks the child
at 0.79 through a channel carrying 5.6% of the information.

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

**In the code:** v0.2+ milestone, and the hardest one. Emergence cannot be
scheduled.

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

**In the code:** `sweep`. The rule's density bands are the constants, and both
band centres are swept across a grid; each setting gets its own universe and is
scored on whether it ends up neither empty nor saturated, still changing, and
spatially structured.

**What would falsify it within the model:** complexity turning out to be common.
The fine-tuning claim needs the productive band to be narrow, and a sweep
finding most laws productive would refute it. The measured answer is 19% of the
distinct laws reachable — a minority, but not the sliver the argument usually
assumes. Two cautions travel with that number: the bar is calibrated from Conway
and so measures resemblance rather than worth, and the productive fraction must
be counted over *distinct laws* rather than grid area, since a neighbourhood of
eight cells only admits densities k/8 and 441 settings collapse onto 42 laws.

---

## The parts that cannot be coded

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
