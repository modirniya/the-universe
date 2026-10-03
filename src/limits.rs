//! Theory 1 as an experiment: the four limits, crossed, against four nulls.
//!
//! The question Theory 1 asks is whether a creator's limits make a universe
//! cheaper *without changing what it produces*. The first nine milestones
//! answered it with five runs per seed (each limit alone, then all together),
//! one divergence statistic, and one control. The audit (`docs/audit.md` §1)
//! found that design could not answer the question: the limits are not
//! independent interventions, the statistic was dominated by the shared start
//! and blind to a loss of variance, and the control was one number rather
//! than a distribution.
//!
//! This module is the redesign.
//!
//! # Design
//!
//! - **Factorial.** All sixteen settings of the four toggles run, so main
//!   effects and interactions can be read off rather than assumed.
//! - **Observables, not a number.** Fidelity is a [`Profile`] of seven
//!   macro-scale observables (see [`crate::observables`]), each standardised
//!   by how much it varies between unconstrained universes that differ only in
//!   seed. The summary distance `D` is the root mean square of those seven
//!   z-scores. Divergence of the macro field is still reported, but over the
//!   second half of the run and at the final tick, never as a whole-run mean.
//! - **Four null models**, each a reference universe altered in a way that
//!   is *not* a limit:
//!   1. `seed` — the reference at the next seed. What chaos alone produces
//!      between two universes of the same kind. Uncorrelated from tick 0.
//!   2. `perturb` — the reference with one base cell flipped at tick 0. The
//!      same-seed null: what a minimal perturbation does over time. This is
//!      the fair comparison for a limit that shares the reference's seed.
//!   3. `density` — the reference seeded at `init_density + 0.01`. About one
//!      cell in a hundred differs at the start: a parameter perturbation.
//!   4. `shuffle` — the reference's own macro field with its cells permuted.
//!      Same marginal, no arrangement: what divergence reads when spatial
//!      structure is unrelated.
//!
//!   Across an ensemble the references at every seed also give a null
//!   *distribution* of pairwise divergences (`n(n-1)/2` pairs), against which
//!   each limit's divergence is placed as a z-score and an empirical tail.
//! - **The closure for unobserved ground is a factor too.** Every lazy
//!   setting runs under the default [`CoarseRule`] in the factorial, and the
//!   `lazy` and `all_on` settings run again under the other two. A finding
//!   about lazy rendering that changes with the closure is a finding about the
//!   closure.
//! - **Cost against fidelity.** Each setting is placed on (work ratio, `D`),
//!   the Pareto set is listed, and "work saved per unit of `D`" is printed —
//!   not because that ratio is the right exchange rate, but so that cheapness
//!   and fidelity stop being read as one axis.
//!
//! # What is a consequence and what is a finding
//!
//! Work and memory ratios are arithmetic on the config: fewer cells, fewer
//! substeps, fewer neighbours, fewer resolved blocks. Their main effects are
//! exactly multiplicative and are printed as consequences. Everything about
//! fidelity had to be run.
//!
//! Falsified within the model if: no setting saves work at a distance
//! comparable to a null's, so that every optimization is as visible as a
//! change of law; or if the nulls themselves are not separable from one
//! another, so that the statistics cannot tell a minimal perturbation from a
//! reseed and nothing judged against them means anything.

use crate::config::Config;
use crate::constraints::{CoarseRule, Constraints};
use crate::experiment::{RunResult, per_seed, run, run_with};
use crate::observables::{NAMES, Profile};
use crate::physics::Work;
use crate::rng::Rng;
use crate::space::macro_divergence;
use crate::stats::{self, Summary};

