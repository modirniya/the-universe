//! Theory 2: nesting. Layers hosting layers, under the degradation rule.
//!
//! **Layer 0** is the host machine's process. The universes this module builds
//! are layers 1, 2, 3 and down, each hosted by the one above it and running on
//! a strict fraction of that host's budget (see [`crate::budget`]).
//!
//! Two things follow, and both are measured rather than assumed:
//!
//! - The chain **terminates**. [`Chain::predicted_max_depth`] is computed in
//!   closed form from the root budget before any universe is built, and the
//!   chain that actually gets built is checked against it.
//! - Layers get **poorer, and therefore smaller**. A child's world is sized to
//!   fit its budget, so degradation shows up as a shrinking universe rather
//!   than as an abstract number.
//!
//! What a layer does *not* have is any way to reach its neighbours. Nesting is
//! only the containment relation; the one-way serializing channel between
//! layers is the pipe, and it is v0.3. Layers here are blind to each other by
//! omission rather than by design, which is worth stating so that the omission
//! is not mistaken for a claim.
//!
//! Nested layers run with every constraint in force. A universe living on a
//! quarter of its host's resources would take every optimization available,
//! and v0.1 established what those are worth.
//!
//! Falsified within the model if: the built chain is deeper than the closed
//! form allows, a layer spends more work than it was given, or degradation
//! turns out not to cost anything — a child indistinguishable from its parent
//! would mean the degradation rule has no consequences worth modelling.

use crate::budget::{Budget, Degradation};
use crate::config::Config;
use crate::constraints::{Constraints, Resolved};
use crate::experiment::{RunResult, run};
use crate::observer::Probe;
use crate::physics::Work;
use crate::space::{Geometry, macro_divergence};

/// The size of one universe.
///
/// Ticks are held constant down the chain: a child is given less *space*, not
/// less history, so that what degradation costs shows up as a smaller world
/// rather than as a shorter one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayerSpec {
    pub width: usize,
    pub height: usize,
    pub ticks: u64,
}

impl LayerSpec {
    pub fn cells(&self) -> usize {
        self.width * self.height
    }
}

/// One universe in the chain, before it has been run.
#[derive(Clone, Copy, Debug)]
pub struct Layer {
    /// 1 is the root simulated universe. Layer 0 is the host process.
    pub depth: usize,
    pub budget: Budget,
    pub spec: LayerSpec,
    pub predicted_work: u64,
}

/// One universe in the chain, after it has been run.
#[derive(Clone, Debug)]
pub struct LayerResult {
    pub layer: Layer,
    /// Whether the layer stayed inside the budget it was given. The chain is
    /// incoherent if this is ever false.
    pub within_budget: bool,
    /// Fraction of the budget actually spent.
    pub budget_used: f64,
    pub work: Work,
    pub final_live_fraction: f64,
    /// Mean tick-to-tick change in the macro field.
    ///
    /// A universe can be full of live cells and still be doing nothing. Churn
    /// separates those cases, which matters here because the question is not
    /// whether a deep layer *exists* but whether it is still interesting.
    pub churn: f64,
    /// True when churn has fallen far enough that the layer has stopped
    /// producing anything, however alive it looks.
    pub sterile: bool,
}

/// A whole chain, and the prediction it was checked against.
#[derive(Clone, Debug)]
pub struct Chain {
    pub root_budget: Budget,
    pub degradation: Degradation,
    /// Computed in closed form before the chain was built.
    pub predicted_max_depth: usize,
    pub layers: Vec<LayerResult>,
    /// Total neighbour visits across every layer.
    pub total_work: u64,
    /// The geometric bound the total must respect.
    pub total_cost_bound: f64,
}

impl Chain {
    /// Depth of the deepest layer that was still doing something.
    ///
    /// The honest answer to "how many interesting layers is this", as distinct
    /// from how many the budget technically affords.
    pub fn productive_depth(&self) -> usize {
        self.layers
            .iter()
            .filter(|l| !l.sterile)
            .map(|l| l.layer.depth)
            .max()
            .unwrap_or(0)
    }
}

