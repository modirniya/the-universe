//! What an outside observer can measure about a universe, beyond one number.
//!
//! The first nine milestones judged fidelity by a single statistic: the mean
//! absolute difference between two macro density fields. The audit
//! (`docs/audit.md` §1.2) found that statistic blind in two ways — it is
//! dominated by the shared-start transient when the runs share a seed, and it
//! rewards a universe with *less* spatial variance by scoring it below the
//! chaos floor. So fidelity is now a vector. Each component is a property of
//! the universe's macro-scale behaviour that could change independently of the
//! others, and the Theory 1 experiment reports every one, in units of how much
//! that component varies between universes that differ only in seed.
//!
//! All of these are computed at the **logging threshold**: on the macro field a
//! parent would see, or on resolved cells only where cells are needed. Nothing
//! here reads an unobserved block's stale cells.
//!
//! # The observables
//!
//! - **occupancy** — mean density. The amount of structure.
//! - **churn** — mean |Δ| of the macro field between consecutive ticks. How
//!   fast the universe rewrites itself at the macro scale.
//! - **dispersion** — macro variance divided by what independent cells of the
//!   same density would give (see [`dispersion`]). 1 is chance; above is
//!   clumped; below is more even than chance.
//! - **spatial correlation** — correlation between each macro cell and the
//!   mean of its four axis neighbours on the torus. How far structure reaches.
//! - **compressibility** — runs along rows of resolved cells, as a share of
//!   what independent cells at the same density would give. Near 1 is noise;
//!   lower is structured.
//! - **components** — 8-connected clusters of live cells per thousand
//!   resolved cells. How the structure is divided up.
//! - **entropy** — Shannon entropy, in bits, of the macro densities binned
//!   sixteen ways. How varied the macro field is.
//!
//! These were chosen for being cheap, resolution-independent at the macro
//! scale, and sensitive to different things. They are not exhaustive and the
//! list is a choice; an observable missing here is a way a limit could show
//! through that this experiment would not see.

use crate::space::World;
use crate::stats;

/// One universe's macro-scale behaviour, averaged over a window of ticks.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Profile {
    pub occupancy: f64,
    pub churn: f64,
    pub dispersion: f64,
    pub spatial_corr: f64,
    pub compressibility: f64,
    pub components: f64,
    pub entropy: f64,
}

/// Names of the components, in the order [`Profile::as_array`] returns them.
pub const NAMES: [&str; 7] = [
    "occupancy",
    "churn",
    "dispersion",
    "spatial_corr",
    "compressibility",
    "components",
    "entropy",
];

impl Profile {
    pub fn as_array(&self) -> [f64; 7] {
        [
            self.occupancy,
            self.churn,
            self.dispersion,
            self.spatial_corr,
            self.compressibility,
            self.components,
            self.entropy,
        ]
    }

    pub fn from_array(a: [f64; 7]) -> Profile {
        Profile {
            occupancy: a[0],
            churn: a[1],
            dispersion: a[2],
            spatial_corr: a[3],
            compressibility: a[4],
            components: a[5],
            entropy: a[6],
        }
    }
}

/// Accumulates one sample per tick and averages at the end.
#[derive(Clone, Debug, Default)]
pub struct Accumulator {
    sums: [f64; 7],
    count: usize,
}

impl Accumulator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add one tick's sample. `prev_macro` is the previous tick's field, for
    /// churn; `None` on the first sampled tick contributes no churn sample.
    pub fn push(&mut self, w: &World, macro_field: &[f64], prev_macro: Option<&[f64]>, m: usize) {
        let s = sample(w, macro_field, prev_macro, m).as_array();
        for (acc, v) in self.sums.iter_mut().zip(s) {
            if v.is_finite() {
                *acc += v;
            }
        }
        self.count += 1;
    }

    pub fn mean(&self) -> Profile {
        if self.count == 0 {
            return Profile::default();
        }
        let n = self.count as f64;
        let mut a = self.sums;
        for v in &mut a {
            *v = *v / n + 0.0;
        }
        Profile::from_array(a)
    }

    pub fn count(&self) -> usize {
        self.count
    }
}

/// Every observable for one instant.
pub fn sample(w: &World, macro_field: &[f64], prev_macro: Option<&[f64]>, m: usize) -> Profile {
    let p = w.live_fraction();
    let churn = prev_macro.map_or(0.0, |prev| {
        crate::space::macro_divergence(prev, macro_field)
    });
    let (resolved_cells, resolved_live) = resolved_occupancy(w);
    let p_res = if resolved_cells == 0 {
        0.0
    } else {
        resolved_live as f64 / resolved_cells as f64
    };
    Profile {
        occupancy: p,
        churn,
        dispersion: dispersion(macro_field, independent_cells_per_macro(w, m)),
        spatial_corr: spatial_correlation(macro_field, m),
        compressibility: compressibility(w, p_res),
        components: if resolved_cells == 0 {
            0.0
        } else {
            components(w) as f64 * 1000.0 / resolved_cells as f64
        },
        entropy: entropy(macro_field),
    }
}

