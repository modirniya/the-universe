//! Theory 6, the measure problem: a "productive fraction" is a number only
//! after someone says what counts as one law and how laws are weighted.
//!
//! The sweep in [`crate::sweep`] reports what share of its laws produce
//! something complex. The audit (`docs/audit.md` §5) noted that the share is
//! taken over the laws a particular grid happens to reach, weighted by how the
//! grid falls on them, under criteria tuned against one law, on one world
//! size, for one duration, from one initial density. Every one of those is a
//! choice of **prior**, and "fine-tuning is X%" means nothing until the prior
//! is named. This module names five and measures the share under each.
//!
//! # Priors
//!
//! - **grid** — the 21×21 grid of band centres at Conway's half-widths, each
//!   point weighted equally. What v0.5 reported first.
//! - **reached** — the distinct laws that grid reaches, each weighted equally.
//!   What v0.5 corrected itself to.
//! - **widened** — the distinct laws reached when both half-widths are swept
//!   too. What v0.9 added.
//! - **bands** — every rule expressible as two contiguous bands of neighbour
//!   counts: 46 birth bands × 46 survival bands = 2116 laws, each weighted
//!   equally. The whole family the band form can state.
//! - **life-like** — a uniform random sample of [`LIFELIKE_SAMPLE`] rules
//!   from the 2¹⁸ outer-totalistic rules on eight neighbours, the family the
//!   band form cannot state but the model can run. The same sample at every
//!   seed, so differences across seeds are differences of initial condition.
//!
//! Every law is scored by the three criteria of [`crate::sweep`], none of which
//! is privileged here.
//!
//! # Sensitivity
//!
//! The band family is also scored at other world sizes, run lengths and
//! initial densities, on a few seeds, to see how much the share moves with the
//! universe the laws are tried in.
//!
//! # What this can and cannot show
//!
//! It can show how much the productive share depends on the prior and on the
//! setting, which is the measure problem made concrete. It cannot say which
//! prior is right: the model has no principle that weights one law above
//! another, and that absence is the honest content of the fine-tuning
//! argument as it stands here. Nothing in this module bears on the constants
//! of our universe.
//!
//! Falsified within the model if: the productive share is the same under
//! every prior and setting (then the measure problem would be moot here), or
//! if some prior makes productive laws a majority (then "fine-tuning" would be
//! the wrong word under that prior, and the report says so).

use crate::config::Config;
use crate::physics::Rules;
use crate::rng::Rng;
use crate::stats;
use crate::sweep::{self, COMPRESSIBILITY, Criterion, GROWTH, HALF_WIDTHS, rule_signature};
use std::collections::BTreeMap;

/// Random outer-totalistic rules sampled for the life-like prior.
pub const LIFELIKE_SAMPLE: usize = 500;
const LIFELIKE_SEED: u64 = 0x4C49_4645_4C49_4B45;
/// Seeds the sensitivity settings are run on: the first this many of the
/// ensemble.
pub const SENSITIVITY_SEEDS: usize = 3;
/// Grid resolution per axis for the grid and reached priors.
pub const STEPS: usize = 21;
pub const SWEEP_MIN: f64 = 0.05;
pub const SWEEP_MAX: f64 = 0.65;

/// One law, scored.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scored {
    pub birth: u16,
    pub survive: u16,
    pub signature: u32,
    pub profile: sweep::Profile,
}

/// Every contiguous band of neighbour counts `0..=8`, including the empty one,
/// as a bit mask.
pub fn band_masks() -> Vec<u16> {
    let mut out = vec![0u16];
    for lo in 0..=8u16 {
        for hi in lo..=8 {
            let mut m = 0u16;
            for k in lo..=hi {
                m |= 1 << k;
            }
            out.push(m);
        }
    }
    out
}

/// The whole band family, as rule tables.
pub fn band_laws() -> Vec<Rules> {
    let masks = band_masks();
    let mut out = Vec::with_capacity(masks.len() * masks.len());
    for &b in &masks {
        for &s in &masks {
            out.push(Rules::table(b, s));
        }
    }
    out
}

/// The life-like sample: the same rules at every seed.
pub fn lifelike_laws() -> Vec<Rules> {
    let mut rng = Rng::new(LIFELIKE_SEED);
    (0..LIFELIKE_SAMPLE)
        .map(|_| {
            let bits = rng.next_u64();
            Rules::table((bits & 0x1FF) as u16, ((bits >> 9) & 0x1FF) as u16)
        })
        .collect()
}