/// Below this mean tick-to-tick change, a layer is treated as sterile.
///
/// Deliberately low: the claim being tested is that deep layers go dead, so the
/// threshold should be hard to trip accidentally in that direction.
pub const STERILE_CHURN: f64 = 0.001;

/// Neighbour visits one layer will spend, computed exactly.
///
/// Not an estimate. It lays out the same geometry the run will use and counts
/// what each block costs: an observed block pays per cell, an unobserved one
/// pays eight visits however large it is. That is the whole of lazy rendering's
/// arithmetic, so the number here is what the layer actually spends, which is
/// what makes it usable as a budget check rather than a guess.
///
/// The probe must be the one that layer will run with, not the root's: a
/// smaller world observed by an unscaled probe would resolve a quite different
/// share of its blocks.
pub fn predict_work(spec: &LayerSpec, probe: &Probe, cfg: &Config) -> u64 {
    let res = Resolved::new(&Constraints::ALL_ON, &cfg.params);
    let geom = Geometry::new(spec.width, spec.height, res.subdivision, res.block_size);
    let neighbours = ((2 * res.radius + 1) * (2 * res.radius + 1) - 1) as u64;
    let observed = probe.observed_blocks(&geom);

    let per_substep: u64 = (0..geom.blocks())
        .map(|b| {
            if observed[b] {
                geom.block_cells(b) as u64 * neighbours
            } else {
                8
            }
        })
        .sum();

    per_substep * spec.ticks * res.substeps as u64
}

/// Rescale the probe so it covers the same fraction of a smaller world.
///
/// What changes down the chain should be the universe, not the observer.
pub fn scale_probe(probe: &Probe, root: &LayerSpec, spec: &LayerSpec) -> Probe {
    let sx = spec.width as f64 / root.width as f64;
    let sy = spec.height as f64 / root.height as f64;
    let origin = |v: usize, s: f64, extent: usize| {
        ((v as f64 * s).round() as usize).min(extent.saturating_sub(1))
    };
    Probe {
        x: origin(probe.x, sx, spec.width),
        y: origin(probe.y, sy, spec.height),
        width: ((probe.width as f64 * sx).round() as usize).clamp(1, spec.width),
        height: ((probe.height as f64 * sy).round() as usize).clamp(1, spec.height),
    }
}

/// The largest world that fits a budget, keeping the root's aspect ratio and
/// never exceeding the host's own size.
///
/// This is a scan rather than algebra, and deliberately so. Lazy rendering
/// charges by the *block*, so cost is quantized — and because the probe is
/// rescaled with the world, a probe that lands on block boundaries resolves far
/// fewer blocks than one of the same area that straddles them. The result is
/// that **cost is not monotonic in world size**: with the default config a
/// 48x48 world costs 3,686,400 while a 64x64 world costs 1,657,600, because at
/// 48 the probe straddles boundaries and drags the whole world into full
/// resolution, and at 64 it does not.
///
/// So walking down from an area estimate would find *a* world that fits while
/// stepping straight past larger ones that also fit. Scanning the range and
/// taking the largest is the only way to mean "largest" here.
///
/// Returns `None` when no world at or above [`Degradation::viable_edge`] fits —
/// the second way a chain can end, and the one that bites first when the blocks
/// are large relative to the world.
pub fn fit_spec(
    budget: Budget,
    root: &LayerSpec,
    max_edge: usize,
    deg: &Degradation,
    cfg: &Config,
) -> Option<LayerSpec> {
    let aspect = root.width as f64 / root.height as f64;

    (deg.viable_edge..=max_edge)
        .rev()
        .map(|height| LayerSpec {
            width: ((height as f64) * aspect).round().max(1.0) as usize,
            height,
            ticks: root.ticks,
        })
        .find(|spec| {
            spec.width >= deg.viable_edge
                && predict_work(spec, &scale_probe(&cfg.observer, root, spec), cfg) <= budget.work
        })
}

