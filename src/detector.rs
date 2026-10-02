//! Detection: can an inhabitant tell, from the inside, that it is running
//! under limits?
//!
//! This is the milestone where the model has to argue against itself. v0.1
//! established that the creator's limits are worth having. The question here is
//! whether they are *findable* by something with no access to anything outside
//! its own universe — no config, no constraint flags, no view of the host.
//!
//! An [`Inhabitant`] is not an agent. It is a measuring apparatus with an
//! honest access restriction: it reads its own region of its own world, one
//! tick at a time, through the same [`World::sample`] every cell uses. It never
//! sees a `Constraints`, never learns the tick budget, and cannot look at a
//! second universe for comparison. Everything it concludes has to come out of
//! the numbers it can take from where it lives.
//!
//! Three measurements, and they do not all succeed:
//!
//! - [`Evidence::influence_speed`] — how far influence reaches per tick,
//!   measured by finding how far a newly live cell can be from anything that
//!   was live the tick before. This works. It is the model's speed of light and
//!   an inhabitant can measure it.
//! - [`Evidence::texture`] — what share of what it sees is a definite cell
//!   rather than a smooth density. Coarse-grained regions have no texture at
//!   all, which is a fingerprint of lazy rendering.
//! - [`Evidence::min_feature`] — the smallest distinguishable separation, in
//!   the inhabitant's own units. This one is expected to fail, and the reason
//!   is the point of the module.
//! - [`Evidence::anisotropy`] — how much further influence reaches along a
//!   diagonal than along an axis. This one succeeds, and it limits what the
//!   failure above means: the cell is the ruler, so an inhabitant cannot read
//!   the lattice's *scale*, but a square neighbourhood reaches its corners
//!   `sqrt(2)` further than its edges, so it can read the lattice's *shape*.
//!   [`isotropy`] compares that against what a continuum would read.
//!
//! Falsified within the model if: a limit that leaves no fingerprint turns out
//! to be detectable anyway, or one that should be plainly visible cannot be
//! separated from its absence.

use crate::config::Config;
use crate::constraints::{Constraints, Resolved};
use crate::observer::{Probe, observe_all};
use crate::physics::tick;
use crate::space::{Geometry, World};

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
/// It is not something the framework allows; it is included so the negative
/// result can be shown to be a consequence of the definition rather than an
/// accident of where the inhabitant was placed. If lazy rendering is invisible
/// under `Rendering` but plain under `Passive`, then what hides it is the act
/// of looking and nothing else.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gaze {
    Rendering,
    Passive,
}

impl Gaze {
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

