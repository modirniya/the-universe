//! Detection: can an inhabitant tell, from the inside, that it is running
//! under limits?
//!
//! This is the milestone where the model has to argue against itself. Theory 1
//! established what the creator's limits are worth. The question here is
//! whether they are *findable* by something with no access to anything outside
//! its own universe — no config, no constraint flags, no view of the host.
//!
//! An [`Inhabitant`] is not an agent. It is a measuring apparatus with an
//! honest access restriction: it reads its own region of its own world, one
//! tick at a time, through the same [`World::sample`] every cell uses. It never
//! sees a `Constraints`, never learns the tick budget, and cannot look at a
//! second universe for comparison.
//!
//! # Detection as a hypothesis test
//!
//! The first version of this module called a limit "found" when two numbers
//! differed by more than a threshold chosen after seeing them (`docs/audit.md`
//! §2.3). It is now a test with a stated error rate:
//!
//! - **H0**: the inhabitant's universe does not have the limit. **H1**: it
//!   does. For each limit, H1 is the universe with every limit in force and H0
//!   the same universe with that one limit relaxed, so a test isolates one
//!   limit's fingerprint.
//! - A **statistic** is a number the inhabitant can compute from its own
//!   region: [`STATISTICS`].
//! - The ensemble's seeds are split. The first half **calibrates**: it fixes
//!   the direction of the test and a threshold at the most extreme H0 value
//!   seen, so the expected false-positive rate is about `1 / (n_cal + 1)`. The
//!   second half **evaluates**: the realised false-positive rate and the power
//!   are measured on seeds the rule never saw. Nothing is calibrated and
//!   evaluated on the same universe.
//! - A **negative control** runs the same machinery on two universes that
//!   both have every limit (H1 at the seed and H1 at the next seed). Any
//!   statistic that "detects" a difference there is detecting seeds, not
//!   limits, and its power elsewhere is not to be trusted.
//! - A **resample control** asks what a lazy-rendering detector is detecting.
//!   The lazy universe computes nothing in unobserved blocks. The control
//!   computes everything, but redraws the cells of those same blocks from
//!   their own density every tick — fully rendered, equally approximated. A
//!   rule calibrated on lazy rendering that also fires here is detecting the
//!   approximation, not the laziness.
//!
//! # What is a consequence and what is a finding
//!
//! Several statistics move for reasons that follow from the definitions, and
//! the report labels them so:
//!
//! - `influence_speed` reads `radius × substeps` because a birth needs live
//!   cells within one neighbourhood per substep. That the speed cap and
//!   discrete time are "found" by it is a consequence, and that they cannot be
//!   told apart by it is too: the inhabitant measures a product.
//! - `anisotropy` reads √2 on a Moore neighbourhood at any scale, because the
//!   corner of a square is √2 further than its edge. That the lattice's
//!   *shape* is visible and its *scale* is not are both consequences; the only
//!   thing that had to be run is that natural births reach the corner at all.
//! - `smoothness` under a passive gaze reads coarse ground as one repeated
//!   number by definition of coarse ground.
//! - `edge_excess` is new in v1.0 and is a **finding**, marked exploratory:
//!   under lazy rendering the cells at the edge of what an inhabitant renders
//!   have neighbours that are densities rather than cells, and the birth rate
//!   there differs from the interior. The statistic was designed from the
//!   mechanism after the audit and then evaluated on held-out seeds; a
//!   preregistered replication on a fresh seed range would make it
//!   confirmatory.
//!
//! The pixel scale is not measured at all any more. The v0.9 module reported a
//! `min_feature` of 1.0 as a measurement; it was a literal. The inhabitant's
//! unit is the cell, so its ruler cannot read its own length. That is a
//! definition and is recorded as one in the claims ledger.
//!
//! Falsified within the model if: a statistic's power is at the false-positive
//! rate for every limit (nothing is findable, and the module has nothing to
//! say), or the negative control shows power well above its false-positive
//! rate (the tests detect seeds, and every positive result is suspect).

use crate::config::Config;
use crate::constraints::{Constraints, Resolved};
use crate::observer::{Probe, observe_all};
use crate::physics::tick;
use crate::rng::Rng;
use crate::space::{Geometry, World};
use crate::stats;

/// Where an inhabitant lives. It can measure here and nowhere else.
#[derive(Clone, Copy, Debug)]
pub struct Inhabitant {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

impl Default for Inhabitant {
    fn default() -> Self {
        Inhabitant {
            x: 0,
            y: 0,
            width: 64,
            height: 64,
        }
    }
}

/// Whether an inhabitant's looking is itself an observation.
///
/// The framework defines a probe as *the event that forces full-resolution
/// computation of a region*. By that definition an inhabitant examining its
/// surroundings is a probe, and looking renders what it looks at — which is
/// [`Gaze::Rendering`], and the honest default.
///
/// [`Gaze::Passive`] is a reader that somehow sees without forcing computation.
/// It is not something the framework allows; it is included so that what the
/// act of looking conceals can be measured by comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gaze {
    Rendering,
    Passive,
}

impl Gaze {
    pub const ALL: [Gaze; 2] = [Gaze::Rendering, Gaze::Passive];

    pub fn label(&self) -> &'static str {
        match self {
            Gaze::Rendering => "looking renders",
            Gaze::Passive => "reads without rendering",
        }
    }
}

impl Inhabitant {
    /// The inhabitant considered as an observer of its own world.
    pub fn as_probe(&self) -> Probe {
        Probe {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
        }
    }

