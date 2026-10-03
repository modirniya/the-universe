//! Theory 1: limits as optimizations.
//!
//! Each field here is a physical limit that a creator might have written into
//! a universe to make it cheaper to run. Turning one off does not add a
//! feature; it *removes* an optimization and buys a more expensive, more
//! faithful universe. The experiment in [`crate::experiment`] pays for both
//! and compares them.
//!
//! Falsified within the model if: switching a constraint on lowers cost by a
//! negligible margin, or changes the macro observables so much that the
//! cheaper universe is plainly a different universe. Either outcome refutes
//! "the creator got this for free" *for that constraint*.

use serde::Deserialize;

/// The four toggles. `true` means the limit is in force (the cheap universe).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Constraints {
    /// Planck-style pixelation. Off: space is subdivided `subdivision` times
    /// per axis, so the same region costs `subdivision^2` more cells.
    pub discrete_space: bool,
    /// One atomic update per tick. Off: `substeps` sub-updates per tick.
    pub discrete_time: bool,
    /// Influence travels at most `capped_radius` cells per substep.
    /// Off: `uncapped_radius`, so each cell reads a far larger neighbourhood.
    pub speed_cap: bool,
    /// Regions are computed at full resolution only while observed.
    /// Off: every region is computed in full, every tick, observed or not.
    pub lazy_rendering: bool,
}

impl Constraints {
    /// All limits in force: the cheapest universe the model can build.
    pub const ALL_ON: Constraints = Constraints {
        discrete_space: true,
        discrete_time: true,
        speed_cap: true,
        lazy_rendering: true,
    };

    /// No limits: the expensive, maximally faithful universe. This is the
    /// reference the constrained runs are judged against.
    pub const ALL_OFF: Constraints = Constraints {
        discrete_space: false,
        discrete_time: false,
        speed_cap: false,
        lazy_rendering: false,
    };

    /// A short stable label used in filenames and report rows.
    pub fn label(&self) -> String {
        if *self == Self::ALL_ON {
            return "all_on".to_string();
        }
        if *self == Self::ALL_OFF {
            return "all_off".to_string();
        }
        let mut on: Vec<&str> = Vec::new();
        if self.discrete_space {
            on.push("space");
        }
        if self.discrete_time {
            on.push("time");
        }
        if self.speed_cap {
            on.push("speed");
        }
        if self.lazy_rendering {
            on.push("lazy");
        }
        if on.is_empty() {
            "none".to_string()
        } else {
            on.join("+")
        }
    }

    /// Every single-constraint universe: exactly one limit in force.
    pub fn singles() -> Vec<Constraints> {
        let mut out = Vec::new();
        for i in 0..4 {
            let mut c = Self::ALL_OFF;
            match i {
                0 => c.discrete_space = true,
                1 => c.discrete_time = true,
                2 => c.speed_cap = true,
                _ => c.lazy_rendering = true,
            }
            out.push(c);
        }
        out
    }
}

/// How an unobserved block's density advances while nothing computes its
/// cells.
///
/// Lazy rendering says unobserved regions are not computed in detail. It does
/// not say what stands in for them, and the choice is an **assumption of the
/// model**, not a consequence of the theory: every result about lazy rendering
/// is a result about lazy rendering *under one of these closures*. The
/// experiment therefore runs the lazy settings under each and reports them side
/// by side.
///
/// - `Indicator` is what v0.1–v0.9 shipped: the rule's birth and survival
///   tests applied to the mean density of the neighbouring blocks. Its doc
///   comment called it the expected outcome; it is not. A block whose
///   neighbours sit in the birth band jumps to `(1 - d) + d = 1` in one
///   substep, and the audit (`docs/audit.md` §1.1) found unobserved ground
///   oscillating between near-empty and near-full under it. It is kept so the
///   earlier results can be reproduced and compared, not because it is a
///   defensible approximation.
/// - `Binomial` is the expected next density if the block's occupants were
///   independently alive with the block's own density: the rule averaged over
///   a binomial neighbour count. This is what a mean field means, and it is
///   the default from v1.0.
/// - `Frozen` holds the density constant. Unobserved ground is literally not
///   computed, which is the cheapest reading of the theory and the one the
///   viewer's caption ("not being computed at all") describes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CoarseRule {
    Indicator,
    #[default]
    Binomial,
    Frozen,
}

impl CoarseRule {
    pub const ALL: [CoarseRule; 3] = [
        CoarseRule::Binomial,
        CoarseRule::Indicator,
        CoarseRule::Frozen,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            CoarseRule::Indicator => "indicator",
            CoarseRule::Binomial => "binomial",
            CoarseRule::Frozen => "frozen",
        }
    }

    /// The inverse of [`Self::label`].
    pub fn parse(name: &str) -> Option<CoarseRule> {
        CoarseRule::ALL.into_iter().find(|r| r.label() == name)
    }
}

/// The magnitudes behind the toggles. These are the creator's dials; the
/// toggles only choose which end of each dial is used.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    /// Cells per base cell per axis when `discrete_space` is off.
    pub subdivision: usize,
    /// Physics substeps per tick when `discrete_time` is off.
    pub substeps: usize,
    /// Influence radius in cells when `speed_cap` is on.
    pub capped_radius: usize,
    /// Influence radius in cells when `speed_cap` is off.
    pub uncapped_radius: usize,
    /// Edge length, in base cells, of a lazily rendered region.
    pub block_size: usize,
    /// What stands in for an unobserved block's cells. See [`CoarseRule`].
    #[serde(default)]
    pub coarse_rule: CoarseRule,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            subdivision: 2,
            substeps: 2,
            capped_radius: 1,
            uncapped_radius: 3,
            block_size: 16,
            coarse_rule: CoarseRule::default(),
        }
    }
}