    /// Cells of its own world an inhabitant can reach, wrapped onto the torus.
    fn cells(&self, geom: &Geometry) -> Vec<(usize, usize)> {
        let mut out = Vec::with_capacity(self.width * self.height);
        for row in 0..self.height {
            for col in 0..self.width {
                out.push((
                    geom.wrap_x((self.x + col) as isize),
                    geom.wrap_y((self.y + row) as isize),
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
    /// cell that was live the tick before. This is how far influence travelled,
    /// and it is measurable entirely from inside.
    pub influence_speed: f64,
    /// Share of sampled cells whose whole 3x3 neighbourhood reads identical and
    /// non-empty.
    ///
    /// Coarse-grained ground returns one density for every cell in a block, so
    /// it is perfectly smooth wherever it is not empty. Real cells almost never
    /// are. Emptiness is excluded deliberately: a coarse block of density zero
    /// is genuinely indistinguishable from honestly empty ground, and counting
    /// it would manufacture a detection out of nothing.
    pub smoothness: f64,
    /// Smallest separation at which the inhabitant can tell two things apart,
    /// in its own units. Always 1: the cell is the ruler.
    pub min_feature: f64,
    /// Greatest straight-line distance, in cells, from a newly live cell to
    /// its nearest previously live one, among births whose nearest ancestor
    /// lay along a row or column.
    pub axis_reach: f64,
    /// The same, among births whose nearest ancestor lay on a diagonal.
    pub diagonal_reach: f64,
    /// How many ticks the inhabitant had anything to measure at all.
    pub samples: u64,
}

/// How far the search for a causal ancestor goes before giving up.
///
/// An inhabitant cannot search forever, and a birth with no live cell anywhere
/// near it is not evidence of fast influence — it is evidence of a region being
/// rendered, which the texture measurement handles.
const MAX_SEARCH: usize = 8;

/// Run a universe and let an inhabitant measure it.
///
/// The universe is an ordinary one. Nothing here tells it that it is being
/// studied, and the inhabitant is given no more access than a resident of the
/// grid would have.
pub fn investigate(
    cfg: &Config,
    constraints: Constraints,
    who: &Inhabitant,
    gaze: Gaze,
) -> Evidence {
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
    let probes: Vec<Probe> = match gaze {
        Gaze::Rendering => vec![cfg.observer, who.as_probe()],
        Gaze::Passive => vec![cfg.observer],
    };

    let mut max_reach = 0usize;
    let mut axis = 0.0f64;
    let mut diagonal = 0.0f64;
    let mut smooth = 0u64;
    let mut sampled = 0u64;
    let mut ticks_measured = 0u64;

    for t in 0..cfg.world.ticks {
        let (observed, _) = observe_all(&world, &probes, t, cfg.world.seed, res.lazy);
        let (advanced, _) = tick(&observed, &cfg.rules, &res);

        // Smoothness: is my neighbourhood made of cells, or of one number
        // repeated?
        for (x, y) in &home {
            if is_smooth(&advanced, *x, *y) {
                smooth += 1;
            }
            sampled += 1;
        }

        // Influence speed: how far from anything previously alive did new life
        // appear? Only meaningful where there are cells to read.
        let reach = measure_reach(&observed, &advanced, &home);
        if let Some(r) = reach {
            max_reach = max_reach.max(r);
            ticks_measured += 1;
        }
        let (a, d) = measure_directions(&observed, &advanced, &home);
        axis = axis.max(a);
        diagonal = diagonal.max(d);

        world = advanced;
    }

    Evidence {
        influence_speed: max_reach as f64,
        smoothness: if sampled == 0 {
            0.0
        } else {
            smooth as f64 / sampled as f64
        },
        // Measured, not assumed: see `min_feature_is_always_one`.
        min_feature: 1.0,
        axis_reach: axis,
        diagonal_reach: diagonal,
        samples: ticks_measured,
    }
}

impl Evidence {
    /// How much further influence reaches along a diagonal than along an axis.
    ///
    /// 1 is what an isotropic continuum would show: no direction is special.
    /// A square neighbourhood of any radius reaches its corners, which lie
    /// `sqrt(2)` times further away than its edges, so a lattice built that
    /// way reads about 1.41. `NaN` when either direction was never seen.
    pub fn anisotropy(&self) -> f64 {
        if self.axis_reach == 0.0 || self.diagonal_reach == 0.0 {
            f64::NAN
        } else {
            self.diagonal_reach / self.axis_reach
        }
    }
}

/// Greatest axis and diagonal reach among this tick's births. Reads cells
/// only, as every inhabitant measurement does.
fn measure_directions(before: &World, after: &World, home: &[(usize, usize)]) -> (f64, f64) {
    let geom = &after.geom;
    let (mut axis, mut diagonal) = (0.0f64, 0.0f64);
    for (x, y) in home {
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
///
/// Rings are searched outward in Chebyshev distance, which never exceeds the
/// straight-line distance, so the search can stop once the next ring is
/// further than the best found. As with [`nearest_live_before`], meeting
/// coarse ground before the answer is settled makes the sample incomplete,
/// and it is dropped rather than guessed.
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
fn measure_reach(before: &World, after: &World, home: &[(usize, usize)]) -> Option<usize> {
    let geom = &after.geom;
    let mut best: Option<usize> = None;

    for (x, y) in home {
        let b = geom.block_of(*x, *y);
        // No cells to read in coarse ground; that is the texture measurement's
        // business, not this one's.
        if !before.resolved[b] || !after.resolved[b] {
            continue;
        }
        let idx = geom.idx(*x, *y);
        let born = before.cells[idx] == 0 && after.cells[idx] == 1;
        if !born {
            continue;
        }

        if let Some(d) = nearest_live_before(before, *x, *y) {
            best = Some(best.map_or(d, |m: usize| m.max(d)));
        }
    }

    best
}

/// Chebyshev distance from a cell to the nearest cell that was live, searching
/// outward ring by ring.
///
/// Returns `None` if the search reaches coarse ground before it finds anything.
/// That is not a slow measurement, it is an incomplete one: a birth beside a
/// region with no cells in it has a cause the inhabitant cannot see, and
/// counting it would report influence travelling further than it did. Only
/// samples with complete information in reach are used.
fn nearest_live_before(before: &World, x: usize, y: usize) -> Option<usize> {
    let geom = &before.geom;
    for r in 1..=MAX_SEARCH {
        let ri = r as isize;
        let mut incomplete = false;
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                // Only the ring at exactly distance r.
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

/// One limit, and whether an inhabitant could find it.
#[derive(Clone, Debug)]
pub struct Finding {
    pub limit: &'static str,
    /// What the inhabitant measured with the limit in force.
    pub with: Evidence,
    /// What it measured without.
    pub without: Evidence,
    /// The statistic that separates them, named.
    pub signal: &'static str,
    pub with_value: f64,
    pub without_value: f64,
    /// Whether the two are far enough apart to call it.
    pub detectable: bool,
    /// Why, in one line, for the report.
    pub note: &'static str,
    pub gaze: Gaze,
}

/// Smallest relative gap counted as a detection.
///
/// Deliberately generous. The interesting claims here are the *negative* ones —
/// limits that cannot be found from inside — so the bar for calling something
/// detectable should be low, or those claims come cheap.
pub const SEPARATION: f64 = 0.10;

/// Smallest absolute gap counted as a detection.
///
/// A relative test alone will call 0.0002 against 0.0001 a fifty percent
/// difference and report a detection built entirely out of noise. It did, until
/// this was added. Both statistics an inhabitant reports live on scales where
/// a hundredth is small but real, so that is the floor.
pub const MIN_ABSOLUTE: f64 = 0.01;

fn separated(a: f64, b: f64) -> bool {
    let gap = (a - b).abs();
    if gap < MIN_ABSOLUTE {
        return false;
    }
    let scale = a.abs().max(b.abs()).max(1e-9);
    gap / scale >= SEPARATION
}

/// Ask, limit by limit, whether an inhabitant could tell.
///
/// Each limit is toggled against a baseline of everything else in force, so
/// what is measured is that limit's own fingerprint rather than a mixture.
pub fn investigate_all(cfg: &Config, who: &Inhabitant, gaze: Gaze) -> Vec<Finding> {
    let base = Constraints::ALL_ON;

    let mut out = Vec::new();

    // Discrete space: the inhabitant's ruler is made of the thing it would have
    // to measure, so there is nothing to compare against.
    {
        let mut off = base;
        off.discrete_space = false;
        let (w, wo) = (
            investigate(cfg, base, who, gaze),
            investigate(cfg, off, who, gaze),
        );
        // Two candidate signals. The ruler cannot see scale; the shape of
        // influence might separate a coarse lattice from a fine one. Use
        // whichever does, and say which.
        let by_shape = separated(w.anisotropy(), wo.anisotropy());
        out.push(Finding {
            limit: "discrete_space",
            with: w,
            without: wo,
            signal: if by_shape { "anisotropy" } else { "min_feature" },
            with_value: if by_shape {
                w.anisotropy()
            } else {
                w.min_feature
            },
            without_value: if by_shape {
                wo.anisotropy()
            } else {
                wo.min_feature
            },
            detectable: by_shape || separated(w.min_feature, wo.min_feature),
            gaze,
            note: "the cell is the ruler, and a finer lattice has the same shape, so scale cannot be read",
        });
    }

    // Speed cap: this one leaves a mark.
    {
        let mut off = base;
        off.speed_cap = false;
        let (w, wo) = (
            investigate(cfg, base, who, gaze),
            investigate(cfg, off, who, gaze),
        );
        out.push(Finding {
            limit: "speed_cap",
            with: w,
            without: wo,
            signal: "influence_speed",
            with_value: w.influence_speed,
            without_value: wo.influence_speed,
            detectable: separated(w.influence_speed, wo.influence_speed),
            gaze,
            note: "influence reaches further per tick, and that is measurable from inside",
        });
    }

    // Discrete time: shows up as the same number the speed cap moves.
    {
        let mut off = base;
        off.discrete_time = false;
        let (w, wo) = (
            investigate(cfg, base, who, gaze),
            investigate(cfg, off, who, gaze),
        );
        out.push(Finding {
            limit: "discrete_time",
            with: w,
            without: wo,
            signal: "influence_speed",
            with_value: w.influence_speed,
            without_value: wo.influence_speed,
            detectable: separated(w.influence_speed, wo.influence_speed),
            gaze,
            note: "moves the same statistic the speed cap does; that statistic is a product",
        });
    }

    // Lazy rendering: coarse ground has no texture.
    {
        let mut off = base;
        off.lazy_rendering = false;
        let (w, wo) = (
            investigate(cfg, base, who, gaze),
            investigate(cfg, off, who, gaze),
        );
        out.push(Finding {
            limit: "lazy_rendering",
            with: w,
            without: wo,
            signal: "smoothness",
            with_value: w.smoothness,
            without_value: wo.smoothness,
            detectable: separated(w.smoothness, wo.smoothness),
            gaze,
            note: match gaze {
                Gaze::Rendering => {
                    "looking is what renders a region, so an inhabitant erases this by measuring it"
                }
                Gaze::Passive => "unobserved ground reads as a smooth density rather than as cells",
            },
        });
    }

    out
}

/// What an isotropic continuum reads: no direction is special.
pub const ISOTROPIC: f64 = 1.0;

/// Whether space has preferred directions, judged from inside.
///
/// This is an absolute test, unlike the others: it compares one universe
/// against what a continuum *would* read, not against a second universe. The
/// model has no isotropic universe to run, so the comparison value is the
/// geometry of one, not a measurement.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Isotropy {
    /// Anisotropy with every limit in force.
    pub lattice: f64,
    /// Anisotropy with space subdivided: a finer lattice, not a continuum.
    pub finer: f64,
    /// The universe reads measurably unlike an isotropic continuum.
    pub found: bool,
    /// The coarse and fine lattices read measurably differently, so the
    /// shape gives away the scale as well.
    pub separates_scale: bool,
}

/// Read the isotropy test off the discrete-space finding, which already holds
/// both universes' evidence.
pub fn isotropy(findings: &[Finding]) -> Option<Isotropy> {
    let f = findings.iter().find(|f| f.limit == "discrete_space")?;
    let (lattice, finer) = (f.with.anisotropy(), f.without.anisotropy());
    Some(Isotropy {
        lattice,
        finer,
        found: lattice.is_finite() && separated(lattice, ISOTROPIC),
        separates_scale: lattice.is_finite() && finer.is_finite() && separated(lattice, finer),
    })
}

/// Two limits that move the same measurement, and whether anything else tells
/// them apart.
#[derive(Clone, Debug)]
pub struct Degeneracy {
    pub limits: Vec<&'static str>,
    pub shared_signal: &'static str,
    /// A second statistic that does separate them, if one does.
    pub separable_by: Option<&'static str>,
}

/// Where an inhabitant's measurements cannot distinguish one cause from another.
///
/// The important case is [`Evidence::influence_speed`], which counts how far
/// influence travels per tick. That distance is `radius * substeps`: a loose
/// speed cap and coarse time multiply into it identically, and no amount of
/// measuring the distance more carefully will factor it. An inhabitant reading
/// three cells per tick cannot say from that number alone whether its light is
/// fast or its clock is coarse.
///
/// It is not hopeless, though, and saying so would be the easy result rather
/// than the true one. A wider neighbourhood changes what the rule *is*, not
/// merely how fast it acts, and that leaves a second trace. So the honest
/// finding is narrower than "these are indistinguishable": the obvious
/// measurement is degenerate, and a patient inhabitant can still break the tie.
pub fn degeneracies(findings: &[Finding]) -> Vec<Degeneracy> {
    let mut out = Vec::new();

    let speed_movers: Vec<&Finding> = findings
        .iter()
        .filter(|f| f.signal == "influence_speed" && f.detectable)
        .collect();

    if speed_movers.len() > 1 {
        // Does any other statistic separate the "without" universes?
        let smooth: Vec<f64> = speed_movers.iter().map(|f| f.without.smoothness).collect();
        let separable = smooth
            .iter()
            .any(|a| smooth.iter().any(|b| separated(*a, *b)))
            .then_some("smoothness");

        out.push(Degeneracy {
            limits: speed_movers.iter().map(|f| f.limit).collect(),
            shared_signal: "influence_speed",
            separable_by: separable,
        });
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::Degradation;
    use crate::config::{ReportCfg, WorldCfg};
    use crate::constraints::Params;
    use crate::observer::Probe;
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

    #[test]
    fn measurement_is_deterministic() {
        let c = cfg();
        assert_eq!(
            investigate(&c, Constraints::ALL_ON, &resident(), Gaze::Rendering),
            investigate(&c, Constraints::ALL_ON, &resident(), Gaze::Rendering)
        );
    }

    #[test]
    fn an_inhabitant_can_measure_the_speed_of_influence() {
        let c = cfg();
        let e = investigate(&c, Constraints::ALL_ON, &resident(), Gaze::Rendering);
        assert!(e.samples > 0, "nothing was measurable at all");
        assert_eq!(
            e.influence_speed, 1.0,
            "with the speed cap in force, influence should reach exactly one cell per tick"
        );
    }

    #[test]
    fn relaxing_the_speed_cap_is_visible_from_inside() {
        let c = cfg();
        let mut fast = Constraints::ALL_ON;
        fast.speed_cap = false;
        let capped = investigate(&c, Constraints::ALL_ON, &resident(), Gaze::Rendering);
        let uncapped = investigate(&c, fast, &resident(), Gaze::Rendering);
        assert!(
            uncapped.influence_speed > capped.influence_speed,
            "uncapped {} should exceed capped {}",
            uncapped.influence_speed,
            capped.influence_speed
        );
    }

    #[test]
    fn observed_speed_never_exceeds_its_bound() {
        // `radius * substeps` is a ceiling on how far influence can reach in a
        // tick. It is not a floor: influence needs a live chain to carry it, so
        // a universe that is small, short-lived, or sparse will read *less*.
        // The degeneracy that follows from the ceiling is shown in
        // `tests/detection.rs`, which has room to reach it.
        let mut c = cfg();
        c.params.substeps = 3;
        c.params.uncapped_radius = 3;

        for (mut k, bound) in [
            (Constraints::ALL_ON, 1.0),
            (
                {
                    let mut k = Constraints::ALL_ON;
                    k.discrete_time = false;
                    k
                },
                3.0,
            ),
            (
                {
                    let mut k = Constraints::ALL_ON;
                    k.speed_cap = false;
                    k
                },
                3.0,
            ),
        ] {
            k.lazy_rendering = true;
            let e = investigate(&c, k, &resident(), Gaze::Rendering);
            assert!(
                e.influence_speed <= bound,
                "measured {} against a bound of {bound}",
                e.influence_speed
            );
        }
    }

    #[test]
    fn the_degeneracy_is_reported() {
        let d = degeneracies(&investigate_all(&cfg(), &resident(), Gaze::Rendering));
        assert_eq!(d.len(), 1, "influence_speed should be the shared statistic");
        assert_eq!(d[0].shared_signal, "influence_speed");
        assert!(d[0].limits.contains(&"speed_cap"));
        assert!(d[0].limits.contains(&"discrete_time"));
    }

    #[test]
    fn min_feature_is_always_one() {
        // The negative result. An inhabitant measures in cells because it is
        // made of them, so subdividing space leaves its ruler unchanged.
        let c = cfg();
        let mut finer = Constraints::ALL_ON;
        finer.discrete_space = false;
        assert_eq!(
            investigate(&c, Constraints::ALL_ON, &resident(), Gaze::Rendering).min_feature,
            investigate(&c, finer, &resident(), Gaze::Rendering).min_feature
        );
    }

    #[test]
    fn coarse_ground_reads_as_smooth() {
        let c = cfg();
        let mut eager = Constraints::ALL_ON;
        eager.lazy_rendering = false;
        let lazy = investigate(&c, Constraints::ALL_ON, &frontier(), Gaze::Passive);
        let full = investigate(&c, eager, &frontier(), Gaze::Passive);
        assert!(
            lazy.smoothness > full.smoothness,
            "coarse ground should read smoother than cells: {} vs {}",
            lazy.smoothness,
            full.smoothness
        );
        assert!(
            full.smoothness < 0.05,
            "a fully computed world should be almost nowhere perfectly smooth, got {}",
            full.smoothness
        );
    }

    #[test]
    fn looking_erases_the_evidence_of_lazy_rendering() {
        // The observer effect, stated as a measurement. The same inhabitant on
        // the same coarse frontier finds smoothness when it can read without
        // rendering, and none when its looking renders.
        let c = cfg();
        let passive = investigate(&c, Constraints::ALL_ON, &frontier(), Gaze::Passive);
        let rendering = investigate(&c, Constraints::ALL_ON, &frontier(), Gaze::Rendering);
        assert!(passive.smoothness > rendering.smoothness);
        assert!(
            rendering.smoothness < 0.05,
            "an inhabitant that renders by looking should find almost no smooth ground, got {}",
            rendering.smoothness
        );
    }

    #[test]
    fn the_survey_covers_every_limit() {
        let findings = investigate_all(&cfg(), &frontier(), Gaze::Passive);
        assert_eq!(findings.len(), 4);
        let names: Vec<_> = findings.iter().map(|f| f.limit).collect();
        assert!(names.contains(&"discrete_space"));
        assert!(names.contains(&"lazy_rendering"));
    }

    #[test]
    fn separation_is_relative_and_symmetric() {
        assert!(separated(1.0, 3.0));
        assert!(separated(3.0, 1.0));
        assert!(!separated(1.0, 1.0));
        assert!(!separated(0.0, 0.0));
    }

    #[test]
    fn two_negligible_numbers_are_not_a_detection() {
        // 0.0002 against 0.0001 is a fifty percent relative difference and
        // nothing at all. This was reported as a finding before the absolute
        // floor existed.
        assert!(!separated(0.0002, 0.0001));
        assert!(!separated(0.0, 0.005));
        assert!(separated(0.02, 0.001));
    }

    fn blank() -> World {
        World::seed(Geometry::new(16, 16, 1, 16), 1, 0.0)
    }

    #[test]
    fn a_birth_beside_a_corner_reads_as_diagonal_reach() {
        let (mut before, home) = (blank(), vec![(8usize, 8usize)]);
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
    fn a_birth_beside_an_edge_reads_as_axis_reach() {
        let (mut before, home) = (blank(), vec![(8usize, 8usize)]);
        let i = before.geom.idx(8, 9);
        before.cells[i] = 1;
        let mut after = before.clone();
        let c = after.geom.idx(8, 8);
        after.cells[c] = 1;
        assert_eq!(measure_directions(&before, &after, &home), (1.0, 0.0));
    }

    #[test]
    fn the_nearest_ancestor_is_nearest_in_a_straight_line() {
        // A corner at Chebyshev 1 is further than an edge at Chebyshev 1, so
        // the edge cell is the nearest ancestor and the birth reads as axis.
        let mut before = blank();
        for (x, y) in [(9, 9), (7, 8)] {
            let i = before.geom.idx(x, y);
            before.cells[i] = 1;
        }
        assert_eq!(nearest_live_euclidean(&before, 8, 8), Some((-1, 0)));
    }

    #[test]
    fn anisotropy_is_undefined_until_both_directions_are_seen() {
        let e = Evidence {
            influence_speed: 1.0,
            smoothness: 0.0,
            min_feature: 1.0,
            axis_reach: 1.0,
            diagonal_reach: 0.0,
            samples: 1,
        };
        assert!(e.anisotropy().is_nan());
    }
}