    /// Cells of its own world an inhabitant can reach, wrapped onto the
    /// torus, each with its Chebyshev distance to the window's edge.
    fn cells(&self, geom: &Geometry) -> Vec<(usize, usize, usize)> {
        let mut out = Vec::with_capacity(self.width * self.height);
        for row in 0..self.height {
            for col in 0..self.width {
                let margin = col
                    .min(row)
                    .min(self.width - 1 - col)
                    .min(self.height - 1 - row);
                out.push((
                    geom.wrap_x((self.x + col) as isize),
                    geom.wrap_y((self.y + row) as isize),
                    margin,
                ));
            }
        }
        out
    }
}

/// What an inhabitant managed to measure.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Evidence {
    /// Greatest distance, in cells, between a newly live cell and the nearest
    /// cell that was live the tick before.
    pub influence_speed: f64,
    /// Share of sampled cells whose whole 3x3 neighbourhood reads identical and
    /// non-empty. Coarse ground is perfectly smooth wherever it is not empty;
    /// real cells almost never are. Emptiness is excluded because an empty
    /// coarse block is genuinely indistinguishable from empty ground.
    pub smoothness: f64,
    /// Greatest straight-line distance from a birth to its nearest ancestor,
    /// among births whose nearest ancestor lay along a row or column.
    pub axis_reach: f64,
    /// The same, among births whose nearest ancestor lay on a diagonal.
    pub diagonal_reach: f64,
    /// Births per resolved cell in the outermost ring of the window.
    pub edge_birth_rate: f64,
    /// Births per resolved cell at least [`INTERIOR_MARGIN`] cells from the edge.
    pub interior_birth_rate: f64,
    /// How many ticks the inhabitant had anything to measure at all.
    pub samples: u64,
}

impl Evidence {
    /// How much further influence reaches along a diagonal than along an axis.
    /// 1 is what an isotropic continuum would show. `NaN` when either direction
    /// was never seen.
    pub fn anisotropy(&self) -> f64 {
        if self.axis_reach == 0.0 || self.diagonal_reach == 0.0 {
            f64::NAN
        } else {
            self.diagonal_reach / self.axis_reach
        }
    }

    /// Birth rate at the edge of the window relative to its interior. 1 means
    /// the edge behaves like the interior; `NaN` when nothing was born inside.
    pub fn edge_excess(&self) -> f64 {
        if self.interior_birth_rate > 0.0 {
            self.edge_birth_rate / self.interior_birth_rate
        } else {
            f64::NAN
        }
    }
}

/// How far from the window's edge a cell must be to count as interior.
pub const INTERIOR_MARGIN: usize = 4;

/// How far the search for a causal ancestor goes before giving up.
const MAX_SEARCH: usize = 8;

/// The statistics an inhabitant can compute, by name.
pub type Statistic = (&'static str, fn(&Evidence) -> f64);

pub const STATISTICS: [Statistic; 4] = [
    ("influence_speed", |e| e.influence_speed),
    ("anisotropy", |e| e.anisotropy()),
    ("smoothness", |e| e.smoothness),
    ("edge_excess", |e| e.edge_excess()),
];

/// One universe an inhabitant is placed in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Condition {
    pub constraints: Constraints,
    pub gaze: Gaze,
    /// The resample control: lazy rendering off, but the blocks lazy
    /// rendering would leave unobserved are redrawn from their own density
    /// every tick. Fully rendered, equally approximated.
    pub resample: bool,
}

const RESAMPLE_TAG: u64 = 0x5245_5341_4D50;

/// Run a universe and let an inhabitant measure it.
pub fn investigate(cfg: &Config, cond: Condition, who: &Inhabitant) -> Evidence {
    let mut constraints = cond.constraints;
    if cond.resample {
        constraints.lazy_rendering = false;
    }
    let res = Resolved::new(&constraints, &cfg.params);
    let geom = Geometry::new(
        cfg.world.width,
        cfg.world.height,
        res.subdivision,
        res.block_size,
    );
    let mut world = World::seed(geom, cfg.world.seed, cfg.world.init_density);
    let home = who.cells(&geom);

    // Under the framework's own definition of a probe, an inhabitant that
    // examines its surroundings is one.
    let probes: Vec<Probe> = match cond.gaze {
        Gaze::Rendering => vec![cfg.observer, who.as_probe()],
        Gaze::Passive => vec![cfg.observer],
    };
    // Which blocks lazy rendering would leave coarse: what the resample
    // control redraws.
    let would_be_coarse: Vec<bool> = {
        let mut acc = vec![true; geom.blocks()];
        for p in &probes {
            for (b, seen) in p.observed_blocks(&geom).into_iter().enumerate() {
                if seen {
                    acc[b] = false;
                }
            }
        }
        acc
    };

    let mut max_reach = 0usize;
    let mut axis = 0.0f64;
    let mut diagonal = 0.0f64;
    let mut smooth = 0u64;
    let mut sampled = 0u64;
    let mut ticks_measured = 0u64;
    let (mut edge_births, mut edge_cells) = (0u64, 0u64);
    let (mut interior_births, mut interior_cells) = (0u64, 0u64);

    for t in 0..cfg.world.ticks {
        let (observed, _) = observe_all(&world, &probes, t, cfg.world.seed, res.lazy);
        let (mut advanced, _) = tick(&observed, &cfg.rules, &res);
        if cond.resample {
            resample_blocks(&mut advanced, &would_be_coarse, t, cfg.world.seed);
        }

        for (x, y, _) in &home {
            if is_smooth(&advanced, *x, *y) {
                smooth += 1;
            }
            sampled += 1;
        }

        if let Some(r) = measure_reach(&observed, &advanced, &home) {
            max_reach = max_reach.max(r);
            ticks_measured += 1;
        }
        let (a, d) = measure_directions(&observed, &advanced, &home);
        axis = axis.max(a);
        diagonal = diagonal.max(d);

        let (eb, ec, ib, ic) = measure_edge(&observed, &advanced, &home);
        edge_births += eb;
        edge_cells += ec;
        interior_births += ib;
        interior_cells += ic;

        world = advanced;
    }

    let rate = |b: u64, c: u64| {
        if c == 0 {
            f64::NAN
        } else {
            b as f64 / c as f64
        }
    };
    Evidence {
        influence_speed: max_reach as f64,
        smoothness: if sampled == 0 {
            0.0
        } else {
            smooth as f64 / sampled as f64
        },
        axis_reach: axis,
        diagonal_reach: diagonal,
        edge_birth_rate: rate(edge_births, edge_cells),
        interior_birth_rate: rate(interior_births, interior_cells),
        samples: ticks_measured,
    }
}