/// One setting of the four limits (and one closure), measured against the
/// unconstrained reference of the same seed.
#[derive(Clone, Debug)]
pub struct Cell {
    pub label: String,
    pub constraints: Constraints,
    pub coarse_rule: CoarseRule,
    pub influence_speed: f64,
    pub work: Work,
    pub peak_live_bytes: usize,
    pub wall_ms: f64,
    /// Neighbour visits as a share of the reference's. Arithmetic on the config.
    pub work_ratio: f64,
    pub memory_ratio: f64,
    pub time_ratio: f64,
    /// Mean macro divergence from the reference over the second half.
    pub div_half: f64,
    /// Macro divergence from the reference at the final tick.
    pub div_final: f64,
    /// Whole-run mean, as v0.9 reported it. Kept for comparison only.
    pub div_whole: f64,
    pub final_live_fraction: f64,
    pub profile: Profile,
    /// Divergence from the reference at every tick.
    pub trace: Vec<f64>,
}

impl Cell {
    fn measure(
        label: String,
        coarse_rule: CoarseRule,
        r: &RunResult,
        reference: &RunResult,
    ) -> Cell {
        Cell {
            label,
            constraints: r.constraints,
            coarse_rule,
            influence_speed: r.influence_speed,
            work: r.work,
            peak_live_bytes: r.peak_live_bytes,
            wall_ms: r.wall_ms,
            work_ratio: ratio(
                r.work.neighbor_visits as f64,
                reference.work.neighbor_visits as f64,
            ),
            memory_ratio: ratio(r.peak_live_bytes as f64, reference.peak_live_bytes as f64),
            time_ratio: ratio(r.wall_ms, reference.wall_ms),
            div_half: r.divergence_from_tick(reference, reference.half()),
            div_final: r.final_divergence_from(reference),
            div_whole: r.divergence_from(reference),
            final_live_fraction: r.final_live_fraction,
            profile: r.profile,
            trace: r.divergence_trace(reference),
        }
    }

    /// Whether this is the unconstrained reference itself.
    pub fn is_reference(&self) -> bool {
        self.constraints == Constraints::ALL_OFF
    }

    pub fn without_trace(mut self) -> Cell {
        self.trace = Vec::new();
        self
    }
}

/// The four null models, by name and in the order they are run.
pub const NULLS: [&str; 4] = ["seed", "perturb", "density", "shuffle"];

/// How far `init_density` is moved for the `density` null.
pub const DENSITY_NUDGE: f64 = 0.01;

/// One seed's worth of the experiment.
#[derive(Clone, Debug)]
pub struct Factorial {
    pub seed: u64,
    /// All sixteen settings under the default closure, the reference first.
    pub cells: Vec<Cell>,
    /// `lazy` and `all_on` under each non-default closure.
    pub ablation: Vec<Cell>,
    /// The nulls, in [`NULLS`] order. Each is measured like a cell; the
    /// `shuffle` null has no run of its own and carries the reference's
    /// profile.
    pub nulls: Vec<Cell>,
    /// The reference's profile.
    pub reference_profile: Profile,
    /// The reference's second-half macro trace, kept so the ensemble can
    /// compute pairwise divergences between references at different seeds.
    pub reference_half_trace: Vec<Vec<f64>>,
}

impl Factorial {
    pub fn reference(&self) -> &Cell {
        &self.cells[0]
    }

    pub fn cell(&self, label: &str) -> Option<&Cell> {
        self.cells
            .iter()
            .chain(&self.ablation)
            .chain(&self.nulls)
            .find(|c| c.label == label)
    }

    pub fn null(&self, name: &str) -> Option<&Cell> {
        self.nulls.iter().find(|c| c.label == name)
    }

    /// Drop everything that is only needed for the pinned seed's own report.
    pub fn compact(mut self) -> Factorial {
        for c in self
            .cells
            .iter_mut()
            .chain(self.ablation.iter_mut())
            .chain(self.nulls.iter_mut())
        {
            c.trace = Vec::new();
        }
        self
    }
}

/// Every setting of the four toggles, the reference first and `all_on` last,
/// in a fixed order so that tables line up across seeds.
pub fn settings() -> Vec<Constraints> {
    let mut out = Vec::with_capacity(16);
    for bits in 0..16u8 {
        out.push(Constraints {
            discrete_space: bits & 1 != 0,
            discrete_time: bits & 2 != 0,
            speed_cap: bits & 4 != 0,
            lazy_rendering: bits & 8 != 0,
        });
    }
    out
}