/// Cells behind one independent macro value.
///
/// The macro field is built from block densities, so when a block is larger
/// than a macro cell the macro cells inside it repeat one value and the
/// effective count is the block's. Taking the larger of the two keeps the
/// dispersion baseline honest across subdivisions.
pub fn independent_cells_per_macro(w: &World, m: usize) -> f64 {
    let per_macro = w.geom.cells() as f64 / (m * m) as f64;
    let per_block = (w.geom.block * w.geom.block) as f64;
    per_macro.max(per_block).max(1.0)
}

/// Field variance relative to uncorrelated cells of the same density.
pub fn dispersion(field: &[f64], cells_per_value: f64) -> f64 {
    if field.is_empty() {
        return 0.0;
    }
    let p = field.iter().sum::<f64>() / field.len() as f64;
    let baseline = p * (1.0 - p) / cells_per_value;
    if baseline <= 0.0 {
        return 0.0;
    }
    let var = field.iter().map(|v| (v - p) * (v - p)).sum::<f64>() / field.len() as f64;
    var / baseline
}

/// Correlation between each macro cell and the mean of its four axis
/// neighbours, on the torus. `NaN` when the field is flat.
pub fn spatial_correlation(field: &[f64], m: usize) -> f64 {
    if m < 2 || field.len() != m * m {
        return f64::NAN;
    }
    let at = |x: isize, y: isize| {
        field[(y.rem_euclid(m as isize) as usize) * m + x.rem_euclid(m as isize) as usize]
    };
    let mut neighbours = Vec::with_capacity(field.len());
    for y in 0..m as isize {
        for x in 0..m as isize {
            neighbours.push((at(x - 1, y) + at(x + 1, y) + at(x, y - 1) + at(x, y + 1)) / 4.0);
        }
    }
    stats::correlation(field, &neighbours)
}

/// Shannon entropy in bits of the macro densities binned sixteen ways.
pub fn entropy(field: &[f64]) -> f64 {
    let mut counts = [0usize; 16];
    for v in field {
        let b = ((v.clamp(0.0, 1.0) * 16.0) as usize).min(15);
        counts[b] += 1;
    }
    stats::entropy_bits(&counts)
}

/// Live cells and total cells on resolved ground.
fn resolved_occupancy(w: &World) -> (usize, usize) {
    let mut cells = 0usize;
    let mut live = 0usize;
    for b in 0..w.geom.blocks() {
        if !w.resolved[b] {
            continue;
        }
        let (x0, y0, x1, y1) = w.geom.block_bounds(b);
        for y in y0..y1 {
            for x in x0..x1 {
                cells += 1;
                live += w.cells[w.geom.idx(x, y)] as usize;
            }
        }
    }
    (cells, live)
}

/// Runs along the rows of each resolved block, as a share of what independent
/// cells at density `p` would give. Stale cells in unresolved blocks are never
/// read.
pub fn compressibility(w: &World, p: f64) -> f64 {
    let mut runs = 0usize;
    let mut pairs = 0usize;
    for b in 0..w.geom.blocks() {
        if !w.resolved[b] {
            continue;
        }
        let (x0, y0, x1, y1) = w.geom.block_bounds(b);
        for y in y0..y1 {
            for x in x0 + 1..x1 {
                pairs += 1;
                if w.cells[w.geom.idx(x, y)] != w.cells[w.geom.idx(x - 1, y)] {
                    runs += 1;
                }
            }
        }
    }
    let expected = pairs as f64 * 2.0 * p * (1.0 - p);
    if expected <= 0.0 {
        return 0.0;
    }
    runs as f64 / expected
}

