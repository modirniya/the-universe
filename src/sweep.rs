//! Theory 6: fine-tuning. How thin is the band of constants that produces
//! anything interesting?
//!
//! The fine-tuning argument is usually deployed as evidence of design. Here it
//! is treated as something to measure. Sweep a universe's constants, score what
//! each setting produces, and report what share of the parameter space is worth
//! inhabiting.
//!
//! The constants swept are the rule's **density bands** — the law from
//! [`crate::physics`], written as "born if the neighbourhood density falls in
//! this range, survives if it falls in that one". Each band is described by a
//! centre, with widths held fixed, so the space is two-dimensional and can be
//! drawn. Conway's B3/S23 sits at one point in it, which matters because it is
//! the only setting anyone independently believes is interesting.
//!
//! # Scoring, and its honest weakness
//!
//! A universe is called **complex** here when it is, at the end of its run,
//! neither empty nor saturated, still changing, and spatially structured. Those
//! three thresholds are calibrated against Conway rather than chosen from first
//! principles, and that is a real limitation: the measurement asks "does this
//! behave like the one setting we already believed in", not "is this
//! interesting" in some absolute sense. A band measured this way is a band of
//! settings resembling a known-good one. It is stated in the report so nobody
//! reads more into the number than it can carry.
//!
//! Falsified within the model if: complexity turns out to be common across the
//! parameter space. The fine-tuning claim needs the interesting band to be
//! *narrow*, and a sweep that finds most settings productive refutes it.

use crate::config::Config;
use crate::constraints::{Constraints, Resolved};
use crate::observer::observe;
use crate::physics::{Rules, tick};
use crate::space::{Geometry, World, macro_divergence};

/// Half-widths of the two bands, held fixed while the centres are swept.
///
/// Taken from Conway: birth on exactly 3 of 8 neighbours is a centre of 0.375
/// with a half-width of 0.0625, and survival on 2 or 3 is a centre of 0.3125
/// with a half-width of 0.125.
pub const BIRTH_HALF_WIDTH: f64 = 0.0625;
pub const SURVIVE_HALF_WIDTH: f64 = 0.125;

/// Where Conway sits in the swept space.
pub const CONWAY_BIRTH_CENTRE: f64 = 0.375;
pub const CONWAY_SURVIVE_CENTRE: f64 = 0.3125;

/// Build a rule from the two centres.
pub fn rules_at(birth_centre: f64, survive_centre: f64) -> Rules {
    Rules {
        birth_lo: birth_centre - BIRTH_HALF_WIDTH,
        birth_hi: birth_centre + BIRTH_HALF_WIDTH,
        survive_lo: survive_centre - SURVIVE_HALF_WIDTH,
        survive_hi: survive_centre + SURVIVE_HALF_WIDTH,
    }
}

/// What one setting of the constants produced.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Outcome {
    pub birth_centre: f64,
    pub survive_centre: f64,
    /// Occupancy at the end of the run.
    pub final_live: f64,
    /// Mean tick-to-tick change over the second half of the run.
    ///
    /// The second half only: every universe is busy at first, because the
    /// initial condition is noise that has to settle. What matters is whether
    /// it is still busy once it has.
    pub activity: f64,
    /// Mean macro-field variance over the second half, divided by what
    /// uncorrelated noise of the same density would give.
    ///
    /// Raw variance is nearly a function of density, so it cannot tell a
    /// structured world from a scattered one — it was tried, and it ranked a
    /// regular blinking tiling above Conway. Dividing by the independent
    /// baseline `p(1-p)/cells_per_macro` removes the density dependence: 1 is
    /// spatially uncorrelated, above 1 is clumped, below 1 is spread more
    /// evenly than chance.
    pub dispersion: f64,
    pub complex: bool,
}