fn with_rule(cfg: &Config, rule: CoarseRule) -> Config {
    let mut c = cfg.clone();
    c.params.coarse_rule = rule;
    c
}

/// Run one seed of the experiment.
pub fn run_factorial(cfg: &Config, mut on_run: impl FnMut(&str)) -> Factorial {
    on_run("all_off (reference)");
    let reference = run(cfg, Constraints::ALL_OFF);
    let half = reference.half();

    let mut cells = Vec::with_capacity(16);
    for c in settings() {
        let r = if c == Constraints::ALL_OFF {
            reference.clone()
        } else {
            on_run(&c.label());
            run(cfg, c)
        };
        cells.push(Cell::measure(
            c.label(),
            cfg.params.coarse_rule,
            &r,
            &reference,
        ));
    }

    let mut ablation = Vec::new();
    for rule in CoarseRule::ALL {
        if rule == cfg.params.coarse_rule {
            continue;
        }
        let alt = with_rule(cfg, rule);
        for c in [lazy_only(), Constraints::ALL_ON] {
            let label = format!("{}@{}", c.label(), rule.label());
            on_run(&label);
            let r = run(&alt, c);
            ablation.push(Cell::measure(label, rule, &r, &reference));
        }
    }

    let mut nulls = Vec::with_capacity(4);
    on_run("null: seed");
    {
        let mut c = cfg.clone();
        c.world.seed = cfg.world.seed.wrapping_add(1);
        let r = run(&c, Constraints::ALL_OFF);
        nulls.push(Cell::measure(
            "seed".into(),
            cfg.params.coarse_rule,
            &r,
            &reference,
        ));
    }
    on_run("null: perturb");
    {
        let r = run_with(cfg, Constraints::ALL_OFF, |w| {
            // Flip one base cell at the centre: every fine cell it covers.
            let s = w.geom.scale;
            let (bx, by) = (w.geom.w / s / 2, w.geom.h / s / 2);
            for dy in 0..s {
                for dx in 0..s {
                    let i = w.geom.idx(bx * s + dx, by * s + dy);
                    w.cells[i] ^= 1;
                }
            }
        });
        nulls.push(Cell::measure(
            "perturb".into(),
            cfg.params.coarse_rule,
            &r,
            &reference,
        ));
    }
    on_run("null: density");
    {
        let mut c = cfg.clone();
        c.world.init_density = (cfg.world.init_density + DENSITY_NUDGE).min(1.0);
        let r = run(&c, Constraints::ALL_OFF);
        nulls.push(Cell::measure(
            "density".into(),
            cfg.params.coarse_rule,
            &r,
            &reference,
        ));
    }
    on_run("null: shuffle");
    nulls.push(shuffle_null(
        &reference,
        cfg.world.seed,
        cfg.params.coarse_rule,
    ));

    Factorial {
        seed: cfg.world.seed,
        cells,
        ablation,
        nulls,
        reference_profile: reference.profile,
        reference_half_trace: reference.macro_trace[half..].to_vec(),
    }
}

fn lazy_only() -> Constraints {
    let mut c = Constraints::ALL_OFF;
    c.lazy_rendering = true;
    c
}