fn score(cfg: &Config, rules: &Rules) -> Scored {
    let (birth, survive) = rules.table.unwrap_or_else(|| {
        // A band rule: read its table off its signature.
        let sig = rule_signature(rules);
        let mut b = 0u16;
        let mut s = 0u16;
        for k in 0..=8u32 {
            if (sig >> (k * 2)) & 1 == 1 {
                b |= 1 << k;
            }
            if (sig >> (k * 2 + 1)) & 1 == 1 {
                s |= 1 << k;
            }
        }
        (b, s)
    });
    Scored {
        birth,
        survive,
        signature: rule_signature(rules),
        profile: sweep::profile(cfg, rules),
    }
}

/// The five priors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prior {
    Grid,
    Reached,
    Widened,
    Bands,
    LifeLike,
}

pub const PRIORS: [Prior; 5] = [
    Prior::Grid,
    Prior::Reached,
    Prior::Widened,
    Prior::Bands,
    Prior::LifeLike,
];

impl Prior {
    pub fn label(&self) -> &'static str {
        match self {
            Prior::Grid => "grid points",
            Prior::Reached => "reached laws",
            Prior::Widened => "widened laws",
            Prior::Bands => "all band laws",
            Prior::LifeLike => "life-like sample",
        }
    }
}

/// Everything one seed measured.
#[derive(Clone, Debug)]
pub struct MeasureRun {
    pub seed: u64,
    /// Every band law, scored once.
    pub bands: Vec<Scored>,
    /// The life-like sample, scored once.
    pub lifelike: Vec<Scored>,
    /// Signature → how many grid points denote it, at Conway's half-widths.
    pub grid_multiplicity: BTreeMap<u32, usize>,
    /// Signatures reached when the half-widths are swept too.
    pub widened: Vec<u32>,
    pub criteria: Vec<Criterion>,
}

impl MeasureRun {
    /// Weighted share of laws a criterion admits under a prior, and the number
    /// of laws (or grid points) it rests on.
    pub fn fraction(&self, prior: Prior, criterion: usize) -> (f64, usize) {
        let c = &self.criteria[criterion];
        let by_sig: BTreeMap<u32, &Scored> = self.bands.iter().map(|s| (s.signature, s)).collect();
        match prior {
            Prior::Grid => {
                let mut total = 0usize;
                let mut hit = 0usize;
                for (sig, n) in &self.grid_multiplicity {
                    total += n;
                    if by_sig.get(sig).is_some_and(|s| c.admits(&s.profile)) {
                        hit += n;
                    }
                }
                (share(hit, total), total)
            }
            Prior::Reached => {
                let laws: Vec<&Scored> = self
                    .grid_multiplicity
                    .keys()
                    .filter_map(|sig| by_sig.get(sig).copied())
                    .collect();
                (
                    share(
                        laws.iter().filter(|s| c.admits(&s.profile)).count(),
                        laws.len(),
                    ),
                    laws.len(),
                )
            }
            Prior::Widened => {
                let laws: Vec<&Scored> = self
                    .widened
                    .iter()
                    .filter_map(|sig| by_sig.get(sig).copied())
                    .collect();
                (
                    share(
                        laws.iter().filter(|s| c.admits(&s.profile)).count(),
                        laws.len(),
                    ),
                    laws.len(),
                )
            }
            Prior::Bands => (
                share(
                    self.bands.iter().filter(|s| c.admits(&s.profile)).count(),
                    self.bands.len(),
                ),
                self.bands.len(),
            ),
            Prior::LifeLike => (
                share(
                    self.lifelike
                        .iter()
                        .filter(|s| c.admits(&s.profile))
                        .count(),
                    self.lifelike.len(),
                ),
                self.lifelike.len(),
            ),
        }
    }

    /// Count admitted under a prior, for a Wilson interval.
    pub fn count(&self, prior: Prior, criterion: usize) -> (usize, usize) {
        let (f, n) = self.fraction(prior, criterion);
        ((f * n as f64).round() as usize, n)
    }

    /// Lowest and highest share over every prior and criterion.
    pub fn range(&self) -> (f64, f64) {
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for p in PRIORS {
            for c in 0..self.criteria.len() {
                let (f, _) = self.fraction(p, c);
                if f.is_finite() {
                    lo = lo.min(f);
                    hi = hi.max(f);
                }
            }
        }
        (lo, hi)
    }
}

