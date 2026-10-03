# Derivations

Results in this repository that follow from the definitions are derived here
rather than presented as experimental findings. Each section names the code
that implements the definition and the test or run that confirms the
arithmetic. If a derivation here is wrong, the check named beside it should
fail; if it is right, running the model adds nothing to it.

## 1. Work and memory ratios of the limits (`constraints`, `physics`)

A universe of `W × H` base cells with `s` fine cells per base cell per axis,
`k` substeps per tick, influence radius `r` and lazy rendering resolving a
share `c` of its blocks spends, per tick,

    visits = k · [ c · (sW)(sH) · ((2r+1)² − 1)  +  (1 − c) · (sW)(sH) / B² · v ]

neighbour visits, where `B` is the block edge in fine cells and `v` the visits
one coarse block costs (`Resolved::coarse_visits`: 8 for the indicator and
binomial closures, 0 for the frozen one). With every block resolved (`c = 1`)
the second term vanishes and the ratio between two settings is the ratio of
`k · s² · ((2r+1)² − 1)`. Under the shipped dials (`s = 2`, `k = 2`, `r = 3`
unconstrained; `s = 1`, `k = 1`, `r = 1` constrained):

| limit | factor |
| --- | --- |
| discrete space | `1 / s² = 1/4` |
| discrete time | `1 / k = 1/2` |
| speed cap | `8 / 48 = 1/6` |
| lazy rendering | `≈ c` for `v ≪ 8 B²`; `1/4` at the shipped probe |
| all four | `≈ 1/192 = 0.0052` |

Memory follows `s² · c` for `peak_live_bytes`. These are the "work" and
"memory" columns of every Theory 1 table, and `limits::tests::work_main_effects_are_the_config_arithmetic`
checks that the factorial's main effects on `log work` are exactly these logs
with zero interaction.

## 2. Depth and total cost of a nested chain (`budget`)

Let the root budget be `b₀` and each child receive the fraction `f ∈ (0, 1)`
of its parent, so `bₙ = f ⁿ b₀` before flooring. A layer is viable while
`bₙ ≥ m` (`viable_work`). The deepest viable layer, counting the root as depth
1, satisfies `f ^(d−1) b₀ ≥ m`, so

    d_max = 1 + ⌊ log(b₀ / m) / log(1 / f) ⌋                 (budget.rs: max_depth)

The sum of all budgets is a geometric series,

    Σ bₙ ≤ b₀ · Σ f ⁿ = b₀ / (1 − f)                           (budget.rs: total_cost_bound)

so a chain of any depth costs its host less than `1 / (1 − f)` times the root
layer: 4/3 at `f = 1/4`. Both are bounds: budgets are integers and each
generation is floored, so the real chain can be shorter than `d_max` (the
test `truncation_can_cost_the_chain_a_layer` pins an example: `f = 0.75`,
`b₀ = 1000`, `m = 100` gives `d_max = 9` and a built depth of 8) and never
longer; `max_depth_bounds_iteration_across_many_settings` checks the bound
against brute force.

A chain can also end on **space**: `fit_spec` finds no world at or above
`viable_edge` whose exact cost fits the budget. Because lazy rendering charges
by the block and the probe is rescaled with the world, the cost is not
monotone in the edge (`layer::tests::cost_is_not_monotonic_in_world_size`), so
a world may fail to fit while a larger one fits, and a world may fail to fit
although `continuous_work` — the same cost with blocks replaced by area —
would have fitted. `layer::map_terminations` classifies every ending as
budget, space or **quantisation** (the last case) over fractions, floors,
block sizes and root sizes, running no physics. Which definition binds where
is what the map shows; that it is one of the three is a consequence.

## 3. The binomial mean field for unobserved ground (`physics`)

Under the assumption that the cells of an unobserved block are independently
alive with the block's density `p`, a cell with `n` neighbours sees a binomial
count `K ~ Bin(n, p)`, and the rule's bands admit it with probability

    P_band(p) = Σ_{k : lo ≤ k/n ≤ hi} C(n, k) p^k (1 − p)^(n−k)

so the expected next density is

    p' = (1 − p) · P_birth(p) + p · P_survive(p)               (physics.rs: binomial_next)

with `p` taken as a mix of the block's own density and its neighbours' in the
proportion of neighbour reads that fall inside the block (`own_share`). For
B3/S23 this map has a stable fixed point near 0.37 (`physics::tests::the_binomial_mean_field_of_life_has_the_known_fixed_point`),
while Life itself decays from a random soup toward a few percent. That the
mean-field closure is a poor approximation of Life is well known and is not a
finding of this project; what the Theory 1 experiment measures is how much of
the lazy-rendering result rests on it, by running the same settings under the
indicator and frozen closures as well.

The indicator closure shipped in v0.1–v0.9 applied the rule's *indicator* to
the neighbour mean instead: `p' = (1−p)·[lo_b ≤ p̄ ≤ hi_b] + p·[lo_s ≤ p̄ ≤ hi_s]`,
which is 1 whenever the neighbour mean sits in the birth band. It is not an
expectation of anything and is kept only for reproduction of the earlier
results (`docs/audit.md` §1.1).

## 4. The lattice's anisotropy (`detector`)

A Moore neighbourhood of radius 1 contains cells at straight-line distance 1
(along an axis) and √2 (on a diagonal), and no others. A newly live cell's
nearest previously live cell is therefore at distance 1 or √2, and the ratio
of the largest diagonal reach to the largest axis reach is exactly √2 as soon
as one birth of each kind has occurred. Subdividing the lattice changes the
unit, not the ratio. So "the lattice's shape reads √2 and its scale does not
show" is geometry; the only empirical content is that natural births reach the
corner, which a Life soup of thousands of cells guarantees in practice. The
`anisotropy` rows of the detection survey are labelled `consequence` for this
reason.

Likewise, under the speed cap a birth needs live cells within one
neighbourhood per substep, so the greatest distance from a birth to its
nearest ancestor in one tick is at most `radius × substeps`
(`detector::tests::observed_speed_never_exceeds_its_bound`). That the
inhabitant reads this product, and cannot factor it, is a consequence.
Whether the ceiling is *reached* in a given region is not, and the survey
reports the false-positive rate that follows.

## 5. The pipe's designed-in half (`pipe`)

`Message::pack` stores a magnitude and the top bits of a position-sensitive
hash. The magnitude carries the horizon's occupancy by construction; the hash
is designed to avalanche, so `avalanche` reading about one half is a test of
the fold, and the hash carrying no information about occupancy
(`information` rows `hash:*`) is a property of hashing. Bits spent on the
magnitude saturate at the `f64` mantissa (53), so widths above 64 bits add
only digest bits. What is not a consequence is how many bits each *task*
survives on, and whether the horizon does anything a window elsewhere does
not; both are measured.
