//! Theory 2: the degradation rule.
//!
//! A universe that hosts a child cannot give that child everything, because it
//! still has to run itself. So a child's budget is a strict fraction of its
//! parent's, and the consequence the framework cannot avoid is that the chain
//! is **finite**: each layer is poorer than the one above, and somewhere down
//! the chain is a layer too poor to host anything.
//!
//! That cuts against the intuition that a simulation chain could be infinite.
//! It also means the maximum depth is not a tuning parameter but a *derived*
//! quantity — [`Degradation::max_depth`] computes it in closed form from the
//! root budget alone, and `layer.rs` checks that actually building the chain
//! produces the same number.
//!
//! Budgets are denominated in **neighbour visits**, the same reproducible work
//! counter the v0.1 experiment used. Wall time would have been the wrong unit:
//! a budget that means different things on different machines is not a budget.
//!
//! Falsified within the model if: a chain runs deeper than the closed form
//! predicts, a layer spends more than it was given, or the total cost of an
//! arbitrarily deep chain fails to converge.

use serde::Deserialize;

/// What a layer may spend, in neighbour visits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Budget {
    pub work: u64,
}

impl Budget {
    pub fn new(work: u64) -> Self {
        Budget { work }
    }
}

/// How much poorer each layer is than its parent, and when the chain stops.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Degradation {
    /// Share of a parent's budget each child receives. Must be in `(0, 1)`:
    /// at 1 the parent has nothing left to run itself, and above 1 the child
    /// is richer than its host, which is the one thing nesting cannot allow.
    pub fraction: f64,
    /// Below this many neighbour visits, a universe cannot run at all.
    pub viable_work: u64,
    /// Below this edge length in cells, a world is too small to be a world.
    pub viable_edge: usize,
}

impl Default for Degradation {
    fn default() -> Self {
        Degradation {
            fraction: 0.25,
            viable_work: 1_000_000,
            viable_edge: 16,
        }
    }
}

impl Degradation {
    /// Whether these settings describe a chain that can terminate.
    pub fn validate(&self) -> Result<(), String> {
        if !(self.fraction > 0.0 && self.fraction < 1.0) {
            return Err(format!(
                "nesting.fraction must be in (0, 1), got {}: at 1 the parent keeps nothing \
                 for itself, and above 1 the child is richer than its host",
                self.fraction
            ));
        }
        if self.viable_work == 0 {
            return Err(
                "nesting.viable_work must be greater than 0, or the chain never ends".into(),
            );
        }
        if self.viable_edge == 0 {
            return Err("nesting.viable_edge must be greater than 0".into());
        }
        Ok(())
    }

    /// The child's share, or `None` once there is too little left to run a
    /// universe with.
    pub fn child_of(&self, parent: Budget) -> Option<Budget> {
        let work = (parent.work as f64 * self.fraction).floor() as u64;
        (work >= self.viable_work).then_some(Budget::new(work))
    }

    /// Upper bound on how deep a chain rooted at this budget can go.
    ///
    /// Counting the root as depth 1, this is the largest `d` with
    /// `root * fraction^(d-1) >= viable_work`. Derived rather than iterated,
    /// so that building the chain has something independent to check against.
    ///
    /// It is a **bound, not an equality**. Budgets are integers and each
    /// generation is floored, so a real chain loses a little at every step and
    /// can come up short of the ideal geometric depth — with `fraction = 0.75`
    /// and a root of 1000 against a viable minimum of 100, this returns 9
    /// while the chain that actually builds is 8 deep. Degradation is slightly
    /// worse in practice than the continuous maths predicts, which is the
    /// direction that flatters the theory least and so is worth stating.
    pub fn max_depth(&self, root: Budget) -> usize {
        if root.work < self.viable_work {
            return 0;
        }
        let ratio = (root.work as f64) / (self.viable_work as f64);
        let steps = ratio.log(1.0 / self.fraction).floor() as usize;
        steps + 1
    }