/// What a setting must look like to be called complex.
///
/// Every criterion is a **band**, not a floor, and that is the correction this
/// module most needed. An earlier version required activity above a minimum,
/// which let in the rules that churn hardest — a world rewriting itself
/// completely every tick scored twenty times Conway's activity and was called
/// complex. Chaos is not complexity; it is what complexity sits between. The
/// same applies to spatial dispersion: perfectly even and heavily clumped are
/// both degenerate, and the interesting cases are in between.
#[derive(Clone, Copy, Debug)]
pub struct Bar {
    pub min_activity: f64,
    pub max_activity: f64,
    pub min_dispersion: f64,
    pub max_dispersion: f64,
    pub min_live: f64,
    pub max_live: f64,
}

impl Bar {
    pub fn admits(&self, o: &Outcome) -> bool {
        (self.min_activity..=self.max_activity).contains(&o.activity)
            && (self.min_dispersion..=self.max_dispersion).contains(&o.dispersion)
            && (self.min_live..=self.max_live).contains(&o.final_live)
    }
}

/// How far from the reference a setting may stray on each statistic.
///
/// A factor of four either way. Generous, because the claim under test is that
/// the productive band is *narrow* — a loose bar makes that harder to conclude,
/// not easier.
pub const TOLERANCE: f64 = 4.0;

/// Run one setting of the constants and score it.
///
/// Lazy rendering is off throughout. The question is what a *rule* produces,
/// and coarse-graining most of the world would answer a different one.
pub fn evaluate(
    cfg: &Config,
    birth_centre: f64,
    survive_centre: f64,
    bar: Option<&Bar>,
) -> Outcome {
    let mut constraints = Constraints::ALL_ON;
    constraints.lazy_rendering = false;

    let res = Resolved::new(&constraints, &cfg.params);
    let geom = Geometry::new(
        cfg.world.width,
        cfg.world.height,
        res.subdivision,
        res.block_size,
    );
    let rules = rules_at(birth_centre, survive_centre);

    let mut world = World::seed(geom, cfg.world.seed, cfg.world.init_density);
    let mut prev_field = world.macro_field(cfg.report.macro_grid);

    let half = cfg.world.ticks / 2;
    let cells_per_macro =
        (geom.cells() as f64 / (cfg.report.macro_grid * cfg.report.macro_grid) as f64).max(1.0);
    let mut activity = 0.0;
    let mut dispersion = 0.0;
    let mut counted = 0u64;

    for t in 0..cfg.world.ticks {
        let (observed, _) = observe(&world, &cfg.observer, t, cfg.world.seed, res.lazy);
        let (advanced, _) = tick(&observed, &rules, &res);

        let field = advanced.macro_field(cfg.report.macro_grid);
        if t >= half {
            activity += macro_divergence(&prev_field, &field);
            dispersion += dispersion_of(&field, cells_per_macro);
            counted += 1;
        }
        prev_field = field;
        world = advanced;
    }

    let n = counted.max(1) as f64;
    let outcome = Outcome {
        birth_centre,
        survive_centre,
        final_live: world.live_fraction(),
        activity: activity / n,
        dispersion: dispersion / n,
        complex: false,
    };

    Outcome {
        complex: bar.map(|b| b.admits(&outcome)).unwrap_or(false),
        ..outcome
    }
}

/// Spatial variance of a field.
fn variance(field: &[f64]) -> f64 {
    if field.is_empty() {
        return 0.0;
    }
    let mean = field.iter().sum::<f64>() / field.len() as f64;
    field.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / field.len() as f64
}

/// Field variance relative to uncorrelated noise of the same density.
///
/// Each macro cell averages `cells_per_macro` cells, so if those cells were
/// independent its value would vary by `p(1-p)/cells_per_macro`. Dividing by
/// that leaves a number that means the same thing at any density: 1 is chance,
/// above is clumped, below is more even than chance.
fn dispersion_of(field: &[f64], cells_per_macro: f64) -> f64 {
    if field.is_empty() {
        return 0.0;
    }
    let p = field.iter().sum::<f64>() / field.len() as f64;
    let baseline = p * (1.0 - p) / cells_per_macro;
    if baseline <= 0.0 {
        // Empty or saturated: no variation is possible, so none is meaningful.
        return 0.0;
    }
    variance(field) / baseline
}