fn share(k: usize, n: usize) -> f64 {
    if n == 0 {
        f64::NAN
    } else {
        k as f64 / n as f64
    }
}

fn lerp(min: f64, max: f64, i: usize, steps: usize) -> f64 {
    if steps <= 1 {
        return min;
    }
    min + (max - min) * (i as f64 / (steps - 1) as f64)
}

/// Which signatures the grid denotes and how often, and which the widened
/// sweep reaches. No physics is run here.
pub fn reach(steps: usize) -> (BTreeMap<u32, usize>, Vec<u32>) {
    let mut grid: BTreeMap<u32, usize> = BTreeMap::new();
    for row in 0..steps {
        for col in 0..steps {
            let r = sweep::rules_at(
                lerp(SWEEP_MIN, SWEEP_MAX, col, steps),
                lerp(SWEEP_MIN, SWEEP_MAX, row, steps),
            );
            *grid.entry(rule_signature(&r)).or_insert(0) += 1;
        }
    }
    let mut widened: Vec<u32> = Vec::new();
    for &bh in HALF_WIDTHS {
        for &sh in HALF_WIDTHS {
            for row in 0..steps {
                for col in 0..steps {
                    let r = sweep::rules_with(
                        lerp(SWEEP_MIN, SWEEP_MAX, col, steps),
                        lerp(SWEEP_MIN, SWEEP_MAX, row, steps),
                        bh,
                        sh,
                    );
                    widened.push(rule_signature(&r));
                }
            }
        }
    }
    widened.sort_unstable();
    widened.dedup();
    (grid, widened)
}

/// Measure one seed.
pub fn run_measure(cfg: &Config) -> MeasureRun {
    let (bar, _) = sweep::calibrate(cfg);
    let (grid_multiplicity, widened) = reach(STEPS);
    MeasureRun {
        seed: cfg.world.seed,
        bands: band_laws().iter().map(|r| score(cfg, r)).collect(),
        lifelike: lifelike_laws().iter().map(|r| score(cfg, r)).collect(),
        grid_multiplicity,
        widened,
        criteria: vec![Criterion::Conway(bar), COMPRESSIBILITY, GROWTH],
    }
}

/// One alternative universe for the band family to be tried in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Setting {
    pub label: &'static str,
    pub edge: usize,
    pub ticks: u64,
    pub density: f64,
}

/// The baseline and six variations: half and 1.5× the edge, half and double
/// the duration, a sparser and a denser start.
pub const SETTINGS: [Setting; 7] = [
    Setting {
        label: "baseline",
        edge: 64,
        ticks: 80,
        density: 0.30,
    },
    Setting {
        label: "edge 32",
        edge: 32,
        ticks: 80,
        density: 0.30,
    },
    Setting {
        label: "edge 96",
        edge: 96,
        ticks: 80,
        density: 0.30,
    },
    Setting {
        label: "ticks 40",
        edge: 64,
        ticks: 40,
        density: 0.30,
    },
    Setting {
        label: "ticks 160",
        edge: 64,
        ticks: 160,
        density: 0.30,
    },
    Setting {
        label: "density 0.15",
        edge: 64,
        ticks: 80,
        density: 0.15,
    },
    Setting {
        label: "density 0.45",
        edge: 64,
        ticks: 80,
        density: 0.45,
    },
];

/// The band family scored in one setting, and the share under each criterion.
#[derive(Clone, Debug)]
pub struct SettingRun {
    pub seed: u64,
    pub setting: Setting,
    pub fractions: Vec<f64>,
    /// Conway's own profile in this setting: whether the reference still passes.
    pub conway_admitted: Vec<bool>,
}

/// Score the band family in one setting at the config's seed.
pub fn run_setting(cfg: &Config, setting: Setting) -> SettingRun {
    let mut c = cfg.clone();
    c.world.width = setting.edge;
    c.world.height = setting.edge;
    c.world.ticks = setting.ticks;
    c.world.init_density = setting.density;
    c.observer.x = 0;
    c.observer.y = 0;
    c.observer.width = setting.edge;
    c.observer.height = setting.edge;
    let (bar, _) = sweep::calibrate(&c);
    let criteria = [Criterion::Conway(bar), COMPRESSIBILITY, GROWTH];
    let conway = sweep::profile(&c, &Rules::default());
    let scored: Vec<sweep::Profile> = band_laws().iter().map(|r| sweep::profile(&c, r)).collect();
    SettingRun {
        seed: cfg.world.seed,
        setting,
        fractions: criteria
            .iter()
            .map(|cr| share(scored.iter().filter(|p| cr.admits(p)).count(), scored.len()))
            .collect(),
        conway_admitted: criteria.iter().map(|cr| cr.admits(&conway)).collect(),
    }
}