/// The reference against a permutation of its own macro field at every tick.
fn shuffle_null(reference: &RunResult, seed: u64, rule: CoarseRule) -> Cell {
    let trace: Vec<f64> = reference
        .macro_trace
        .iter()
        .enumerate()
        .map(|(t, field)| {
            let mut shuffled = field.clone();
            let mut rng = Rng::derive(seed, t as u64, 0x5348_5546, 0);
            for i in (1..shuffled.len()).rev() {
                let j = (rng.next_u64() % (i as u64 + 1)) as usize;
                shuffled.swap(i, j);
            }
            macro_divergence(field, &shuffled)
        })
        .collect();
    let half = reference.half();
    let n = trace.len();
    Cell {
        label: "shuffle".into(),
        constraints: Constraints::ALL_OFF,
        coarse_rule: rule,
        influence_speed: reference.influence_speed,
        work: reference.work,
        peak_live_bytes: reference.peak_live_bytes,
        wall_ms: reference.wall_ms,
        work_ratio: 1.0,
        memory_ratio: 1.0,
        time_ratio: 1.0,
        div_half: if n > half {
            trace[half..].iter().sum::<f64>() / (n - half) as f64
        } else {
            f64::NAN
        },
        div_final: trace.last().copied().unwrap_or(f64::NAN),
        div_whole: if n == 0 {
            f64::NAN
        } else {
            trace.iter().sum::<f64>() / n as f64
        },
        final_live_fraction: reference.final_live_fraction,
        profile: reference.profile,
        trace,
    }
}

fn ratio(a: f64, b: f64) -> f64 {
    if b == 0.0 { f64::NAN } else { a / b }
}

// ---------------------------------------------------------------------------
// The ensemble
// ---------------------------------------------------------------------------

/// The experiment at every seed, with the across-seed calibration that turns
/// each cell's observables into standardised distances.
#[derive(Clone, Debug)]
pub struct FactorialEnsemble {
    pub runs: Vec<(u64, Factorial)>,
    /// Second-half divergence between every pair of references at different
    /// seeds: the null distribution for "two universes of the same kind".
    pub pairwise_null: Vec<f64>,
    /// Standard deviation of each observable across the references.
    pub reference_sd: [f64; 7],
    pub reference_mean: [f64; 7],
}

/// Labels of the cells in the order every seed ran them.
pub fn cell_labels(ens: &FactorialEnsemble) -> Vec<String> {
    ens.runs
        .first()
        .map(|(_, f)| f.cells.iter().map(|c| c.label.clone()).collect())
        .unwrap_or_default()
}

/// Labels of the ablation cells.
pub fn ablation_labels(ens: &FactorialEnsemble) -> Vec<String> {
    ens.runs
        .first()
        .map(|(_, f)| f.ablation.iter().map(|c| c.label.clone()).collect())
        .unwrap_or_default()
}

impl FactorialEnsemble {
    pub fn len(&self) -> usize {
        self.runs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.runs.is_empty()
    }

    /// Each observable of a cell as a z-score against the reference of the
    /// same seed, in units of the across-seed spread of that observable.
    pub fn z_profile(&self, f: &Factorial, cell: &Cell) -> [f64; 7] {
        let a = cell.profile.as_array();
        let r = f.reference_profile.as_array();
        let mut z = [0.0; 7];
        for i in 0..7 {
            z[i] = if self.reference_sd[i] > 0.0 {
                (a[i] - r[i]) / self.reference_sd[i]
            } else {
                f64::NAN
            };
        }
        z
    }

    /// Root mean square of the finite z-scores: one number for how far a cell
    /// sits from its reference, in units of seed-to-seed variation.
    pub fn distance(&self, f: &Factorial, cell: &Cell) -> f64 {
        let z = self.z_profile(f, cell);
        let finite: Vec<f64> = z.iter().copied().filter(|v| v.is_finite()).collect();
        if finite.is_empty() {
            return f64::NAN;
        }
        (finite.iter().map(|v| v * v).sum::<f64>() / finite.len() as f64).sqrt()
    }

    /// One quantity of one labelled cell across seeds.
    pub fn values(&self, label: &str, f: impl Fn(&Factorial, &Cell) -> f64) -> Vec<f64> {
        self.runs
            .iter()
            .filter_map(|(_, fac)| fac.cell(label).map(|c| f(fac, c)))
            .collect()
    }

    pub fn summary(&self, label: &str, f: impl Fn(&Factorial, &Cell) -> f64) -> Summary {
        Summary::of(self.values(label, f))
    }

    /// Distance across seeds for one label.
    pub fn distances(&self, label: &str) -> Vec<f64> {
        self.values(label, |fac, c| self.distance(fac, c))
    }

