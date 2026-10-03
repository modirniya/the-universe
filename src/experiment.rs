//! Running one universe, and running many: the primitives every experiment
//! shares.
//!
//! [`run`] is the loop that is the whole model: observe, which forces detail
//! into existence where something is looking; apply the laws; record what an
//! outside observer could have seen. It returns a [`RunResult`] holding the
//! macro trace, the cost counters and a [`Profile`] of macro-scale observables
//! averaged over the second half of the run.
//!
//! [`per_seed`] runs any experiment once per ensemble seed, the pinned seed
//! first and alone, the rest in parallel. [`Spread`] is the oldest summary in
//! the repository; [`crate::stats::Summary`] is the fuller one.
//!
//! The Theory 1 experiment itself — the factorial over the four limits, its
//! null models and its cost–fidelity analysis — lives in [`crate::limits`].

use crate::config::Config;
use crate::constraints::{Constraints, Resolved};
use crate::observables::{Accumulator, Profile};
use crate::observer::observe;
use crate::physics::{Work, tick};
use crate::space::{Geometry, World, macro_divergence};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
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
    /// Macro-scale observables averaged over the second half of the run.
    pub profile: Profile,
}

impl RunResult {
    /// Ticks the run lasted, as recorded.
    pub fn ticks(&self) -> usize {
        self.macro_trace.len()
    }

    /// The first tick of the second half: where the transient is treated as
    /// over and the window every late-time statistic uses begins.
    pub fn half(&self) -> usize {
        self.ticks() / 2
    }

    /// Mean divergence from another run across the whole history.
    ///
    /// Kept for comparison with the v0.9 figures. When the two runs share a
    /// seed this is dominated by the shared start; prefer
    /// [`Self::divergence_from_tick`] with [`Self::half`].
    pub fn divergence_from(&self, other: &RunResult) -> f64 {
        self.divergence_from_tick(other, 0)
    }

    /// Mean divergence from another run over ticks `from..`.
    pub fn divergence_from_tick(&self, other: &RunResult, from: usize) -> f64 {
        let n = self.macro_trace.len().min(other.macro_trace.len());
        if from >= n {
            return f64::NAN;
        }
        (from..n)
            .map(|t| macro_divergence(&self.macro_trace[t], &other.macro_trace[t]))
            .sum::<f64>()
            / (n - from) as f64
    }

    /// Divergence from another run at every shared tick.
    pub fn divergence_trace(&self, other: &RunResult) -> Vec<f64> {
        let n = self.macro_trace.len().min(other.macro_trace.len());
        (0..n)
            .map(|t| macro_divergence(&self.macro_trace[t], &other.macro_trace[t]))
            .collect()
    }

    /// Mean absolute difference in total occupancy across the run.
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

    /// Divergence at the last shared tick.
    pub fn final_divergence_from(&self, other: &RunResult) -> f64 {
        let n = self.macro_trace.len().min(other.macro_trace.len());
        if n == 0 {
            return 0.0;
        }
        macro_divergence(&self.macro_trace[n - 1], &other.macro_trace[n - 1])
    }

    /// Drop the per-tick traces, keeping everything computed from them.
    pub fn without_traces(mut self) -> RunResult {
        self.macro_trace = Vec::new();
        self.live_trace = Vec::new();
        self
    }
}

/// Run one universe start to finish, optionally from an altered initial
/// world.
///
/// `prepare` is applied to the seeded world before the first tick. The null
/// models use it to flip one cell; everything else passes the identity.
pub fn run_with(
    cfg: &Config,
    constraints: Constraints,
    prepare: impl FnOnce(&mut World),
) -> RunResult {
    let res = Resolved::new(&constraints, &cfg.params);
    let geom = Geometry::new(
        cfg.world.width,
        cfg.world.height,
        res.subdivision,
        res.block_size,
    );

    let mut world = World::seed(geom, cfg.world.seed, cfg.world.init_density);
    prepare(&mut world);
    world.sync_coarse_from_cells();

    let mut work = Work::default();
    let mut macro_trace = Vec::with_capacity(cfg.world.ticks as usize);
    let mut live_trace = Vec::with_capacity(cfg.world.ticks as usize);
    let mut acc = Accumulator::new();
    let half = (cfg.world.ticks / 2) as usize;

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
        let field = advanced.macro_field(cfg.report.macro_grid);
        if t as usize >= half {
            let prev = macro_trace.last().map(|v: &Vec<f64>| v.as_slice());
            acc.push(&advanced, &field, prev, cfg.report.macro_grid);
        }
        macro_trace.push(field);
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
        profile: acc.mean(),
    }
}

/// Run one universe start to finish.
pub fn run(cfg: &Config, constraints: Constraints) -> RunResult {
    run_with(cfg, constraints, |_| {})
}

// ---------------------------------------------------------------------------
// Ensembles: one seed is an example, not a finding
// ---------------------------------------------------------------------------

/// The seeds an ensemble runs, in order. The first is always the config's own
/// seed, so the pinned example is a member of every ensemble built from it.
pub fn ensemble_seeds(cfg: &Config) -> Vec<u64> {
    (0..cfg.world.seeds as u64)
        .map(|i| {
            cfg.world
                .seed
                .wrapping_add(i.wrapping_mul(cfg.world.seed_stride))
        })
        .collect()
}

