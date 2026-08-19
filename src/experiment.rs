//! The benchmark harness: the part that turns Theory 1 into a claim you can
//! check.
//!
//! The question is whether a creator's limits make a universe cheaper *without
//! changing what it produces*. So the reference run is the universe with no
//! limits at all — the expensive, maximally faithful one — and every other run
//! is judged against it on two axes:
//!
//! - **Cost**, in neighbour visits (machine-independent, reproducible) and
//!   wall time (neither, reported anyway because it is what a creator would
//!   actually pay).
//! - **Divergence**, the mean absolute difference between macro density fields
//!   at the logging threshold. This is the only fair comparison available
//!   between worlds running at different internal resolutions.
//!
//! A limit that costs little and diverges little is a free lunch: a creator
//! would take it and no inhabitant could tell. A limit that diverges sharply
//! is not a free lunch, and the model says so rather than hiding it.
//!
//! Falsified within the model if: no constraint achieves a large cost saving
//! at low divergence. Then limits-as-optimizations buys the creator nothing,
//! and Theory 1 fails on its own terms.

use crate::config::Config;
use crate::constraints::{Constraints, Resolved};
use crate::observer::observe;
use crate::physics::{Work, tick};
use crate::space::{Geometry, World, macro_divergence};
use std::time::Instant;

/// Everything one universe did, and what it cost.
#[derive(Clone, Debug)]
pub struct RunResult {
    pub label: String,
    pub constraints: Constraints,
    pub resolved: Resolved,
    /// Speed of influence in base cells per tick.
    pub influence_speed: f64,
    pub wall_ms: f64,
    pub work: Work,
    /// Peak bytes a resource-honest implementation would hold.
    pub peak_live_bytes: usize,
    /// Bytes this implementation actually allocated.
    pub allocated_bytes: usize,
    pub fine_cells: usize,
    pub final_live_fraction: f64,
    /// Macro density field after each tick.
    pub macro_trace: Vec<Vec<f64>>,
    /// Occupancy of the whole world after each tick.
    pub live_trace: Vec<f64>,
}

impl RunResult {
    /// Mean divergence from another run across the whole history.
    pub fn divergence_from(&self, other: &RunResult) -> f64 {
        let n = self.macro_trace.len().min(other.macro_trace.len());
        if n == 0 {
            return 0.0;
        }
        (0..n)
            .map(|t| macro_divergence(&self.macro_trace[t], &other.macro_trace[t]))
            .sum::<f64>()
            / n as f64
    }

    /// Mean absolute difference in total occupancy across the run.
    ///
    /// Where `divergence_from` asks whether the same things are in the same
    /// places, this asks only whether the universes hold the same *amount* of
    /// structure. Two chaotic universes decorrelate in position long before
    /// they disagree in aggregate, so this is the measure that survives chaos.
    pub fn live_delta_from(&self, other: &RunResult) -> f64 {
        let n = self.live_trace.len().min(other.live_trace.len());
        if n == 0 {
            return 0.0;
        }
        (0..n)
            .map(|t| (self.live_trace[t] - other.live_trace[t]).abs())
            .sum::<f64>()
            / n as f64
    }

    /// Divergence at the last shared tick: how different the two universes
    /// ended up, rather than how differently they travelled.
    pub fn final_divergence_from(&self, other: &RunResult) -> f64 {
        let n = self.macro_trace.len().min(other.macro_trace.len());
        if n == 0 {
            return 0.0;
        }
        macro_divergence(&self.macro_trace[n - 1], &other.macro_trace[n - 1])
    }
}