    /// Mean z-score per observable for one label.
    pub fn mean_z(&self, label: &str) -> [f64; 7] {
        let mut out = [0.0; 7];
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = Summary::of(self.values(label, |fac, c| self.z_profile(fac, c)[i])).mean;
        }
        out
    }

    /// Paired ratio of a cell's second-half divergence to a null's, per seed.
    pub fn ratio_to_null(&self, label: &str, null: &str) -> Vec<f64> {
        self.runs
            .iter()
            .filter_map(|(_, fac)| {
                let c = fac.cell(label)?;
                let n = fac.null(null)?;
                Some(ratio(c.div_half, n.div_half))
            })
            .collect()
    }

    /// Seeds in which a cell's second-half divergence exceeded a null's.
    pub fn above_null(&self, label: &str, null: &str) -> (usize, usize) {
        let r = self.ratio_to_null(label, null);
        (r.iter().filter(|v| **v > 1.0).count(), r.len())
    }

    /// Where a cell's mean second-half divergence sits in the pairwise null:
    /// `(z, empirical tail)`.
    pub fn against_pairwise(&self, label: &str) -> (f64, f64) {
        let d = self.summary(label, |_, c| c.div_half).mean;
        (
            stats::z_score(d, &self.pairwise_null),
            stats::tail(d, &self.pairwise_null),
        )
    }

    /// Labels of the cells no other cell beats on both work and distance.
    pub fn pareto(&self) -> Vec<String> {
        let labels = cell_labels(self);
        let points: Vec<(String, f64, f64)> = labels
            .iter()
            .map(|l| {
                (
                    l.clone(),
                    self.summary(l, |_, c| c.work_ratio).mean,
                    Summary::of(self.distances(l)).mean,
                )
            })
            .collect();
        points
            .iter()
            .filter(|(l, w, d)| {
                !points
                    .iter()
                    .any(|(ol, ow, od)| ol != l && ow <= w && od <= d && (ow < w || od < d))
            })
            .map(|(l, _, _)| l.clone())
            .collect()
    }

    /// Main effect of one factor on a per-cell quantity: the mean over cells
    /// with it on minus the mean over cells with it off, averaged across seeds.
    pub fn main_effect(&self, factor: usize, f: impl Fn(&Factorial, &Cell) -> f64) -> Summary {
        Summary::of(self.runs.iter().map(|(_, fac)| {
            let (mut on, mut off) = (Vec::new(), Vec::new());
            for c in &fac.cells {
                let v = f(fac, c);
                if flag(&c.constraints, factor) {
                    on.push(v);
                } else {
                    off.push(v);
                }
            }
            mean(&on) - mean(&off)
        }))
    }

    /// Two-factor interaction: half the difference between the effect of `a`
    /// when `b` is on and when `b` is off.
    pub fn interaction(&self, a: usize, b: usize, f: impl Fn(&Factorial, &Cell) -> f64) -> Summary {
        Summary::of(self.runs.iter().map(|(_, fac)| {
            let effect_given = |b_on: bool| {
                let (mut on, mut off) = (Vec::new(), Vec::new());
                for c in fac.cells.iter().filter(|c| flag(&c.constraints, b) == b_on) {
                    let v = f(fac, c);
                    if flag(&c.constraints, a) {
                        on.push(v);
                    } else {
                        off.push(v);
                    }
                }
                mean(&on) - mean(&off)
            };
            (effect_given(true) - effect_given(false)) / 2.0
        }))
    }
}

/// Names of the four factors, in the bit order [`settings`] uses.
pub const FACTORS: [&str; 4] = ["space", "time", "speed", "lazy"];

fn flag(c: &Constraints, factor: usize) -> bool {
    match factor {
        0 => c.discrete_space,
        1 => c.discrete_time,
        2 => c.speed_cap,
        _ => c.lazy_rendering,
    }
}