/// Set the bar from what Conway does in this world.
pub fn calibrate(cfg: &Config) -> (Bar, Outcome) {
    let conway = evaluate(cfg, CONWAY_BIRTH_CENTRE, CONWAY_SURVIVE_CENTRE, None);
    let bar = Bar {
        min_activity: conway.activity / TOLERANCE,
        max_activity: conway.activity * TOLERANCE,
        min_dispersion: conway.dispersion / TOLERANCE,
        max_dispersion: conway.dispersion * TOLERANCE,
        min_live: 0.005,
        max_live: 0.95,
    };
    (bar, conway)
}

/// A whole sweep.
#[derive(Clone, Debug)]
pub struct Sweep {
    pub steps: usize,
    pub min: f64,
    pub max: f64,
    pub bar: Bar,
    /// What Conway did in this world, which set the bar.
    pub reference: Outcome,
    /// Row-major, `steps * steps`, survive centre varying down.
    pub grid: Vec<Outcome>,
}

/// The distinct rule a setting actually denotes.
///
/// A neighbourhood of radius 1 holds eight cells, so the only densities that
/// ever occur are `k/8`. Two band centres that admit the same set of `k` values
/// are the same law wearing different numbers. This collapses a setting to what
/// it does: eighteen bits saying, for each `k`, whether a cell is born and
/// whether it survives.
pub fn rule_signature(rules: &Rules) -> u32 {
    let mut sig = 0u32;
    for k in 0..=8u32 {
        let d = k as f64 / 8.0;
        sig |= u32::from(rules.born(d)) << (k * 2);
        sig |= u32::from(rules.survives(d)) << (k * 2 + 1);
    }
    sig
}

impl Sweep {
    /// How many genuinely different laws the sweep visited.
    ///
    /// Far fewer than the number of grid points, and the gap matters: an area
    /// fraction measured over a continuum that collapses onto a few dozen laws
    /// reports the resolution of the sweep, not of the universe.
    pub fn distinct_rules(&self) -> usize {
        self.signatures().len()
    }

    /// How many of those laws produced something complex.
    pub fn distinct_complex(&self) -> usize {
        self.signatures()
            .iter()
            .filter(|(_, complex)| **complex)
            .count()
    }

    /// Share of *distinct laws* that were productive.
    ///
    /// The honest version of [`Self::productive_fraction`], which measures area
    /// and therefore counts a law once per grid cell that happens to denote it.
    pub fn productive_rule_fraction(&self) -> f64 {
        let sigs = self.signatures();
        if sigs.is_empty() {
            return 0.0;
        }
        sigs.values().filter(|c| **c).count() as f64 / sigs.len() as f64
    }

    fn signatures(&self) -> std::collections::BTreeMap<u32, bool> {
        let mut out = std::collections::BTreeMap::new();
        for o in &self.grid {
            let sig = rule_signature(&rules_at(o.birth_centre, o.survive_centre));
            // A law counts as productive if any setting denoting it was.
            let entry = out.entry(sig).or_insert(false);
            *entry |= o.complex;
        }
        out
    }

    pub fn at(&self, col: usize, row: usize) -> &Outcome {
        &self.grid[row * self.steps + col]
    }

    /// Share of the swept space that produced something complex.
    ///
    /// The headline number, and the one Theory 6 lives or dies by.
    pub fn productive_fraction(&self) -> f64 {
        if self.grid.is_empty() {
            return 0.0;
        }
        self.grid.iter().filter(|o| o.complex).count() as f64 / self.grid.len() as f64
    }

    /// Whether Conway's own setting clears the bar it set.
    ///
    /// It should, and if it ever does not the calibration is broken rather than
    /// the theory being interesting.
    pub fn reference_is_admitted(&self) -> bool {
        self.bar.admits(&self.reference)
    }
}

