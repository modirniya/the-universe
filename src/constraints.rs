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
}

impl Default for Params {
    fn default() -> Self {
        Params {
            subdivision: 2,
            substeps: 2,
            capped_radius: 1,
            uncapped_radius: 3,
            block_size: 16,
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
}