fn mean(v: &[f64]) -> f64 {
    let finite: Vec<f64> = v.iter().copied().filter(|x| x.is_finite()).collect();
    if finite.is_empty() {
        f64::NAN
    } else {
        finite.iter().sum::<f64>() / finite.len() as f64
    }
}

/// Run the experiment at every ensemble seed and calibrate across them.
///
/// Members other than the pinned seed are compacted: their per-tick traces are
/// dropped once what was computed from them is kept.
pub fn run_ensemble(cfg: &Config, on_seed: impl Fn(u64) + Sync) -> FactorialEnsemble {
    let runs: Vec<(u64, Factorial)> = per_seed(cfg, |c| {
        on_seed(c.world.seed);
        let f = run_factorial(c, |_| {});
        if c.world.seed == cfg.world.seed {
            f
        } else {
            f.compact()
        }
    });
    calibrate(runs)
}

/// Build the ensemble's calibration from already-run members.
pub fn calibrate(runs: Vec<(u64, Factorial)>) -> FactorialEnsemble {
    let mut pairwise_null = Vec::new();
    for i in 0..runs.len() {
        for j in (i + 1)..runs.len() {
            let (a, b) = (
                &runs[i].1.reference_half_trace,
                &runs[j].1.reference_half_trace,
            );
            let n = a.len().min(b.len());
            if n == 0 {
                continue;
            }
            pairwise_null
                .push((0..n).map(|t| macro_divergence(&a[t], &b[t])).sum::<f64>() / n as f64);
        }
    }
    let mut reference_sd = [0.0; 7];
    let mut reference_mean = [0.0; 7];
    for i in 0..7 {
        let s = Summary::of(runs.iter().map(|(_, f)| f.reference_profile.as_array()[i]));
        reference_sd[i] = s.sd;
        reference_mean[i] = s.mean;
    }
    let runs = runs
        .into_iter()
        .map(|(s, mut f)| {
            f.reference_half_trace = Vec::new();
            (s, f)
        })
        .collect();
    FactorialEnsemble {
        runs,
        pairwise_null,
        reference_sd,
        reference_mean,
    }
}