/// Run `f` once per ensemble seed and return the results in seed order.
///
/// The pinned seed runs alone, before anything else starts, so the wall time
/// it reports is measured on a quiet machine. The rest run in parallel, one
/// thread per core. Every counter is unaffected by that, because each universe
/// is computed from its own seed alone and the results are placed by index,
/// never by completion order. Wall time from those parallel members is not
/// honest and must not be reported.
pub fn per_seed<T: Send>(cfg: &Config, f: impl Fn(&Config) -> T + Sync) -> Vec<(u64, T)> {
    let seeds = ensemble_seeds(cfg);
    let with = |seed: u64| {
        let mut c = cfg.clone();
        c.world.seed = seed;
        c
    };

    let mut out = Vec::with_capacity(seeds.len());
    out.push((seeds[0], f(&with(seeds[0]))));

    let rest = &seeds[1..];
    if rest.is_empty() {
        return out;
    }
    let jobs = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(rest.len());
    let next = AtomicUsize::new(0);
    let slots: Vec<Mutex<Option<T>>> = rest.iter().map(|_| Mutex::new(None)).collect();
    std::thread::scope(|scope| {
        for _ in 0..jobs {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= rest.len() {
                        break;
                    }
                    let r = f(&with(rest[i]));
                    *slots[i].lock().expect("a worker panicked") = Some(r);
                }
            });
        }
    });
    out.extend(rest.iter().zip(slots).map(|(seed, slot)| {
        let r = slot
            .into_inner()
            .expect("a worker panicked")
            .expect("every seed was run");
        (*seed, r)
    }));
    out
}

/// Mean, minimum and maximum of one quantity across an ensemble.
///
/// Non-finite values are left out and not counted, so `n` says how many
/// members the summary actually rests on. See [`crate::stats::Summary`] for
/// the version with a dispersion and an interval.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spread {
    pub mean: f64,
    pub min: f64,
    pub max: f64,
    pub n: usize,
}

impl Spread {
    pub fn of(values: impl IntoIterator<Item = f64>) -> Spread {
        let mut n = 0usize;
        let mut sum = 0.0;
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        for v in values.into_iter().filter(|v| v.is_finite()) {
            n += 1;
            sum += v;
            min = min.min(v);
            max = max.max(v);
        }
        if n == 0 {
            return Spread {
                mean: f64::NAN,
                min: f64::NAN,
                max: f64::NAN,
                n,
            };
        }
        Spread {
            // `+ 0.0` normalises the -0.0 an empty-ish float fold can produce.
            mean: sum / n as f64 + 0.0,
            min: min + 0.0,
            max: max + 0.0,
            n,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::Degradation;
    use crate::config::{Config, ReportCfg, WorldCfg};
    use crate::constraints::Params;
    use crate::observer::Probe;
    use crate::physics::Rules;

    pub(crate) fn cfg() -> Config {
        Config {
            world: WorldCfg {
                width: 32,
                height: 32,
                ticks: 12,
                seed: 5,
                init_density: 0.3,
                seeds: 1,
                seed_stride: 1000,
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
        assert_eq!(a.profile, b.profile);
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
        assert_eq!(r.divergence_from_tick(&r, r.half()), 0.0);
        assert!(r.divergence_trace(&r).iter().all(|d| *d == 0.0));
    }

    #[test]
    fn a_window_past_the_end_is_not_a_number() {
        let r = run(&cfg(), Constraints::ALL_OFF);
        assert!(r.divergence_from_tick(&r, r.ticks()).is_nan());
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
    fn the_profile_covers_the_second_half_only() {
        let c = cfg();
        let r = run(&c, Constraints::ALL_OFF);
        // Occupancy in the profile is the mean of the second-half live trace.
        let h = r.half();
        let want = r.live_trace[h..].iter().sum::<f64>() / (r.ticks() - h) as f64;
        assert!((r.profile.occupancy - want).abs() < 1e-12);
    }

    #[test]
    fn a_prepared_world_runs_differently_from_an_unprepared_one() {
        let c = cfg();
        let plain = run(&c, Constraints::ALL_OFF);
        let flipped = run_with(&c, Constraints::ALL_OFF, |w| w.cells[0] ^= 1);
        assert_ne!(plain.macro_trace, flipped.macro_trace);
        assert_eq!(
            plain.work, flipped.work,
            "a flipped cell costs nothing extra"
        );
    }

    #[test]
    fn ensemble_seeds_start_at_the_pinned_seed_and_step_by_the_stride() {
        let mut c = cfg();
        c.world.seeds = 4;
        c.world.seed_stride = 1000;
        assert_eq!(ensemble_seeds(&c), vec![5, 1005, 2005, 3005]);
    }

    #[test]
    fn per_seed_places_results_by_seed_not_by_completion() {
        // More seeds than cores, so several workers race for them. The order
        // that comes back must be the seed order regardless.
        let mut c = cfg();
        c.world.seeds = 17;
        let got = per_seed(&c, |c| c.world.seed * 3);
        let seeds = ensemble_seeds(&c);
        assert_eq!(got.len(), 17);
        for ((s, v), want) in got.iter().zip(&seeds) {
            assert_eq!(s, want);
            assert_eq!(*v, want * 3);
        }
    }

    #[test]
    fn a_spread_ignores_what_is_not_a_number_and_says_so() {
        let sp = Spread::of([1.0, f64::NAN, 3.0, 2.0]);
        assert_eq!(sp.n, 3);
        assert_eq!((sp.min, sp.max), (1.0, 3.0));
        assert!((sp.mean - 2.0).abs() < 1e-12);
        let empty = Spread::of([f64::NAN]);
        assert_eq!(empty.n, 0);
        assert!(empty.mean.is_nan());
    }

    #[test]
    fn lazy_rendering_lowers_peak_memory() {
        let c = cfg();
        let mut lazy_only = Constraints::ALL_OFF;
        lazy_only.lazy_rendering = true;
        assert!(run(&c, lazy_only).peak_live_bytes < run(&c, Constraints::ALL_OFF).peak_live_bytes);
    }
}