/// Redraw every cell of the given blocks from the block's own density.
fn resample_blocks(w: &mut World, which: &[bool], tick: u64, seed: u64) {
    for (b, &redraw) in which.iter().enumerate().take(w.geom.blocks()) {
        if !redraw || !w.resolved[b] {
            continue;
        }
        let d = w.coarse[b];
        let (x0, y0, x1, y1) = w.geom.block_bounds(b);
        let mut rng = Rng::derive(seed, b as u64, tick, RESAMPLE_TAG);
        for y in y0..y1 {
            for x in x0..x1 {
                let i = w.geom.idx(x, y);
                w.cells[i] = u8::from(rng.chance(d));
            }
        }
        w.coarse[b] = w.density_from_cells(b);
    }
}

/// Births and resolved cells in the outermost ring and in the interior.
fn measure_edge(
    before: &World,
    after: &World,
    home: &[(usize, usize, usize)],
) -> (u64, u64, u64, u64) {
    let geom = &after.geom;
    let (mut eb, mut ec, mut ib, mut ic) = (0u64, 0u64, 0u64, 0u64);
    for (x, y, margin) in home {
        let b = geom.block_of(*x, *y);
        if !before.resolved[b] || !after.resolved[b] {
            continue;
        }
        let idx = geom.idx(*x, *y);
        let born = u64::from(before.cells[idx] == 0 && after.cells[idx] == 1);
        if *margin == 0 {
            ec += 1;
            eb += born;
        } else if *margin >= INTERIOR_MARGIN {
            ic += 1;
            ib += born;
        }
    }
    (eb, ec, ib, ic)
}

/// Greatest axis and diagonal reach among this tick's births.
fn measure_directions(before: &World, after: &World, home: &[(usize, usize, usize)]) -> (f64, f64) {
    let geom = &after.geom;
    let (mut axis, mut diagonal) = (0.0f64, 0.0f64);
    for (x, y, _) in home {
        let b = geom.block_of(*x, *y);
        if !before.resolved[b] || !after.resolved[b] {
            continue;
        }
        let idx = geom.idx(*x, *y);
        if !(before.cells[idx] == 0 && after.cells[idx] == 1) {
            continue;
        }
        if let Some((dx, dy)) = nearest_live_euclidean(before, *x, *y) {
            let d = ((dx * dx + dy * dy) as f64).sqrt();
            if dx == 0 || dy == 0 {
                axis = axis.max(d);
            } else if dx.abs() == dy.abs() {
                diagonal = diagonal.max(d);
            }
        }
    }
    (axis, diagonal)
}

/// Displacement to the nearest previously live cell by straight-line distance.
fn nearest_live_euclidean(before: &World, x: usize, y: usize) -> Option<(isize, isize)> {
    let geom = &before.geom;
    let mut best: Option<(isize, isize, isize)> = None;
    for r in 1..=MAX_SEARCH as isize {
        if let Some((_, _, d2)) = best
            && r * r > d2
        {
            break;
        }
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let nx = geom.wrap_x(x as isize + dx);
                let ny = geom.wrap_y(y as isize + dy);
                if !before.resolved[geom.block_of(nx, ny)] {
                    return None;
                }
                if before.cells[geom.idx(nx, ny)] == 1 {
                    let d2 = dx * dx + dy * dy;
                    if best.is_none_or(|(_, _, b)| d2 < b) {
                        best = Some((dx, dy, d2));
                    }
                }
            }
        }
    }
    best.map(|(dx, dy, _)| (dx, dy))
}

/// Whether a cell's 3x3 neighbourhood reads as one repeated non-empty number.
fn is_smooth(w: &World, x: usize, y: usize) -> bool {
    let centre = w.sample(x, y);
    if centre == 0.0 {
        return false;
    }
    for dy in -1..=1isize {
        for dx in -1..=1isize {
            let nx = w.geom.wrap_x(x as isize + dx);
            let ny = w.geom.wrap_y(y as isize + dy);
            if w.sample(nx, ny) != centre {
                return false;
            }
        }
    }
    true
}

/// Greatest distance from a newly live cell to the nearest previously live one.
fn measure_reach(before: &World, after: &World, home: &[(usize, usize, usize)]) -> Option<usize> {
    let geom = &after.geom;
    let mut best: Option<usize> = None;
    for (x, y, _) in home {
        let b = geom.block_of(*x, *y);
        if !before.resolved[b] || !after.resolved[b] {
            continue;
        }
        let idx = geom.idx(*x, *y);
        if !(before.cells[idx] == 0 && after.cells[idx] == 1) {
            continue;
        }
        if let Some(d) = nearest_live_before(before, *x, *y) {
            best = Some(best.map_or(d, |m: usize| m.max(d)));
        }
    }
    best
}