/// The observable names, re-exported so reports need one import.
pub const OBSERVABLES: [&str; 7] = NAMES;

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
    fn sixteen_settings_reference_first_all_on_last_no_repeats() {
        let s = settings();
        assert_eq!(s.len(), 16);
        assert_eq!(s[0], Constraints::ALL_OFF);
        assert_eq!(s[15], Constraints::ALL_ON);
        let mut labels: Vec<String> = s.iter().map(|c| c.label()).collect();
        labels.sort();
        labels.dedup();
        assert_eq!(labels.len(), 16);
    }

    #[test]
    fn the_factorial_covers_every_setting_every_null_and_the_ablation() {
        let mut seen = Vec::new();
        let f = run_factorial(&cfg(), |l| seen.push(l.to_string()));
        assert_eq!(f.cells.len(), 16);
        assert!(f.reference().is_reference());
        assert_eq!(f.reference().div_half, 0.0);
        assert_eq!(f.nulls.len(), 4);
        for n in NULLS {
            assert!(f.null(n).is_some(), "{n}");
        }
        assert_eq!(
            f.ablation.len(),
            4,
            "lazy and all_on under two other closures"
        );
        assert!(
            f.ablation
                .iter()
                .all(|c| c.coarse_rule != cfg().params.coarse_rule)
        );
        // 15 constrained runs + 4 ablation + 4 nulls + reference = 24 announcements.
        assert_eq!(seen.len(), 24);
    }

    #[test]
    fn every_null_diverges_from_the_reference_but_the_reference_does_not() {
        let f = run_factorial(&cfg(), |_| {});
        for n in &f.nulls {
            assert!(n.div_half > 0.0, "{} should diverge", n.label);
            assert!(n.div_half.is_finite());
        }
        assert_eq!(f.reference().div_final, 0.0);
    }

    #[test]
    fn the_perturbation_null_starts_closer_than_the_seed_null() {
        // One flipped cell is a smaller change than a reseed, and at tick 0
        // the divergence must say so.
        let f = run_factorial(&cfg(), |_| {});
        let p = &f.null("perturb").unwrap().trace;
        let s = &f.null("seed").unwrap().trace;
        assert!(p[0] < s[0], "perturb {} vs seed {}", p[0], s[0]);
    }

    #[test]
    fn the_shuffle_null_costs_nothing_and_keeps_the_marginal() {
        let f = run_factorial(&cfg(), |_| {});
        let s = f.null("shuffle").unwrap();
        assert_eq!(s.work_ratio, 1.0);
        assert_eq!(s.profile, f.reference_profile);
        assert!(s.div_half > 0.0);
    }

    #[test]
    fn a_factorial_is_reproducible() {
        let a = run_factorial(&cfg(), |_| {});
        let b = run_factorial(&cfg(), |_| {});
        for (x, y) in a.cells.iter().zip(&b.cells) {
            assert_eq!(x.div_half, y.div_half, "{}", x.label);
            assert_eq!(x.profile, y.profile);
            assert_eq!(x.work, y.work);
        }
        for (x, y) in a.nulls.iter().zip(&b.nulls) {
            assert_eq!(x.div_half, y.div_half, "{}", x.label);
        }
    }

    #[test]
    fn an_ensemble_calibrates_across_its_members() {
        let mut c = cfg();
        c.world.seeds = 4;
        let ens = run_ensemble(&c, |_| {});
        assert_eq!(ens.len(), 4);
        assert_eq!(ens.pairwise_null.len(), 6);
        assert!(ens.pairwise_null.iter().all(|d| *d > 0.0));
        assert!(ens.reference_sd.iter().any(|s| *s > 0.0));
        // The reference is at zero distance from itself by construction.
        let d = ens.distances("all_off");
        assert!(d.iter().all(|v| *v == 0.0), "{d:?}");
        // Only the pinned seed keeps its traces.
        assert!(!ens.runs[0].1.cells[1].trace.is_empty());
        assert!(ens.runs[1].1.cells[1].trace.is_empty());
        assert!(
            ens.runs
                .iter()
                .all(|(_, f)| f.reference_half_trace.is_empty())
        );
    }

    #[test]
    fn the_pareto_set_contains_the_reference_and_nothing_dominated() {
        let mut c = cfg();
        c.world.seeds = 2;
        let ens = run_ensemble(&c, |_| {});
        let front = ens.pareto();
        assert!(front.contains(&"all_off".to_string()));
        for l in &front {
            let w = ens.summary(l, |_, c| c.work_ratio).mean;
            let d = Summary::of(ens.distances(l)).mean;
            for other in cell_labels(&ens) {
                if &other == l {
                    continue;
                }
                let ow = ens.summary(&other, |_, c| c.work_ratio).mean;
                let od = Summary::of(ens.distances(&other)).mean;
                assert!(
                    !(ow <= w && od <= d && (ow < w || od < d)),
                    "{other} dominates {l}"
                );
            }
        }
    }

    #[test]
    fn work_main_effects_are_the_config_arithmetic() {
        // log(work ratio) is exactly additive in the four factors, because
        // each one multiplies the visit count by a constant. A consequence,
        // pinned so the report can print it as one.
        let mut c = cfg();
        c.world.seeds = 1;
        let ens = run_ensemble(&c, |_| {});
        let log_work = |_: &Factorial, cell: &Cell| cell.work_ratio.ln();
        let p = Params::default();
        let want = [
            (1.0 / (p.subdivision * p.subdivision) as f64).ln(),
            (1.0 / p.substeps as f64).ln(),
            (8.0 / 48.0f64).ln(),
        ];
        for (i, w) in want.iter().enumerate() {
            let got = ens.main_effect(i, log_work).mean;
            assert!((got - w).abs() < 0.02, "{}: {got} vs {w}", FACTORS[i]);
        }
        for a in 0..3 {
            for b in (a + 1)..3 {
                assert!(ens.interaction(a, b, log_work).mean.abs() < 0.02);
            }
        }
    }
}