    /// Upper bound on the whole chain's cost, however deep it goes.
    ///
    /// The budgets form a geometric series, so an infinitely deep chain would
    /// still cost at most `root / (1 - fraction)`. Nesting is bounded in total
    /// spend, not merely in depth — which is the more interesting half of the
    /// claim, since it means a parent can host a chain without the chain
    /// eventually costing more than the parent.
    pub fn total_cost_bound(&self, root: Budget) -> f64 {
        root.work as f64 / (1.0 - self.fraction)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deg() -> Degradation {
        Degradation {
            fraction: 0.5,
            viable_work: 100,
            viable_edge: 4,
        }
    }

    #[test]
    fn a_child_is_strictly_poorer_than_its_parent() {
        let d = deg();
        let parent = Budget::new(10_000);
        let child = d.child_of(parent).expect("should afford a child");
        assert!(child.work < parent.work, "nesting must degrade");
    }

    #[test]
    fn the_chain_terminates() {
        let d = deg();
        let mut b = Budget::new(10_000);
        let mut depth = 1;
        while let Some(next) = d.child_of(b) {
            b = next;
            depth += 1;
            assert!(depth < 1000, "chain failed to terminate");
        }
        assert_eq!(depth, d.max_depth(Budget::new(10_000)));
    }

    #[test]
    fn max_depth_bounds_iteration_across_many_settings() {
        // The closed form is what `layer.rs` is checked against, so it is
        // worth checking against brute force first. Integer flooring means a
        // real chain can be shorter, never longer.
        for fraction in [0.1, 0.25, 0.333, 0.5, 0.75, 0.9] {
            for root in [1_000u64, 50_000, 1_000_000, 987_654] {
                let d = Degradation {
                    fraction,
                    viable_work: 100,
                    viable_edge: 4,
                };
                let mut b = Budget::new(root);
                let mut counted = if root >= 100 { 1 } else { 0 };
                while let Some(next) = d.child_of(b) {
                    b = next;
                    counted += 1;
                }
                let bound = d.max_depth(Budget::new(root));
                assert!(
                    counted <= bound,
                    "fraction {fraction}, root {root}: built {counted}, bound {bound}"
                );
                assert!(
                    counted + 1 >= bound,
                    "fraction {fraction}, root {root}: bound {bound} is far above the real {counted}"
                );
            }
        }
    }

    #[test]
    fn truncation_can_cost_the_chain_a_layer() {
        // The documented example, pinned so the caveat cannot quietly stop
        // being true.
        let d = Degradation {
            fraction: 0.75,
            viable_work: 100,
            viable_edge: 4,
        };
        let root = Budget::new(1_000);
        let mut b = root;
        let mut counted = 1;
        while let Some(next) = d.child_of(b) {
            b = next;
            counted += 1;
        }
        assert_eq!(counted, 8);
        assert_eq!(d.max_depth(root), 9);
    }

    #[test]
    fn a_root_too_poor_to_run_has_no_depth() {
        assert_eq!(deg().max_depth(Budget::new(99)), 0);
        assert_eq!(deg().max_depth(Budget::new(100)), 1);
    }

    #[test]
    fn total_cost_converges() {
        let d = deg();
        let root = Budget::new(10_000);
        let mut total = root.work as f64;
        let mut b = root;
        while let Some(next) = d.child_of(b) {
            total += next.work as f64;
            b = next;
        }
        assert!(
            total <= d.total_cost_bound(root),
            "chain cost {total} exceeded the geometric bound {}",
            d.total_cost_bound(root)
        );
    }

    #[test]
    fn a_deeper_chain_still_respects_the_bound() {
        // Small fraction, huge root: the bound must hold for long chains too.
        let d = Degradation {
            fraction: 0.9,
            viable_work: 1,
            viable_edge: 4,
        };
        let root = Budget::new(1_000_000);
        let mut total = root.work as f64;
        let mut b = root;
        while let Some(next) = d.child_of(b) {
            total += next.work as f64;
            b = next;
        }
        assert!(total <= d.total_cost_bound(root));
    }

    #[test]
    fn a_fraction_at_or_above_one_is_refused() {
        for f in [1.0, 1.5, 0.0, -0.2] {
            let d = Degradation {
                fraction: f,
                ..deg()
            };
            assert!(d.validate().is_err(), "fraction {f} should be refused");
        }
        assert!(deg().validate().is_ok());
    }
}