/// What the toggles and dials work out to for one run.
///
/// Note the coupling this exposes, which is a result of the model rather than
/// an assumption fed into it: influence per *tick* is `radius * substeps`
/// cells, and a cell is `1 / subdivision` of a base length. So refining time
/// without refining space raises the physical speed of influence. Discrete
/// time and the speed cap are not independent limits; the creator cannot
/// relax one without paying in the other. See README, "Findings".
#[derive(Clone, Copy, Debug)]
pub struct Resolved {
    pub subdivision: usize,
    pub substeps: usize,
    pub radius: usize,
    pub block_size: usize,
    pub lazy: bool,
    pub coarse_rule: CoarseRule,
}

impl Resolved {
    pub fn new(c: &Constraints, p: &Params) -> Self {
        let subdivision = if c.discrete_space { 1 } else { p.subdivision };
        Resolved {
            subdivision,
            substeps: if c.discrete_time { 1 } else { p.substeps },
            radius: if c.speed_cap {
                p.capped_radius
            } else {
                p.uncapped_radius
            },
            block_size: p.block_size * subdivision,
            lazy: c.lazy_rendering,
            coarse_rule: p.coarse_rule,
        }
    }

    /// Neighbours a cell reads per substep: the Moore neighbourhood of this
    /// radius, less the cell itself.
    pub fn neighbours(&self) -> u64 {
        ((2 * self.radius + 1) * (2 * self.radius + 1) - 1) as u64
    }

    /// Neighbour visits one unobserved block costs per substep.
    ///
    /// The indicator and binomial closures read the eight neighbouring block
    /// densities; a frozen block reads nothing. This is the whole of lazy
    /// rendering's cost arithmetic and `layer::predict_work` relies on it
    /// being exact.
    pub fn coarse_visits(&self) -> u64 {
        match self.coarse_rule {
            CoarseRule::Indicator | CoarseRule::Binomial => 8,
            CoarseRule::Frozen => 0,
        }
    }

    /// Influence speed in *base* cell lengths per tick. The speed of light of
    /// this universe, in units comparable across resolutions.
    pub fn influence_speed(&self) -> f64 {
        (self.radius * self.substeps) as f64 / self.subdivision as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_stable() {
        assert_eq!(Constraints::ALL_ON.label(), "all_on");
        assert_eq!(Constraints::ALL_OFF.label(), "all_off");
    }

    #[test]
    fn singles_turn_on_exactly_one() {
        let s = Constraints::singles();
        assert_eq!(s.len(), 4);
        for c in s {
            let n = [
                c.discrete_space,
                c.discrete_time,
                c.speed_cap,
                c.lazy_rendering,
            ]
            .iter()
            .filter(|b| **b)
            .count();
            assert_eq!(n, 1, "{c:?}");
        }
    }

    #[test]
    fn constraints_lower_the_work_dials() {
        let p = Params::default();
        let on = Resolved::new(&Constraints::ALL_ON, &p);
        let off = Resolved::new(&Constraints::ALL_OFF, &p);
        assert!(on.subdivision <= off.subdivision);
        assert!(on.substeps <= off.substeps);
        assert!(on.radius <= off.radius);
    }

    #[test]
    fn refining_time_alone_raises_influence_speed() {
        // The coupling documented on `Resolved`.
        let p = Params::default();
        let base = Resolved::new(&Constraints::ALL_ON, &p);
        let mut finer_time = Constraints::ALL_ON;
        finer_time.discrete_time = false;
        let ft = Resolved::new(&finer_time, &p);
        assert!(ft.influence_speed() > base.influence_speed());

        // Refining space alongside it pays the debt back.
        let mut both = finer_time;
        both.discrete_space = false;
        let b = Resolved::new(&both, &p);
        assert!(b.influence_speed() <= ft.influence_speed());
    }

    #[test]
    fn the_default_closure_is_binomial_and_every_closure_has_a_label() {
        assert_eq!(Params::default().coarse_rule, CoarseRule::Binomial);
        let labels: std::collections::BTreeSet<&str> =
            CoarseRule::ALL.iter().map(|r| r.label()).collect();
        assert_eq!(labels.len(), 3);
    }

    #[test]
    fn labels_round_trip_through_parse() {
        for r in CoarseRule::ALL {
            assert_eq!(CoarseRule::parse(r.label()), Some(r));
        }
        assert_eq!(CoarseRule::parse("nonsense"), None);
    }

    #[test]
    fn a_frozen_block_costs_nothing_to_advance() {
        let p = Params {
            coarse_rule: CoarseRule::Frozen,
            ..Params::default()
        };
        assert_eq!(Resolved::new(&Constraints::ALL_ON, &p).coarse_visits(), 0);
        assert_eq!(
            Resolved::new(&Constraints::ALL_ON, &Params::default()).coarse_visits(),
            8
        );
        assert_eq!(Resolved::new(&Constraints::ALL_ON, &p).neighbours(), 8);
        assert_eq!(Resolved::new(&Constraints::ALL_OFF, &p).neighbours(), 48);
    }
}