/// Run one universe start to finish.
///
/// The loop is the whole model in six lines: observe, which forces detail into
/// existence where something is looking; then apply the laws; then record what
/// an outside observer would have been able to see.
pub fn run(cfg: &Config, constraints: Constraints) -> RunResult {
    let res = Resolved::new(&constraints, &cfg.params);
    let geom = Geometry::new(
        cfg.world.width,
        cfg.world.height,
        res.subdivision,
        res.block_size,
    );

    let mut world = World::seed(geom, cfg.world.seed, cfg.world.init_density);
    let mut work = Work::default();
    let mut macro_trace = Vec::with_capacity(cfg.world.ticks as usize);
    let mut live_trace = Vec::with_capacity(cfg.world.ticks as usize);

    // Peak is sampled after the first observation, never at construction.
    // `World::seed` materialises every cell because it is easier to write
    // that way, but a resource-honest implementation would draw the initial
    // condition on demand like any other detail. Counting the construction
    // moment would charge lazy rendering for memory it does not hold while
    // the universe is running. Since `observe` is what sets the resolved
    // flags, and physics never changes them, one sample per tick is enough.
    let mut peak_live = 0usize;

    let started = Instant::now();
    for t in 0..cfg.world.ticks {
        let (observed, render_work) = observe(&world, &cfg.observer, t, cfg.world.seed, res.lazy);
        let (advanced, physics_work) = tick(&observed, &cfg.rules, &res);
        work.add(render_work);
        work.add(physics_work);
        peak_live = peak_live.max(observed.live_state_bytes());
        macro_trace.push(advanced.macro_field(cfg.report.macro_grid));
        live_trace.push(advanced.live_fraction());
        world = advanced;
    }
    let wall_ms = started.elapsed().as_secs_f64() * 1000.0;

    RunResult {
        label: constraints.label(),
        constraints,
        resolved: res,
        influence_speed: res.influence_speed(),
        wall_ms,
        work,
        peak_live_bytes: peak_live,
        allocated_bytes: world.allocated_bytes(),
        fine_cells: geom.cells(),
        final_live_fraction: world.live_fraction(),
        macro_trace,
        live_trace,
    }
}

/// One run measured against the unconstrained reference.
#[derive(Clone, Debug)]
pub struct Comparison {
    pub run: RunResult,
    /// Neighbour visits as a fraction of the reference's. Below 1 is cheaper.
    pub work_ratio: f64,
    /// Wall time as a fraction of the reference's.
    pub time_ratio: f64,
    /// Peak live bytes as a fraction of the reference's.
    pub memory_ratio: f64,
    pub mean_divergence: f64,
    pub final_divergence: f64,
    /// Difference in total occupancy: the chaos-resistant measure.
    pub live_delta: f64,
}

/// The full experiment: the unconstrained universe, each single limit, and all
/// limits together.
#[derive(Clone, Debug)]
pub struct Experiment {
    pub reference: RunResult,
    pub comparisons: Vec<Comparison>,
    /// Macro divergence between two unconstrained universes that differ only
    /// in seed. See [`run_all`].
    pub chaos_floor: f64,
    /// The same control, measured in total occupancy.
    pub chaos_floor_live: f64,
}

/// Run the reference universe, every single limit, all limits together, and a
/// control.
///
/// The control is the part that makes the rest mean anything. This world is
/// chaotic: perturb it however slightly and the macro field decorrelates,
/// so a large divergence number on its own says nothing about whether a limit
/// changed the universe. To calibrate, the reference is run a second time with
/// nothing altered but the seed. Those two universes are unquestionably the
/// same *kind* of universe, and the divergence between them is the floor that
/// chaos alone produces. A limit is only meaningfully visible if it diverges by
/// more than that.
pub fn run_all(cfg: &Config, mut on_run: impl FnMut(&str)) -> Experiment {
    on_run(&Constraints::ALL_OFF.label());
    let reference = run(cfg, Constraints::ALL_OFF);

    on_run("control (reference, different seed)");
    let control = {
        let mut c = cfg.clone();
        c.world.seed = cfg.world.seed.wrapping_add(1);
        run(&c, Constraints::ALL_OFF)
    };
    let chaos_floor = control.divergence_from(&reference);
    let chaos_floor_live = control.live_delta_from(&reference);

    let mut settings = Constraints::singles();
    settings.push(Constraints::ALL_ON);

    let comparisons = settings
        .into_iter()
        .map(|c| {
            on_run(&c.label());
            let r = run(cfg, c);
            Comparison {
                work_ratio: ratio(
                    r.work.neighbor_visits as f64,
                    reference.work.neighbor_visits as f64,
                ),
                time_ratio: ratio(r.wall_ms, reference.wall_ms),
                memory_ratio: ratio(r.peak_live_bytes as f64, reference.peak_live_bytes as f64),
                mean_divergence: r.divergence_from(&reference),
                final_divergence: r.final_divergence_from(&reference),
                live_delta: r.live_delta_from(&reference),
                run: r,
            }
        })
        .collect();

    Experiment {
        reference,
        comparisons,
        chaos_floor,
        chaos_floor_live,
    }
}