/// Chebyshev distance to the nearest previously live cell, or `None` if the
/// search meets coarse ground first: an incomplete measurement is dropped
/// rather than guessed.
fn nearest_live_before(before: &World, x: usize, y: usize) -> Option<usize> {
    let geom = &before.geom;
    for r in 1..=MAX_SEARCH {
        let ri = r as isize;
        let mut incomplete = false;
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                if dx.abs() != ri && dy.abs() != ri {
                    continue;
                }
                let nx = geom.wrap_x(x as isize + dx);
                let ny = geom.wrap_y(y as isize + dy);
                if !before.resolved[geom.block_of(nx, ny)] {
                    incomplete = true;
                    continue;
                }
                if before.cells[geom.idx(nx, ny)] == 1 {
                    return Some(r);
                }
            }
        }
        if incomplete {
            return None;
        }
    }
    None
}

// ---------------------------------------------------------------------------
// The survey: every condition, at one seed
// ---------------------------------------------------------------------------

/// The four limits, by name and in the order they are tested.
pub const LIMITS: [&str; 4] = [
    "discrete_space",
    "discrete_time",
    "speed_cap",
    "lazy_rendering",
];

/// H0 for one limit: every limit in force except that one.
pub fn without(limit: &str) -> Constraints {
    let mut c = Constraints::ALL_ON;
    match limit {
        "discrete_space" => c.discrete_space = false,
        "discrete_time" => c.discrete_time = false,
        "speed_cap" => c.speed_cap = false,
        "lazy_rendering" => c.lazy_rendering = false,
        other => panic!("unknown limit {other}"),
    }
    c
}

/// Everything one seed's inhabitant measured, in every condition.
#[derive(Clone, Debug)]
pub struct SeedEvidence {
    pub seed: u64,
    /// `(gaze, label, evidence)`. Labels: `all_on`, `without:<limit>`,
    /// `resample`, `twin` (all limits, next seed).
    pub rows: Vec<(Gaze, String, Evidence)>,
}

impl SeedEvidence {
    pub fn get(&self, gaze: Gaze, label: &str) -> Option<&Evidence> {
        self.rows
            .iter()
            .find(|(g, l, _)| *g == gaze && l == label)
            .map(|(_, _, e)| e)
    }
}

/// Measure every condition at one seed.
pub fn measure_seed(cfg: &Config, who: &Inhabitant) -> SeedEvidence {
    let mut rows = Vec::new();
    let mut twin_cfg = cfg.clone();
    twin_cfg.world.seed = cfg.world.seed.wrapping_add(1);
    for gaze in Gaze::ALL {
        let cond = |constraints, resample| Condition {
            constraints,
            gaze,
            resample,
        };
        rows.push((
            gaze,
            "all_on".to_string(),
            investigate(cfg, cond(Constraints::ALL_ON, false), who),
        ));
        for limit in LIMITS {
            rows.push((
                gaze,
                format!("without:{limit}"),
                investigate(cfg, cond(without(limit), false), who),
            ));
        }
        rows.push((
            gaze,
            "resample".to_string(),
            investigate(cfg, cond(without("lazy_rendering"), true), who),
        ));
        rows.push((
            gaze,
            "twin".to_string(),
            investigate(&twin_cfg, cond(Constraints::ALL_ON, false), who),
        ));
    }
    SeedEvidence {
        seed: cfg.world.seed,
        rows,
    }
}

// ---------------------------------------------------------------------------
// The tests
// ---------------------------------------------------------------------------

/// Which way a statistic moves under H1, fixed on the calibration seeds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// H1 reads higher; flag values above the threshold.
    Above,
    /// H1 reads lower; flag values below it.
    Below,
    /// Calibration saw no difference, or nothing finite; flag nothing.
    None,
}

/// A decision rule: a direction and a threshold.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rule {
    pub direction: Direction,
    pub threshold: f64,
    /// Calibration seeds the rule rests on.
    pub n_cal: usize,
}

impl Rule {
    /// Whether a value is flagged as H1.
    pub fn flags(&self, v: f64) -> bool {
        if !v.is_finite() {
            return false;
        }
        match self.direction {
            Direction::Above => v > self.threshold,
            Direction::Below => v < self.threshold,
            Direction::None => false,
        }
    }

    /// Expected false-positive rate of a threshold set at the most extreme of
    /// `n_cal` H0 values: `1 / (n_cal + 1)`.
    pub fn expected_fpr(&self) -> f64 {
        if self.n_cal == 0 {
            f64::NAN
        } else {
            1.0 / (self.n_cal + 1) as f64
        }
    }
}

/// Fix a rule from paired calibration values `(h0, h1)`.
///
/// The direction is the sign of the mean difference; the threshold is the
/// most extreme finite H0 value in that direction, so that no calibration H0
/// is flagged. If no finite pair exists or the means are equal, the rule flags
/// nothing.
pub fn calibrate(pairs: &[(f64, f64)]) -> Rule {
    let finite: Vec<(f64, f64)> = pairs
        .iter()
        .copied()
        .filter(|(a, b)| a.is_finite() && b.is_finite())
        .collect();
    let n_cal = finite.len();
    if n_cal == 0 {
        return Rule {
            direction: Direction::None,
            threshold: f64::NAN,
            n_cal,
        };
    }
    let mean_h0 = finite.iter().map(|p| p.0).sum::<f64>() / n_cal as f64;
    let mean_h1 = finite.iter().map(|p| p.1).sum::<f64>() / n_cal as f64;
    let h0 = finite.iter().map(|p| p.0);
    if mean_h1 > mean_h0 {
        Rule {
            direction: Direction::Above,
            threshold: h0.fold(f64::NEG_INFINITY, f64::max),
            n_cal,
        }
    } else if mean_h1 < mean_h0 {
        Rule {
            direction: Direction::Below,
            threshold: h0.fold(f64::INFINITY, f64::min),
            n_cal,
        }
    } else {
        Rule {
            direction: Direction::None,
            threshold: mean_h0,
            n_cal,
        }
    }
}