/// Wilson interval on a prior's count at one seed.
pub fn interval(run: &MeasureRun, prior: Prior, criterion: usize) -> (f64, f64) {
    let (k, n) = run.count(prior, criterion);
    stats::wilson(k, n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::Degradation;
    use crate::config::{ReportCfg, WorldCfg};
    use crate::constraints::{Constraints, Params};
    use crate::observer::Probe;
    use crate::pipe::Horizon;

    fn cfg() -> Config {
        Config {
            world: WorldCfg {
                width: 24,
                height: 24,
                ticks: 20,
                seed: 42,
                init_density: 0.3,
                seeds: 1,
                seed_stride: 1000,
            },
            rules: Rules::default(),
            constraints: Constraints::ALL_ON,
            params: Params {
                block_size: 8,
                ..Params::default()
            },
            observer: Probe {
                x: 0,
                y: 0,
                width: 24,
                height: 24,
            },
            report: ReportCfg {
                macro_grid: 6,
                out_dir: "out".into(),
            },
            nesting: Degradation::default(),
            horizon: Horizon::default(),
        }
    }

    #[test]
    fn the_band_family_has_forty_six_squared_laws_all_distinct() {
        let masks = band_masks();
        assert_eq!(masks.len(), 46);
        let laws = band_laws();
        assert_eq!(laws.len(), 46 * 46);
        let mut sigs: Vec<u32> = laws.iter().map(rule_signature).collect();
        sigs.sort_unstable();
        sigs.dedup();
        assert_eq!(sigs.len(), 2116, "every band law is a distinct rule");
        assert!(
            laws.iter()
                .any(|r| rule_signature(r) == rule_signature(&Rules::default()))
        );
    }

    #[test]
    fn the_lifelike_sample_is_fixed_and_mostly_outside_the_band_family() {
        let a = lifelike_laws();
        let b = lifelike_laws();
        assert_eq!(a.len(), LIFELIKE_SAMPLE);
        assert_eq!(a[0].table, b[0].table);
        let band_sigs: std::collections::BTreeSet<u32> =
            band_laws().iter().map(rule_signature).collect();
        let inside = a
            .iter()
            .filter(|r| band_sigs.contains(&rule_signature(r)))
            .count();
        assert!(
            inside * 4 < a.len(),
            "{inside} of {} are band laws",
            a.len()
        );
    }

    #[test]
    fn the_grid_reaches_few_laws_and_the_widened_sweep_more() {
        let (grid, widened) = reach(STEPS);
        assert_eq!(grid.values().sum::<usize>(), STEPS * STEPS);
        assert!(grid.len() < 60, "{}", grid.len());
        assert!(widened.len() > grid.len());
        let band_sigs: std::collections::BTreeSet<u32> =
            band_laws().iter().map(rule_signature).collect();
        assert!(grid.keys().all(|s| band_sigs.contains(s)));
        assert!(widened.iter().all(|s| band_sigs.contains(s)));
    }

    #[test]
    fn a_measure_run_gives_a_share_under_every_prior_and_criterion() {
        let run = run_measure(&cfg());
        assert_eq!(run.bands.len(), 2116);
        assert_eq!(run.lifelike.len(), LIFELIKE_SAMPLE);
        for p in PRIORS {
            for c in 0..3 {
                let (f, n) = run.fraction(p, c);
                assert!((0.0..=1.0).contains(&f), "{p:?} {c}: {f}");
                assert!(n > 0);
            }
        }
        let (lo, hi) = run.range();
        assert!(lo <= hi);
        let (_, n) = run.fraction(Prior::Grid, 0);
        assert_eq!(n, STEPS * STEPS);
    }

    #[test]
    fn a_setting_run_scores_the_band_family_in_another_universe() {
        let mut c = cfg();
        c.world.ticks = 10;
        let s = run_setting(
            &c,
            Setting {
                label: "tiny",
                edge: 16,
                ticks: 10,
                density: 0.3,
            },
        );
        assert_eq!(s.fractions.len(), 3);
        assert!(s.fractions.iter().all(|f| (0.0..=1.0).contains(f)));
        assert_eq!(s.conway_admitted.len(), 3);
    }
}