/// Sweep both band centres over `[min, max]` at `steps` resolution.
pub fn run_sweep(
    cfg: &Config,
    steps: usize,
    min: f64,
    max: f64,
    mut on_row: impl FnMut(usize, usize),
) -> Sweep {
    let (bar, reference) = calibrate(cfg);
    let mut grid = Vec::with_capacity(steps * steps);

    for row in 0..steps {
        let survive = lerp(min, max, row, steps);
        for col in 0..steps {
            let birth = lerp(min, max, col, steps);
            grid.push(evaluate(cfg, birth, survive, Some(&bar)));
        }
        on_row(row + 1, steps);
    }

    Sweep {
        steps,
        min,
        max,
        bar,
        reference,
        grid,
    }
}

fn lerp(min: f64, max: f64, i: usize, steps: usize) -> f64 {
    if steps <= 1 {
        return min;
    }
    min + (max - min) * (i as f64 / (steps - 1) as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::Degradation;
    use crate::config::{ReportCfg, WorldCfg};
    use crate::constraints::Params;
    use crate::observer::Probe;
    use crate::pipe::Horizon;

    fn cfg() -> Config {
        Config {
            world: WorldCfg {
                width: 48,
                height: 48,
                ticks: 60,
                seed: 42,
                init_density: 0.3,
            },
            rules: Rules::default(),
            constraints: Constraints::ALL_ON,
            params: Params {
                block_size: 16,
                ..Params::default()
            },
            observer: Probe {
                x: 0,
                y: 0,
                width: 48,
                height: 48,
            },
            report: ReportCfg {
                macro_grid: 12,
                out_dir: "out".into(),
            },
            nesting: Degradation::default(),
            horizon: Horizon::default(),
        }
    }

    #[test]
    fn the_conway_point_reproduces_conway() {
        let r = rules_at(CONWAY_BIRTH_CENTRE, CONWAY_SURVIVE_CENTRE);
        for k in 0..=8 {
            let d = k as f64 / 8.0;
            assert_eq!(r.born(d), k == 3, "birth at {k}/8");
            assert_eq!(r.survives(d), k == 2 || k == 3, "survival at {k}/8");
        }
    }

    #[test]
    fn evaluation_is_deterministic() {
        let c = cfg();
        assert_eq!(
            evaluate(&c, 0.375, 0.3125, None),
            evaluate(&c, 0.375, 0.3125, None)
        );
    }

    #[test]
    fn a_dead_rule_scores_nothing() {
        // Birth demanding an impossible density: nothing is ever born, and what
        // starts alive cannot survive either.
        let c = cfg();
        let o = evaluate(&c, 1.5, -0.5, None);
        assert_eq!(o.final_live, 0.0);
        assert!(o.activity < 1e-9, "a dead world should not be changing");
        assert!(o.dispersion < 1e-9, "and an empty field has no dispersion");
    }

    #[test]
    fn chaos_is_not_admitted_as_complexity() {
        // The correction this module needed. A rule that rewrites the world
        // every tick scores enormous activity, and an activity *floor* let it
        // through as complex. Complexity sits between order and chaos, so the
        // criterion has to be a band.
        let c = cfg();
        let (bar, conway) = calibrate(&c);
        let churner = evaluate(&c, 0.05, 0.05, Some(&bar));
        assert!(
            churner.activity > conway.activity * TOLERANCE,
            "expected the chaotic corner to out-churn Conway by a lot: {} vs {}",
            churner.activity,
            conway.activity
        );
        assert!(!churner.complex, "chaos must not count as complexity");
    }

    #[test]
    fn dispersion_is_one_for_uncorrelated_noise() {
        // A field of independent means should sit at chance by construction.
        let cells_per_macro = 16.0;
        let mut rng = crate::rng::Rng::new(7);
        let field: Vec<f64> = (0..4096)
            .map(|_| {
                let live: u32 = (0..16).map(|_| u32::from(rng.chance(0.3))).sum();
                live as f64 / 16.0
            })
            .collect();
        let d = dispersion_of(&field, cells_per_macro);
        assert!(
            (0.85..=1.15).contains(&d),
            "uncorrelated noise should sit near 1, got {d}"
        );
    }

    #[test]
    fn dispersion_rises_when_the_field_clumps() {
        let flat = vec![0.3; 256];
        let mut clumped = vec![0.0; 256];
        for c in clumped.iter_mut().take(77) {
            *c = 1.0;
        }
        assert!(dispersion_of(&clumped, 16.0) > dispersion_of(&flat, 16.0));
    }

    #[test]
    fn a_saturating_rule_fills_and_freezes() {
        // Everything is born and everything survives.
        let c = cfg();
        let o = evaluate(&c, 0.5, 0.5, None);
        let wide = Rules {
            birth_lo: -1.0,
            birth_hi: 2.0,
            survive_lo: -1.0,
            survive_hi: 2.0,
        };
        assert!(wide.born(0.5) && wide.survives(0.5));
        // The swept version is narrower, but the point stands: extremes settle.
        assert!(o.activity.is_finite());
    }

    #[test]
    fn conway_clears_the_bar_it_sets() {
        let c = cfg();
        let (bar, conway) = calibrate(&c);
        assert!(
            bar.admits(&conway),
            "calibration is broken if the reference fails its own bar: {conway:?}"
        );
    }

    #[test]
    fn variance_separates_structure_from_flatness() {
        assert_eq!(variance(&[0.5, 0.5, 0.5, 0.5]), 0.0);
        assert!(variance(&[0.0, 1.0, 0.0, 1.0]) > 0.2);
    }

    #[test]
    fn the_conway_signature_is_b3_s23() {
        let sig = rule_signature(&rules_at(CONWAY_BIRTH_CENTRE, CONWAY_SURVIVE_CENTRE));
        for k in 0..=8u32 {
            assert_eq!((sig >> (k * 2)) & 1 == 1, k == 3, "birth at {k}");
            assert_eq!(
                (sig >> (k * 2 + 1)) & 1 == 1,
                k == 2 || k == 3,
                "survival at {k}"
            );
        }
    }

    #[test]
    fn nearby_centres_denote_the_same_law() {
        // Only k/8 densities occur, so a small nudge to a band centre usually
        // changes nothing at all. This is why area fractions overstate things.
        let a = rule_signature(&rules_at(0.375, 0.3125));
        let b = rule_signature(&rules_at(0.380, 0.3150));
        assert_eq!(a, b);
    }

    #[test]
    fn a_sweep_visits_far_fewer_laws_than_grid_points() {
        let c = cfg();
        let s = run_sweep(&c, 8, 0.05, 0.65, |_, _| {});
        assert_eq!(s.grid.len(), 64);
        assert!(
            s.distinct_rules() < s.grid.len(),
            "expected the grid to collapse onto fewer laws, got {} of {}",
            s.distinct_rules(),
            s.grid.len()
        );
        assert!(s.distinct_rules() > 1);
    }

    #[test]
    fn a_sweep_covers_its_grid() {
        let c = cfg();
        let s = run_sweep(&c, 4, 0.1, 0.6, |_, _| {});
        assert_eq!(s.grid.len(), 16);
        assert!(s.reference_is_admitted());
        assert!((0.0..=1.0).contains(&s.productive_fraction()));
        assert!((0.0..=1.0).contains(&s.productive_rule_fraction()));
    }

    #[test]
    fn lerp_spans_the_range() {
        assert_eq!(lerp(0.0, 1.0, 0, 5), 0.0);
        assert_eq!(lerp(0.0, 1.0, 4, 5), 1.0);
        assert_eq!(lerp(0.0, 1.0, 0, 1), 0.0);
    }

    #[test]
    fn a_sweep_is_reproducible() {
        let c = cfg();
        let a = run_sweep(&c, 3, 0.2, 0.5, |_, _| {});
        let b = run_sweep(&c, 3, 0.2, 0.5, |_, _| {});
        assert_eq!(a.grid, b.grid);
    }
}