/// What a rule did on seeds it never saw.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Outcome {
    /// Evaluation pairs with a finite value on the side being counted.
    pub n_h0: usize,
    pub n_h1: usize,
    pub false_positives: usize,
    pub true_positives: usize,
}

impl Outcome {
    pub fn fpr(&self) -> f64 {
        share(self.false_positives, self.n_h0)
    }

    /// Power: the share of H1 universes flagged.
    pub fn tpr(&self) -> f64 {
        share(self.true_positives, self.n_h1)
    }

    /// `(TPR + (1 - FPR)) / 2`: the accuracy of a blinded classifier shown
    /// equal numbers of each, against a chance level of one half.
    pub fn balanced_accuracy(&self) -> f64 {
        (self.tpr() + 1.0 - self.fpr()) / 2.0
    }

    pub fn fpr_interval(&self) -> (f64, f64) {
        stats::wilson(self.false_positives, self.n_h0)
    }

    pub fn tpr_interval(&self) -> (f64, f64) {
        stats::wilson(self.true_positives, self.n_h1)
    }
}

fn share(k: usize, n: usize) -> f64 {
    if n == 0 {
        f64::NAN
    } else {
        k as f64 / n as f64
    }
}

/// Apply a rule to paired evaluation values `(h0, h1)`.
pub fn evaluate(rule: &Rule, pairs: &[(f64, f64)]) -> Outcome {
    let mut o = Outcome {
        n_h0: 0,
        n_h1: 0,
        false_positives: 0,
        true_positives: 0,
    };
    for (h0, h1) in pairs {
        if h0.is_finite() {
            o.n_h0 += 1;
            o.false_positives += usize::from(rule.flags(*h0));
        }
        if h1.is_finite() {
            o.n_h1 += 1;
            o.true_positives += usize::from(rule.flags(*h1));
        }
    }
    o
}

/// Why a result is not a discovery, where it is not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standing {
    /// Follows from the definitions; the run confirms arithmetic or geometry.
    Consequence(&'static str),
    /// Could have come out otherwise; the design was fixed before the data.
    Finding,
    /// Could have come out otherwise, but the statistic was designed after an
    /// earlier look at the model. Confirmatory only after replication.
    Exploratory(&'static str),
}

impl Standing {
    pub fn label(&self) -> &'static str {
        match self {
            Standing::Consequence(_) => "consequence",
            Standing::Finding => "finding",
            Standing::Exploratory(_) => "exploratory",
        }
    }

    pub fn note(&self) -> &'static str {
        match self {
            Standing::Consequence(n) | Standing::Exploratory(n) => n,
            Standing::Finding => "",
        }
    }
}

/// How a (limit, statistic, gaze) combination stands before any data.
pub fn standing(limit: &str, statistic: &str, gaze: Gaze) -> Standing {
    match (limit, statistic) {
        ("speed_cap" | "discrete_time", "influence_speed") => Standing::Consequence(
            "reach is radius x substeps by construction; the two limits move the same product",
        ),
        ("discrete_space", "anisotropy") => Standing::Consequence(
            "a square neighbourhood reaches its corner sqrt(2) further at every scale",
        ),
        ("lazy_rendering", "smoothness") if gaze == Gaze::Passive => {
            Standing::Consequence("coarse ground is one repeated number by definition")
        }
        (_, "edge_excess") => Standing::Exploratory(
            "statistic designed in v1.0 from the mechanism, after the audit; evaluated on held-out seeds",
        ),
        _ => Standing::Finding,
    }
}

/// One hypothesis test: a limit, a statistic, a gaze, the rule calibration
/// fixed, and what it did on the evaluation seeds.
#[derive(Clone, Debug)]
pub struct Test {
    pub limit: &'static str,
    pub statistic: &'static str,
    pub gaze: Gaze,
    pub rule: Rule,
    pub outcome: Outcome,
    pub standing: Standing,
    /// Mean of the statistic under H0 and H1 over every seed, for the report.
    pub mean_h0: f64,
    pub mean_h1: f64,
}

/// The lazy-rendering rule applied to the resample control.
#[derive(Clone, Copy, Debug)]
pub struct ResampleCheck {
    pub statistic: &'static str,
    pub gaze: Gaze,
    /// Evaluation seeds on which the lazy rule flagged the lazy universe.
    pub lazy_flagged: usize,
    /// Evaluation seeds on which the same rule flagged the resample control.
    pub resample_flagged: usize,
    pub n: usize,
}

/// The lattice's shape, read at the pinned seed, against the geometry of a
/// continuum. A consequence: see the module docs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Isotropy {
    pub lattice: f64,
    pub finer: f64,
    pub continuum: f64,
}

