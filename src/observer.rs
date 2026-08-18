//! Theory 1: lazy rendering. The probe, and the moment a region is forced to
//! exist in detail.
//!
//! A **probe** is whatever compels full-resolution computation. In v0.1 it is
//! a fixed window: a rectangle of base cells, unmoving for the whole run. That
//! is the least interesting probe on purpose — it holds observation constant
//! so the cost difference is attributable to the optimization rather than to
//! the observer wandering about. Moving cursors and scripted observation
//! events are v0.2 work.
//!
//! Two events matter:
//!
//! - **Render.** A block enters observation. It has no cells, only a density,
//!   so cells are drawn from that density through the creator's RNG. Detail
//!   that was never computed is committed to at the instant it is looked at,
//!   and once committed it is as real as anything else in the world.
//! - **Collapse.** A block leaves observation. Its cells are summarised to a
//!   density and stop being computed. What made it up is gone; the aggregate
//!   survives.
//!
//! Rendering is seeded positionally, so a block rendered at a given tick gets
//! the same detail no matter what else was rendered first.
//!
//! Falsified within the model if: a run's macro observables depend on the
//! order blocks were visited, or rendering a region costs as much as having
//! computed it all along.

use crate::physics::Work;
use crate::rng::Rng;
use crate::space::{Geometry, World};
use serde::Deserialize;

/// A fixed rectangular window, in *base* cells.
///
/// Base cells rather than fine cells so that the same probe covers the same
/// physical region whether or not space has been subdivided.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Probe {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

impl Probe {
    /// Fraction of a base-sized world this probe covers.
    pub fn coverage(&self, base_w: usize, base_h: usize) -> f64 {
        let area = (self.width.min(base_w) * self.height.min(base_h)) as f64;
        area / (base_w * base_h) as f64
    }

    /// Which blocks this probe touches, in fine-grid terms.
    pub fn observed_blocks(&self, geom: &Geometry) -> Vec<bool> {
        let s = geom.scale;
        let px0 = self.x * s;
        let py0 = self.y * s;
        let pw = self.width * s;
        let ph = self.height * s;

        (0..geom.blocks())
            .map(|b| {
                let (x0, y0, x1, y1) = geom.block_bounds(b);
                overlaps_wrapped(px0, pw, x0, x1 - x0, geom.w)
                    && overlaps_wrapped(py0, ph, y0, y1 - y0, geom.h)
            })
            .collect()
    }
}

/// Do two intervals overlap on a circle of circumference `n`?
///
/// The world is a torus, so a probe may straddle the seam.
fn overlaps_wrapped(a0: usize, alen: usize, b0: usize, blen: usize, n: usize) -> bool {
    if alen == 0 || blen == 0 || n == 0 {
        return false;
    }
    if alen >= n {
        return true;
    }
    // Shift both intervals so `a` starts at 0, then test on the line.
    let b_start = (b0 + n - a0 % n) % n;
    b_start < alen || b_start + blen > n
}

/// Bring the world's fidelity into line with what is being observed.
///
/// Returns the updated world and the cost of any rendering it forced. With
/// lazy rendering off every block stays resolved and this is a no-op after the
/// first call.
pub fn observe(w: &World, probe: &Probe, tick: u64, seed: u64, lazy: bool) -> (World, Work) {
    let mut next = w.clone();
    let mut work = Work::default();

    let observed = if lazy {
        probe.observed_blocks(&w.geom)
    } else {
        vec![true; w.geom.blocks()]
    };

    for (b, &is_observed) in observed.iter().enumerate() {
        match (is_observed, w.resolved[b]) {
            (true, false) => {
                work.cells_rendered += render_block(&mut next, b, tick, seed) as u64;
                next.resolved[b] = true;
            }
            (false, true) => {
                next.coarse[b] = w.density_from_cells(b);
                next.resolved[b] = false;
            }
            _ => {}
        }
    }

    (next, work)
}