/// Number of 8-connected clusters of live cells on resolved ground, on the
/// torus. Any size counts.
pub fn components(w: &World) -> usize {
    let geom = &w.geom;
    let live = |x: usize, y: usize| w.resolved[geom.block_of(x, y)] && w.cells[geom.idx(x, y)] == 1;
    let mut seen = vec![false; geom.cells()];
    let mut stack = Vec::new();
    let mut n = 0usize;
    for y0 in 0..geom.h {
        for x0 in 0..geom.w {
            let i0 = geom.idx(x0, y0);
            if seen[i0] || !live(x0, y0) {
                continue;
            }
            n += 1;
            seen[i0] = true;
            stack.push((x0, y0));
            while let Some((x, y)) = stack.pop() {
                for dy in -1..=1isize {
                    for dx in -1..=1isize {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let nx = geom.wrap_x(x as isize + dx);
                        let ny = geom.wrap_y(y as isize + dy);
                        let ni = geom.idx(nx, ny);
                        if !seen[ni] && live(nx, ny) {
                            seen[ni] = true;
                            stack.push((nx, ny));
                        }
                    }
                }
            }
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::space::Geometry;

    fn world_from(live: &[(usize, usize)]) -> World {
        let mut w = World::seed(Geometry::new(16, 16, 1, 8), 0, 0.0);
        w.cells.iter_mut().for_each(|c| *c = 0);
        for (x, y) in live {
            let i = w.geom.idx(*x, *y);
            w.cells[i] = 1;
        }
        w.sync_coarse_from_cells();
        w
    }

    #[test]
    fn components_count_clusters_not_cells() {
        let w = world_from(&[(2, 2), (3, 2), (2, 3), (10, 10), (15, 15), (0, 0)]);
        // (15,15) and (0,0) touch across the torus seam.
        assert_eq!(components(&w), 3);
        assert_eq!(components(&world_from(&[])), 0);
    }

    #[test]
    fn unresolved_ground_is_never_read() {
        let mut w = world_from(&[(2, 2), (3, 2), (10, 10)]);
        w.resolved[0] = false;
        assert_eq!(components(&w), 1);
        let (cells, live) = resolved_occupancy(&w);
        assert_eq!(cells, 256 - 64);
        assert_eq!(live, 1);
    }

    #[test]
    fn compressibility_is_about_one_for_noise_and_low_for_blocks() {
        let mut rng = crate::rng::Rng::new(3);
        let mut noise = World::seed(Geometry::new(64, 64, 1, 16), 0, 0.0);
        for c in noise.cells.iter_mut() {
            *c = u8::from(rng.chance(0.5));
        }
        noise.sync_coarse_from_cells();
        let c = compressibility(&noise, 0.5);
        assert!((0.9..1.1).contains(&c), "noise reads {c}");

        let mut stripes = World::seed(Geometry::new(64, 64, 1, 16), 0, 0.0);
        for y in 0..64 {
            for x in 0..64 {
                let i = stripes.geom.idx(x, y);
                stripes.cells[i] = u8::from(x < 32);
            }
        }
        stripes.sync_coarse_from_cells();
        assert!(compressibility(&stripes, 0.5) < 0.1);
    }

    #[test]
    fn spatial_correlation_separates_a_gradient_from_a_checkerboard() {
        let m = 8;
        // A smooth periodic bump, so the torus seam is as smooth as the rest.
        let gradient: Vec<f64> = (0..m * m)
            .map(|i| 0.5 + 0.5 * (std::f64::consts::TAU * (i % m) as f64 / m as f64).cos())
            .collect();
        let checker: Vec<f64> = (0..m * m).map(|i| ((i % m + i / m) % 2) as f64).collect();
        assert!(spatial_correlation(&gradient, m) > 0.5);
        assert!(spatial_correlation(&checker, m) < -0.9);
        assert!(spatial_correlation(&vec![0.25; m * m], m).is_nan());
    }

    #[test]
    fn dispersion_and_entropy_basics() {
        assert!(dispersion(&[0.3; 16], 10.0) < 1e-12);
        assert!(dispersion(&[0.0, 1.0, 0.0, 1.0], 10.0) > 1.0);
        assert_eq!(entropy(&[0.3; 16]), 0.0);
        assert!(entropy(&[0.0, 0.25, 0.5, 0.75]) > 1.9);
    }

    #[test]
    fn the_accumulator_averages_what_it_was_fed() {
        let w = world_from(&[(2, 2), (3, 2)]);
        let f = w.macro_field(4);
        let mut acc = Accumulator::new();
        acc.push(&w, &f, None, 4);
        acc.push(&w, &f, Some(&f), 4);
        let p = acc.mean();
        assert_eq!(acc.count(), 2);
        assert!((p.occupancy - 2.0 / 256.0).abs() < 1e-12);
        assert_eq!(p.churn, 0.0);
        assert_eq!(Profile::from_array(p.as_array()), p);
    }

    #[test]
    fn the_baseline_uses_the_block_when_it_is_the_coarser_unit() {
        let w = World::seed(Geometry::new(32, 32, 1, 16), 0, 0.3);
        // 4 macro cells of 256 cells each, blocks of 256: equal.
        assert_eq!(independent_cells_per_macro(&w, 2), 256.0);
        // 64 macro cells of 16 cells each, but blocks of 256 repeat one value.
        assert_eq!(independent_cells_per_macro(&w, 8), 256.0);
    }
}