/// The whole detection experiment across an ensemble.
#[derive(Clone, Debug)]
pub struct Survey {
    pub evidence: Vec<SeedEvidence>,
    pub n_cal: usize,
    pub tests: Vec<Test>,
    /// The same tests run with H1 replaced by an all-limits universe at the
    /// next seed. Power here is power to detect a seed.
    pub negative_control: Vec<Test>,
    pub resample: Vec<ResampleCheck>,
    pub isotropy: Option<Isotropy>,
}

impl Survey {
    /// Calibrate on the first `n_cal` seeds and evaluate on the rest.
    pub fn build(evidence: Vec<SeedEvidence>, n_cal: usize) -> Survey {
        let n_cal = n_cal.min(evidence.len());
        let (cal, eval) = evidence.split_at(n_cal);
        let pairs =
            |rows: &[SeedEvidence], gaze: Gaze, h0: &str, h1: &str, f: fn(&Evidence) -> f64| {
                rows.iter()
                    .filter_map(|se| Some((f(se.get(gaze, h0)?), f(se.get(gaze, h1)?))))
                    .collect::<Vec<(f64, f64)>>()
            };
        let mean_of = |rows: &[SeedEvidence], gaze: Gaze, label: &str, f: fn(&Evidence) -> f64| {
            stats::Summary::of(rows.iter().filter_map(|se| se.get(gaze, label).map(f))).mean
        };

        let mut tests = Vec::new();
        let mut negative_control = Vec::new();
        let mut resample = Vec::new();
        for gaze in Gaze::ALL {
            for limit in LIMITS {
                let h0 = format!("without:{limit}");
                for (name, f) in STATISTICS {
                    let rule = calibrate(&pairs(cal, gaze, &h0, "all_on", f));
                    let outcome = evaluate(&rule, &pairs(eval, gaze, &h0, "all_on", f));
                    tests.push(Test {
                        limit,
                        statistic: name,
                        gaze,
                        rule,
                        outcome,
                        standing: standing(limit, name, gaze),
                        mean_h0: mean_of(&evidence, gaze, &h0, f),
                        mean_h1: mean_of(&evidence, gaze, "all_on", f),
                    });
                    if limit == "lazy_rendering" {
                        let eval_pairs = pairs(eval, gaze, &h0, "all_on", f);
                        let control_pairs = pairs(eval, gaze, &h0, "resample", f);
                        resample.push(ResampleCheck {
                            statistic: name,
                            gaze,
                            lazy_flagged: eval_pairs.iter().filter(|p| rule.flags(p.1)).count(),
                            resample_flagged: control_pairs
                                .iter()
                                .filter(|p| rule.flags(p.1))
                                .count(),
                            n: eval_pairs.len(),
                        });
                    }
                }
            }
            // Negative control: all limits at this seed against all limits at
            // the next. There is no limit to find.
            for (name, f) in STATISTICS {
                let rule = calibrate(&pairs(cal, gaze, "all_on", "twin", f));
                let outcome = evaluate(&rule, &pairs(eval, gaze, "all_on", "twin", f));
                negative_control.push(Test {
                    limit: "none (two seeds)",
                    statistic: name,
                    gaze,
                    rule,
                    outcome,
                    standing: Standing::Finding,
                    mean_h0: mean_of(&evidence, gaze, "all_on", f),
                    mean_h1: mean_of(&evidence, gaze, "twin", f),
                });
            }
        }

        let isotropy = evidence.first().and_then(|se| {
            Some(Isotropy {
                lattice: se.get(Gaze::Rendering, "all_on")?.anisotropy(),
                finer: se
                    .get(Gaze::Rendering, "without:discrete_space")?
                    .anisotropy(),
                continuum: 1.0,
            })
        });

        Survey {
            evidence,
            n_cal,
            tests,
            negative_control,
            resample,
            isotropy,
        }
    }

    pub fn n_eval(&self) -> usize {
        self.evidence.len().saturating_sub(self.n_cal)
    }

    pub fn test(&self, limit: &str, statistic: &str, gaze: Gaze) -> Option<&Test> {
        self.tests
            .iter()
            .find(|t| t.limit == limit && t.statistic == statistic && t.gaze == gaze)
    }

    /// The best-powered statistic for a limit under a gaze, with its outcome.
    pub fn best(&self, limit: &str, gaze: Gaze) -> Option<&Test> {
        self.tests
            .iter()
            .filter(|t| t.limit == limit && t.gaze == gaze && t.outcome.tpr().is_finite())
            .max_by(|a, b| {
                a.outcome
                    .balanced_accuracy()
                    .partial_cmp(&b.outcome.balanced_accuracy())
                    .unwrap()
            })
    }

    /// Highest power any statistic reached on the negative control: how much
    /// "detection" the machinery produces when there is nothing to detect.
    pub fn control_ceiling(&self) -> f64 {
        self.negative_control
            .iter()
            .map(|t| t.outcome.tpr())
            .filter(|v| v.is_finite())
            .fold(0.0, f64::max)
    }
}