/// Draw a block's cells from its density. The render event itself.
fn render_block(w: &mut World, b: usize, tick: u64, seed: u64) -> usize {
    let (x0, y0, x1, y1) = w.geom.block_bounds(b);
    let d = w.coarse[b];
    let mut rng = Rng::derive(seed, b as u64, tick, 0x5245_4E44_4552);
    let mut n = 0;
    for y in y0..y1 {
        for x in x0..x1 {
            let i = w.geom.idx(x, y);
            w.cells[i] = u8::from(rng.chance(d));
            n += 1;
        }
    }
    // Rendering commits to a sample, so the block's density shifts slightly
    // off the mean it was drawn from. Record what is actually there.
    w.coarse[b] = w.density_from_cells(b);
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geom() -> Geometry {
        Geometry::new(32, 32, 1, 8)
    }

    fn probe() -> Probe {
        Probe {
            x: 0,
            y: 0,
            width: 8,
            height: 8,
        }
    }

    #[test]
    fn probe_covers_one_block_of_sixteen() {
        let obs = probe().observed_blocks(&geom());
        assert_eq!(obs.iter().filter(|b| **b).count(), 1);
        assert!(obs[0]);
    }

    #[test]
    fn probe_scales_with_subdivision() {
        // The same probe must cover the same physical region at finer space.
        let coarse = probe().observed_blocks(&Geometry::new(32, 32, 1, 8));
        let fine = probe().observed_blocks(&Geometry::new(32, 32, 2, 16));
        assert_eq!(
            coarse.iter().filter(|b| **b).count(),
            fine.iter().filter(|b| **b).count()
        );
    }

    #[test]
    fn probe_can_straddle_the_seam() {
        let p = Probe {
            x: 30,
            y: 0,
            width: 4,
            height: 8,
        };
        let obs = p.observed_blocks(&geom());
        assert!(obs[3], "should touch the right-hand block");
        assert!(obs[0], "and wrap onto the left-hand one");
    }

    #[test]
    fn coverage_is_a_fraction() {
        assert!((probe().coverage(32, 32) - 0.0625).abs() < 1e-12);
    }

    #[test]
    fn unobserved_blocks_collapse_when_lazy() {
        let w = World::seed(geom(), 7, 0.3);
        let (next, _) = observe(&w, &probe(), 0, 7, true);
        assert_eq!(next.resolved.iter().filter(|r| **r).count(), 1);
        assert!(next.live_state_bytes() < w.live_state_bytes());
    }

    #[test]
    fn collapse_preserves_the_density_it_summarises() {
        let w = World::seed(geom(), 7, 0.3);
        let (next, _) = observe(&w, &probe(), 0, 7, true);
        for b in 0..w.geom.blocks() {
            assert!((next.coarse[b] - w.density_from_cells(b)).abs() < 1e-12);
        }
    }

    #[test]
    fn nothing_collapses_when_lazy_is_off() {
        let w = World::seed(geom(), 7, 0.3);
        let (next, work) = observe(&w, &probe(), 0, 7, false);
        assert!(next.resolved.iter().all(|r| *r));
        assert_eq!(work, Work::default());
    }

    #[test]
    fn rendering_reproduces_the_density_it_was_drawn_from() {
        let mut w = World::seed(Geometry::new(64, 64, 1, 16), 7, 0.3);
        for b in 0..w.geom.blocks() {
            w.resolved[b] = false;
            w.coarse[b] = 0.5;
        }
        let p = Probe {
            x: 0,
            y: 0,
            width: 64,
            height: 64,
        };
        let (next, work) = observe(&w, &p, 3, 7, true);
        assert!(next.resolved.iter().all(|r| *r));
        assert_eq!(work.cells_rendered, next.geom.cells() as u64);
        // Drawn, not copied: close to 0.5 but not exactly.
        assert!((next.live_fraction() - 0.5).abs() < 0.05);
    }

    #[test]
    fn rendering_is_order_independent() {
        // Two worlds identical except for the order blocks appear to be
        // visited must render identically.
        let mut a = World::seed(geom(), 7, 0.3);
        for b in 0..a.geom.blocks() {
            a.resolved[b] = false;
            a.coarse[b] = 0.4;
        }
        let b = a.clone();
        let p = Probe {
            x: 0,
            y: 0,
            width: 32,
            height: 32,
        };
        let (ra, _) = observe(&a, &p, 5, 7, true);
        let (rb, _) = observe(&b, &p, 5, 7, true);
        assert_eq!(ra.cells, rb.cells);
    }

    #[test]
    fn render_depends_on_tick() {
        let mut w = World::seed(geom(), 7, 0.3);
        for b in 0..w.geom.blocks() {
            w.resolved[b] = false;
            w.coarse[b] = 0.4;
        }
        let p = Probe {
            x: 0,
            y: 0,
            width: 32,
            height: 32,
        };
        let at_3 = observe(&w, &p, 3, 7, true).0;
        let at_4 = observe(&w, &p, 4, 7, true).0;
        assert_ne!(at_3.cells, at_4.cells);
    }
}