fn ratio(a: f64, b: f64) -> f64 {
    if b == 0.0 { f64::NAN } else { a / b }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::Degradation;
    use crate::config::{Config, ReportCfg, WorldCfg};
    use crate::constraints::Params;
    use crate::observer::Probe;
    use crate::physics::Rules;

    fn cfg() -> Config {
        Config {
            world: WorldCfg {
                width: 32,
                height: 32,
                ticks: 12,
                seed: 5,
                init_density: 0.3,
            },
            rules: Rules::default(),
            constraints: Constraints::ALL_ON,
            params: Params::default(),
            observer: Probe {
                x: 0,
                y: 0,
                width: 8,
                height: 8,
            },
            report: ReportCfg {
                macro_grid: 8,
                out_dir: "out".into(),
            },
            nesting: Degradation::default(),
            horizon: crate::pipe::Horizon::default(),
        }
    }

    #[test]
    fn a_run_is_reproducible() {
        let a = run(&cfg(), Constraints::ALL_ON);
        let b = run(&cfg(), Constraints::ALL_ON);
        assert_eq!(a.work, b.work);
        assert_eq!(a.macro_trace, b.macro_trace);
    }

    #[test]
    fn different_seeds_make_different_universes() {
        let mut c = cfg();
        let a = run(&c, Constraints::ALL_ON);
        c.world.seed = 6;
        let b = run(&c, Constraints::ALL_ON);
        assert_ne!(a.macro_trace, b.macro_trace);
    }

    #[test]
    fn every_constraint_lowers_the_work_counter() {
        // The core claim of Theory 1, checked directly on the reproducible
        // cost metric rather than on wall time.
        let c = cfg();
        let reference = run(&c, Constraints::ALL_OFF);
        for single in Constraints::singles() {
            let r = run(&c, single);
            assert!(
                r.work.neighbor_visits < reference.work.neighbor_visits,
                "{} did not reduce work: {} vs {}",
                single.label(),
                r.work.neighbor_visits,
                reference.work.neighbor_visits
            );
        }
    }

    #[test]
    fn all_constraints_together_are_the_cheapest_universe() {
        let c = cfg();
        let all_on = run(&c, Constraints::ALL_ON);
        for single in Constraints::singles() {
            assert!(all_on.work.neighbor_visits <= run(&c, single).work.neighbor_visits);
        }
    }

    #[test]
    fn the_reference_diverges_from_itself_by_nothing() {
        let r = run(&cfg(), Constraints::ALL_OFF);
        assert_eq!(r.divergence_from(&r), 0.0);
    }

    #[test]
    fn macro_traces_are_comparable_across_resolutions() {
        // Runs at different internal resolutions must still yield fields of
        // the same shape, or no comparison is possible at all.
        let c = cfg();
        let coarse = run(&c, Constraints::ALL_ON);
        let fine = run(&c, Constraints::ALL_OFF);
        assert_ne!(coarse.fine_cells, fine.fine_cells);
        assert_eq!(coarse.macro_trace[0].len(), fine.macro_trace[0].len());
        assert!(coarse.divergence_from(&fine).is_finite());
    }

    #[test]
    fn run_all_covers_every_setting() {
        let mut seen = Vec::new();
        let e = run_all(&cfg(), |l| seen.push(l.to_string()));
        assert_eq!(e.comparisons.len(), 5);
        assert_eq!(seen.len(), 7, "reference, control, then five variants");
        assert!(e.comparisons.iter().any(|c| c.run.label == "all_on"));
    }

    #[test]
    fn the_control_establishes_a_nonzero_chaos_floor() {
        // If re-seeding the reference produced no divergence, the divergence
        // column would be meaningless and every limit would look damning.
        let e = run_all(&cfg(), |_| {});
        assert!(
            e.chaos_floor > 0.0,
            "a chaotic world must decorrelate on reseed"
        );
        assert!(e.chaos_floor.is_finite());
    }

    #[test]
    fn run_all_is_reproducible_including_its_control() {
        let a = run_all(&cfg(), |_| {});
        let b = run_all(&cfg(), |_| {});
        assert_eq!(a.chaos_floor, b.chaos_floor);
        assert_eq!(a.chaos_floor_live, b.chaos_floor_live);
    }

    #[test]
    fn occupancy_divergence_survives_where_field_divergence_saturates() {
        // Two runs that decorrelate in position can still agree closely on
        // how much structure they hold; that is the point of `live_delta`.
        let e = run_all(&cfg(), |_| {});
        assert!(e.chaos_floor_live <= e.chaos_floor + 1e-9);
    }

    #[test]
    fn lazy_rendering_lowers_peak_memory() {
        let c = cfg();
        let mut lazy_only = Constraints::ALL_OFF;
        lazy_only.lazy_rendering = true;
        assert!(run(&c, lazy_only).peak_live_bytes < run(&c, Constraints::ALL_OFF).peak_live_bytes);
    }
}