/// Build the chain without running it.
pub fn build(root_budget: Budget, root: &LayerSpec, deg: &Degradation, cfg: &Config) -> Vec<Layer> {
    let mut layers = Vec::new();
    let mut budget = root_budget;
    let mut depth = 1;
    // A child is never larger than its host, whatever its budget would allow.
    // Nesting is containment, and a contained universe cannot be the bigger of
    // the two.
    let mut max_edge = root.height;

    while let Some(spec) = fit_spec(budget, root, max_edge, deg, cfg) {
        layers.push(Layer {
            depth,
            budget,
            spec,
            predicted_work: predict_work(&spec, &scale_probe(&cfg.observer, root, &spec), cfg),
        });
        max_edge = spec.height;
        match deg.child_of(budget) {
            Some(next) => budget = next,
            None => break,
        }
        depth += 1;
    }

    layers
}

/// Mean tick-to-tick change in the macro field.
fn churn_of(r: &RunResult) -> f64 {
    if r.macro_trace.len() < 2 {
        return 0.0;
    }
    r.macro_trace
        .windows(2)
        .map(|w| macro_divergence(&w[0], &w[1]))
        .sum::<f64>()
        / (r.macro_trace.len() - 1) as f64
}

/// Build and run the whole chain.
///
/// Each layer is an ordinary v0.1 universe with every constraint in force,
/// sized to its budget. `on_layer` is called before each one so a caller can
/// report progress on a long chain.
pub fn run_chain(
    cfg: &Config,
    root_budget: Budget,
    deg: &Degradation,
    mut on_layer: impl FnMut(usize, &LayerSpec),
) -> Chain {
    let root = LayerSpec {
        width: cfg.world.width,
        height: cfg.world.height,
        ticks: cfg.world.ticks,
    };
    let planned = build(root_budget, &root, deg, cfg);

    let mut layers = Vec::with_capacity(planned.len());
    let mut total_work = 0u64;

    for layer in planned {
        on_layer(layer.depth, &layer.spec);

        let mut layer_cfg = cfg.clone();
        layer_cfg.world.width = layer.spec.width;
        layer_cfg.world.height = layer.spec.height;
        layer_cfg.world.ticks = layer.spec.ticks;
        layer_cfg.observer = scale_probe(&cfg.observer, &root, &layer.spec);

        let r = run(&layer_cfg, Constraints::ALL_ON);
        let spent = r.work.neighbor_visits;
        total_work += spent;

        let churn = churn_of(&r);
        layers.push(LayerResult {
            layer,
            within_budget: spent <= layer.budget.work,
            budget_used: spent as f64 / layer.budget.work as f64,
            work: r.work,
            final_live_fraction: r.final_live_fraction,
            churn,
            sterile: churn < STERILE_CHURN,
        });
    }

    Chain {
        root_budget,
        degradation: *deg,
        predicted_max_depth: deg.max_depth(root_budget),
        layers,
        total_work,
        total_cost_bound: deg.total_cost_bound(root_budget),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ReportCfg, WorldCfg};
    use crate::constraints::Params;
    use crate::physics::Rules;

    fn cfg() -> Config {
        Config {
            world: WorldCfg {
                width: 64,
                height: 64,
                ticks: 30,
                seed: 42,
                init_density: 0.3,
            },
            rules: Rules::default(),
            constraints: Constraints::ALL_ON,
            params: Params {
                block_size: 8,
                ..Params::default()
            },
            observer: Probe {
                x: 16,
                y: 16,
                width: 32,
                height: 32,
            },
            report: ReportCfg {
                macro_grid: 8,
                out_dir: "out".into(),
            },
            nesting: Degradation::default(),
        }
    }

    fn deg() -> Degradation {
        Degradation {
            fraction: 0.25,
            viable_work: 50_000,
            viable_edge: 8,
        }
    }

    fn root_spec(c: &Config) -> LayerSpec {
        LayerSpec {
            width: c.world.width,
            height: c.world.height,
            ticks: c.world.ticks,
        }
    }

    fn work_of(c: &Config, root: &LayerSpec, edge: usize) -> u64 {
        let s = LayerSpec {
            width: edge,
            height: edge,
            ticks: root.ticks,
        };
        predict_work(&s, &scale_probe(&c.observer, root, &s), c)
    }

    /// The root budgeted at exactly what its own world costs -- the default
    /// `nest` uses, and the only setting under which every child is forced to
    /// shrink.
    fn tight_root_budget(c: &Config) -> Budget {
        let root = root_spec(c);
        Budget::new(predict_work(&root, &c.observer, c))
    }

    #[test]
    fn a_child_world_is_smaller_than_its_parent() {
        let c = cfg();
        let layers = build(tight_root_budget(&c), &root_spec(&c), &deg(), &c);
        assert!(layers.len() >= 2, "expected a chain, got {}", layers.len());
        for pair in layers.windows(2) {
            assert!(
                pair[1].spec.cells() < pair[0].spec.cells(),
                "layer {} was not smaller than layer {}",
                pair[1].depth,
                pair[0].depth
            );
        }
    }

    #[test]
    fn a_child_with_slack_may_keep_its_hosts_size() {
        // Degradation says a child is *poorer*, not that it is smaller. When
        // the root is handed far more than its world costs, it is running
        // below its means and so is its child, and neither has to shrink.
        // Pinned because it looks like a bug and is not: shrinkage is derived
        // from scarcity here, never imposed.
        let c = cfg();
        let layers = build(Budget::new(500_000_000), &root_spec(&c), &deg(), &c);
        assert!(layers.len() >= 2);
        assert_eq!(layers[0].spec, layers[1].spec);
        assert!(layers[1].budget.work < layers[0].budget.work, "but poorer");
    }

    #[test]
    fn a_child_is_never_larger_than_its_host() {
        let c = cfg();
        let layers = build(Budget::new(50_000_000), &root_spec(&c), &deg(), &c);
        for pair in layers.windows(2) {
            assert!(pair[1].spec.height <= pair[0].spec.height);
            assert!(pair[1].spec.width <= pair[0].spec.width);
        }
    }

    #[test]
    fn the_built_chain_never_exceeds_the_closed_form() {
        let c = cfg();
        let d = deg();
        for root_work in [1_000_000u64, 20_000_000, 500_000_000] {
            let root = Budget::new(root_work);
            let built = build(root, &root_spec(&c), &d, &c).len();
            assert!(
                built <= d.max_depth(root),
                "built {built} layers, closed form allows {}",
                d.max_depth(root)
            );
        }
    }

    #[test]
    fn depths_are_consecutive_from_one() {
        let c = cfg();
        let layers = build(Budget::new(50_000_000), &root_spec(&c), &deg(), &c);
        for (i, l) in layers.iter().enumerate() {
            assert_eq!(
                l.depth,
                i + 1,
                "layer 0 is the host process, not a universe"
            );
        }
    }

    #[test]
    fn every_layer_stays_inside_its_budget() {
        // The invariant that makes the chain coherent: a child cannot outspend
        // what its host gave it.
        let c = cfg();
        let chain = run_chain(&c, Budget::new(20_000_000), &deg(), |_, _| {});
        assert!(!chain.layers.is_empty());
        for l in &chain.layers {
            assert!(
                l.within_budget,
                "layer {} spent {} of {}",
                l.layer.depth, l.work.neighbor_visits, l.layer.budget.work
            );
        }
    }

    #[test]
    fn the_prediction_is_exact() {
        // `predict_work` counts the same blocks the run will count, so it
        // should land on the number rather than near it. If this ever drifts,
        // the budget check has quietly become an approximation.
        let c = cfg();
        let chain = run_chain(&c, Budget::new(20_000_000), &deg(), |_, _| {});
        assert!(!chain.layers.is_empty());
        for l in &chain.layers {
            assert_eq!(
                l.work.neighbor_visits, l.layer.predicted_work,
                "layer {} spent {} but was predicted to spend {}",
                l.layer.depth, l.work.neighbor_visits, l.layer.predicted_work
            );
        }
    }

    #[test]
    fn the_chain_respects_the_geometric_bound() {
        let c = cfg();
        let chain = run_chain(&c, Budget::new(20_000_000), &deg(), |_, _| {});
        assert!(
            (chain.total_work as f64) <= chain.total_cost_bound,
            "chain cost {} exceeded bound {}",
            chain.total_work,
            chain.total_cost_bound
        );
    }

    #[test]
    fn a_root_too_poor_to_run_produces_no_chain() {
        let c = cfg();
        let chain = run_chain(&c, Budget::new(10), &deg(), |_, _| {});
        assert!(chain.layers.is_empty());
        assert_eq!(chain.productive_depth(), 0);
    }

    #[test]
    fn the_chain_is_reproducible() {
        let c = cfg();
        let a = run_chain(&c, Budget::new(20_000_000), &deg(), |_, _| {});
        let b = run_chain(&c, Budget::new(20_000_000), &deg(), |_, _| {});
        assert_eq!(a.layers.len(), b.layers.len());
        for (x, y) in a.layers.iter().zip(b.layers.iter()) {
            assert_eq!(x.work, y.work);
            assert_eq!(x.churn, y.churn);
            assert_eq!(x.layer.spec, y.layer.spec);
        }
    }

    #[test]
    fn the_probe_covers_roughly_the_same_share_of_every_layer() {
        let c = cfg();
        let root = root_spec(&c);
        let layers = build(Budget::new(50_000_000), &root, &deg(), &c);
        let want = c.observer.coverage(root.width, root.height);
        for l in &layers {
            let p = scale_probe(&c.observer, &root, &l.spec);
            let got = p.coverage(l.spec.width, l.spec.height);
            assert!(
                (got - want).abs() < 0.15,
                "layer {} probe covers {got:.3}, root covers {want:.3}",
                l.depth
            );
        }
    }

    #[test]
    fn cost_is_not_monotonic_in_world_size() {
        // Pins the finding that forced `fit_spec` to be a scan: a larger world
        // can be cheaper, because the rescaled probe lands on block boundaries
        // instead of straddling them. If this ever stops being true the scan
        // could be simplified back to a walk -- but not before.
        let c = cfg();
        let root = root_spec(&c);
        let bigger_is_cheaper = (deg().viable_edge..=root.height).any(|small| {
            ((small + 1)..=root.height)
                .any(|big| work_of(&c, &root, big) < work_of(&c, &root, small))
        });
        assert!(
            bigger_is_cheaper,
            "expected some larger world to cost less than a smaller one"
        );
    }

    #[test]
    fn fit_spec_returns_the_largest_world_that_fits() {
        let c = cfg();
        let root = root_spec(&c);
        let d = deg();
        for budget in [200_000u64, 2_000_000, 9_000_000] {
            let b = Budget::new(budget);
            let Some(got) = fit_spec(b, &root, root.height, &d, &c) else {
                continue;
            };
            for edge in (got.height + 1)..=root.height {
                assert!(
                    work_of(&c, &root, edge) > budget,
                    "{edge}x{edge} also fits budget {budget}, but fit_spec chose {}",
                    got.height
                );
            }
        }
    }

    #[test]
    fn a_smaller_world_is_predicted_to_cost_less_when_alignment_is_held_fixed() {
        // Comparing worlds that are whole multiples of the block size keeps the
        // probe aligned in both, so the quantization effect is factored out and
        // only area is left.
        let c = cfg();
        let root = root_spec(&c);
        assert!(work_of(&c, &root, 32) < work_of(&c, &root, 64));
    }
}