/// Measure every seed of the config's ensemble and build the survey, with the
/// first half of the seeds calibrating and the second half evaluating.
pub fn survey(cfg: &Config, who: &Inhabitant, on_seed: impl Fn(u64) + Sync) -> Survey {
    let evidence: Vec<SeedEvidence> = crate::experiment::per_seed(cfg, |c| {
        on_seed(c.world.seed);
        measure_seed(c, who)
    })
    .into_iter()
    .map(|(_, e)| e)
    .collect();
    let n_cal = evidence.len() / 2;
    Survey::build(evidence, n_cal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::Degradation;
    use crate::config::{ReportCfg, WorldCfg};
    use crate::constraints::Params;
    use crate::physics::Rules;
    use crate::pipe::Horizon;

    fn cfg() -> Config {
        Config {
            world: WorldCfg {
                width: 64,
                height: 64,
                ticks: 40,
                seed: 42,
                init_density: 0.3,
                seeds: 1,
                seed_stride: 1000,
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
                width: 32,
                height: 32,
            },
            report: ReportCfg {
                macro_grid: 8,
                out_dir: "out".into(),
            },
            nesting: Degradation::default(),
            horizon: Horizon::default(),
        }
    }

    /// An inhabitant living inside the observed region, where there are cells.
    fn resident() -> Inhabitant {
        Inhabitant {
            x: 4,
            y: 4,
            width: 24,
            height: 24,
        }
    }

    /// One spanning both observed and coarse ground.
    fn frontier() -> Inhabitant {
        Inhabitant {
            x: 16,
            y: 16,
            width: 32,
            height: 32,
        }
    }

    fn rendering(c: Constraints) -> Condition {
        Condition {
            constraints: c,
            gaze: Gaze::Rendering,
            resample: false,
        }
    }

    fn passive(c: Constraints) -> Condition {
        Condition {
            constraints: c,
            gaze: Gaze::Passive,
            resample: false,
        }
    }

    #[test]
    fn measurement_is_deterministic() {
        let c = cfg();
        assert_eq!(
            investigate(&c, rendering(Constraints::ALL_ON), &resident()),
            investigate(&c, rendering(Constraints::ALL_ON), &resident())
        );
    }

    #[test]
    fn an_inhabitant_can_measure_the_speed_of_influence() {
        let c = cfg();
        let e = investigate(&c, rendering(Constraints::ALL_ON), &resident());
        assert!(e.samples > 0, "nothing was measurable at all");
        assert_eq!(e.influence_speed, 1.0);
    }

    #[test]
    fn relaxing_the_speed_cap_is_visible_from_inside() {
        let c = cfg();
        let capped = investigate(&c, rendering(Constraints::ALL_ON), &resident());
        let uncapped = investigate(&c, rendering(without("speed_cap")), &resident());
        assert!(uncapped.influence_speed > capped.influence_speed);
    }

    #[test]
    fn observed_speed_never_exceeds_its_bound() {
        let mut c = cfg();
        c.params.substeps = 3;
        c.params.uncapped_radius = 3;
        for (k, bound) in [
            (Constraints::ALL_ON, 1.0),
            (without("discrete_time"), 3.0),
            (without("speed_cap"), 3.0),
        ] {
            let e = investigate(&c, rendering(k), &resident());
            assert!(
                e.influence_speed <= bound,
                "{} > {bound}",
                e.influence_speed
            );
        }
    }

    #[test]
    fn coarse_ground_reads_as_smooth_to_a_passive_reader_only() {
        let c = cfg();
        let lazy_passive = investigate(&c, passive(Constraints::ALL_ON), &frontier());
        let full_passive = investigate(&c, passive(without("lazy_rendering")), &frontier());
        let lazy_rendering = investigate(&c, rendering(Constraints::ALL_ON), &frontier());
        assert!(lazy_passive.smoothness > full_passive.smoothness);
        assert!(full_passive.smoothness < 0.05);
        assert!(
            lazy_rendering.smoothness < 0.05,
            "looking renders what it looks at"
        );
    }

    #[test]
    fn the_resample_control_is_fully_rendered() {
        let c = cfg();
        let r = investigate(
            &c,
            Condition {
                constraints: Constraints::ALL_ON,
                gaze: Gaze::Passive,
                resample: true,
            },
            &frontier(),
        );
        assert!(
            r.smoothness < 0.05,
            "resampled ground is cells, not a density"
        );
        assert!(r.edge_birth_rate.is_finite());
    }

    #[test]
    fn edge_births_are_counted_only_where_cells_exist() {
        let before = World::seed(Geometry::new(16, 16, 1, 16), 1, 0.0);
        let mut after = before.clone();
        let who = Inhabitant {
            x: 2,
            y: 2,
            width: 12,
            height: 12,
        };
        // One birth on the window's edge, one deep inside.
        for (x, y) in [(2, 5), (7, 7)] {
            let i = after.geom.idx(x, y);
            after.cells[i] = 1;
        }
        let home = who.cells(&before.geom);
        let (eb, ec, ib, ic) = measure_edge(&before, &after, &home);
        assert_eq!((eb, ib), (1, 1));
        assert_eq!(ec, 44, "the ring of a 12x12 window");
        assert_eq!(ic, 16, "cells at least 4 from the edge of a 12x12 window");
        // Coarse ground contributes nothing.
        let mut coarse = before.clone();
        coarse.resolved[0] = false;
        assert_eq!(measure_edge(&coarse, &after, &home), (0, 0, 0, 0));
    }

    #[test]
    fn calibration_fixes_direction_and_an_extreme_threshold() {
        let r = calibrate(&[(1.0, 3.0), (1.2, 2.9), (0.9, 3.1)]);
        assert_eq!(r.direction, Direction::Above);
        assert_eq!(r.threshold, 1.2);
        assert_eq!(r.n_cal, 3);
        assert!(r.flags(1.3) && !r.flags(1.2) && !r.flags(f64::NAN));
        assert!((r.expected_fpr() - 0.25).abs() < 1e-12);

        let r = calibrate(&[(3.0, 1.0), (3.0, 1.0)]);
        assert_eq!(r.direction, Direction::Below);
        assert_eq!(r.threshold, 3.0);
        assert!(r.flags(1.0) && !r.flags(3.0));

        let none = calibrate(&[(1.0, 1.0), (2.0, 2.0)]);
        assert_eq!(none.direction, Direction::None);
        assert!(!none.flags(5.0));
        assert_eq!(calibrate(&[]).n_cal, 0);
        assert_eq!(calibrate(&[(f64::NAN, 1.0)]).direction, Direction::None);
    }

    #[test]
    fn evaluation_counts_each_side_and_skips_what_is_not_a_number() {
        let rule = calibrate(&[(1.0, 3.0)]);
        let o = evaluate(&rule, &[(0.5, 2.0), (1.5, 2.0), (f64::NAN, 0.5)]);
        assert_eq!((o.n_h0, o.n_h1), (2, 3));
        assert_eq!((o.false_positives, o.true_positives), (1, 2));
        assert!((o.fpr() - 0.5).abs() < 1e-12);
        assert!((o.tpr() - 2.0 / 3.0).abs() < 1e-12);
        assert!((o.balanced_accuracy() - (2.0 / 3.0 + 0.5) / 2.0).abs() < 1e-12);
    }

    #[test]
    fn the_survey_tests_every_limit_statistic_and_gaze() {
        let mut c = cfg();
        c.world.width = 48;
        c.world.height = 48;
        c.world.ticks = 20;
        c.world.seeds = 4;
        let s = survey(&c, &frontier(), |_| {});
        assert_eq!(s.evidence.len(), 4);
        assert_eq!(s.n_cal, 2);
        assert_eq!(s.n_eval(), 2);
        assert_eq!(s.tests.len(), 2 * 4 * 4);
        assert_eq!(s.negative_control.len(), 2 * 4);
        assert_eq!(s.resample.len(), 2 * 4);
        assert!(s.isotropy.is_some());
        let t = s
            .test("speed_cap", "influence_speed", Gaze::Rendering)
            .unwrap();
        assert_eq!(t.standing.label(), "consequence");
        assert!(matches!(
            s.test("lazy_rendering", "edge_excess", Gaze::Rendering)
                .unwrap()
                .standing,
            Standing::Exploratory(_)
        ));
    }

    #[test]
    fn the_speed_cap_is_found_with_full_power() {
        // Under the cap the statistic is exactly 1 by construction, so every H1
        // universe is flagged once the rule has a direction. The false-positive
        // rate is not pinned: whether an uncapped universe *reaches* its
        // ceiling of 3 is region-dependent (v0.4), and in a world this small it
        // need not, so an H0 reading below the calibration minimum is possible.
        let mut c = cfg();
        c.world.width = 48;
        c.world.height = 48;
        c.world.ticks = 20;
        c.world.seeds = 4;
        let s = survey(&c, &resident(), |_| {});
        let t = s
            .test("speed_cap", "influence_speed", Gaze::Rendering)
            .unwrap();
        assert_eq!(t.mean_h1, 1.0);
        assert_eq!(t.rule.direction, Direction::Below);
        assert_eq!(t.outcome.tpr(), 1.0);
    }

    #[test]
    fn a_survey_of_one_seed_has_nothing_to_evaluate() {
        let mut c = cfg();
        c.world.width = 32;
        c.world.height = 32;
        c.world.ticks = 10;
        let s = survey(&c, &resident(), |_| {});
        assert_eq!(s.n_cal, 0);
        assert_eq!(s.n_eval(), 1);
        for t in &s.tests {
            assert_eq!(t.rule.direction, Direction::None);
        }
    }

    #[test]
    fn standing_is_fixed_before_the_data() {
        assert!(matches!(
            standing("speed_cap", "influence_speed", Gaze::Rendering),
            Standing::Consequence(_)
        ));
        assert!(matches!(
            standing("discrete_space", "anisotropy", Gaze::Passive),
            Standing::Consequence(_)
        ));
        assert_eq!(
            standing("lazy_rendering", "smoothness", Gaze::Rendering),
            Standing::Finding
        );
        assert!(matches!(
            standing("lazy_rendering", "smoothness", Gaze::Passive),
            Standing::Consequence(_)
        ));
    }

    fn blank() -> World {
        World::seed(Geometry::new(16, 16, 1, 16), 1, 0.0)
    }

    #[test]
    fn a_birth_beside_a_corner_reads_as_diagonal_reach() {
        let mut before = blank();
        let home = vec![(8usize, 8usize, 4usize)];
        let i = before.geom.idx(9, 9);
        before.cells[i] = 1;
        let mut after = before.clone();
        let c = after.geom.idx(8, 8);
        after.cells[c] = 1;
        let (axis, diagonal) = measure_directions(&before, &after, &home);
        assert_eq!(axis, 0.0);
        assert!((diagonal - std::f64::consts::SQRT_2).abs() < 1e-12);
    }

    #[test]
    fn the_nearest_ancestor_is_nearest_in_a_straight_line() {
        let mut before = blank();
        for (x, y) in [(9, 9), (7, 8)] {
            let i = before.geom.idx(x, y);
            before.cells[i] = 1;
        }
        assert_eq!(nearest_live_euclidean(&before, 8, 8), Some((-1, 0)));
    }

    #[test]
    fn anisotropy_and_edge_excess_are_undefined_without_both_sides() {
        let e = Evidence {
            influence_speed: 1.0,
            smoothness: 0.0,
            axis_reach: 1.0,
            diagonal_reach: 0.0,
            edge_birth_rate: 0.1,
            interior_birth_rate: 0.0,
            samples: 1,
        };
        assert!(e.anisotropy().is_nan());
        assert!(e.edge_excess().is_nan());
    }
}
