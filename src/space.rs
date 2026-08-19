//! Theory 1: discrete space, and the storage that makes lazy rendering real.
//!
//! The world is a torus of cells. It is stored twice, at two fidelities:
//!
//! - `cells`  — one byte per cell, authoritative inside *resolved* blocks.
//! - `coarse` — one density per block, authoritative inside *unresolved* ones.
//!
//! A block is resolved only while something observes it. That is not a display
//! trick: unresolved blocks are never computed per-cell, and neighbouring
//! resolved cells read them as a density rather than as detail. Fidelity is
//! paid for exactly where it is looked at.
//!
//! Falsified within the model if: a run with lazy rendering on and a run with
//! it off produce the same cost. Then the optimization buys nothing and the
//! creator had no reason to write it.

use crate::rng::Rng;

/// Fine-grid dimensions and the block partition laid over them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Geometry {
    /// Fine grid width in cells.
    pub w: usize,
    /// Fine grid height in cells.
    pub h: usize,
    /// Fine cells per block edge.
    pub block: usize,
    /// Blocks across.
    pub bw: usize,
    /// Blocks down.
    pub bh: usize,
    /// Fine cells per base cell, per axis. 1 when space is discrete.
    pub scale: usize,
}

impl Geometry {
    /// `base_w`/`base_h` are in base cells; the fine grid is `scale` times
    /// finer per axis. Block edges are rounded up so the partition covers the
    /// grid even when it does not divide evenly.
    pub fn new(base_w: usize, base_h: usize, scale: usize, block: usize) -> Self {
        assert!(scale >= 1 && block >= 1, "scale and block must be >= 1");
        let w = base_w * scale;
        let h = base_h * scale;
        Geometry {
            w,
            h,
            block,
            bw: w.div_ceil(block),
            bh: h.div_ceil(block),
            scale,
        }
    }

    #[inline]
    pub fn cells(&self) -> usize {
        self.w * self.h
    }

    #[inline]
    pub fn blocks(&self) -> usize {
        self.bw * self.bh
    }

    #[inline]
    pub fn idx(&self, x: usize, y: usize) -> usize {
        y * self.w + x
    }

    /// Block index owning a fine cell.
    #[inline]
    pub fn block_of(&self, x: usize, y: usize) -> usize {
        (y / self.block) * self.bw + (x / self.block)
    }

    /// Half-open fine-cell bounds of a block: `(x0, y0, x1, y1)`.
    pub fn block_bounds(&self, b: usize) -> (usize, usize, usize, usize) {
        let bx = b % self.bw;
        let by = b / self.bw;
        let x0 = bx * self.block;
        let y0 = by * self.block;
        (
            (x0),
            (y0),
            (x0 + self.block).min(self.w),
            (y0 + self.block).min(self.h),
        )
    }

    /// Cells actually inside a block, accounting for a ragged right/bottom edge.
    pub fn block_cells(&self, b: usize) -> usize {
        let (x0, y0, x1, y1) = self.block_bounds(b);
        (x1 - x0) * (y1 - y0)
    }

    /// Toroidal wrap. The universe has no edge for a probe to fall off.
    #[inline]
    pub fn wrap_x(&self, x: isize) -> usize {
        x.rem_euclid(self.w as isize) as usize
    }

    #[inline]
    pub fn wrap_y(&self, y: isize) -> usize {
        y.rem_euclid(self.h as isize) as usize
    }
}

/// One universe's state at one instant.
#[derive(Clone, Debug)]
pub struct World {
    pub geom: Geometry,
    /// Per-cell occupancy, 0 or 1. Meaningful only inside resolved blocks.
    pub cells: Vec<u8>,
    /// Per-block mean occupancy in `[0, 1]`. Authoritative when unresolved,
    /// and kept in step with `cells` when resolved.
    pub coarse: Vec<f64>,
    /// Whether each block is currently computed per-cell.
    pub resolved: Vec<bool>,
}

impl World {
    /// Seed a world from the creator's RNG.
    ///
    /// The pattern is drawn at *base* resolution and then upsampled, so that
    /// a coarse universe and a subdivided one begin from the same macro
    /// configuration. Without this the resolution comparison would be a
    /// comparison of two different initial conditions.
    pub fn seed(geom: Geometry, seed: u64, density: f64) -> Self {
        let base_w = geom.w / geom.scale;
        let base_h = geom.h / geom.scale;
        let mut rng = Rng::new(seed);
        let base: Vec<u8> = (0..base_w * base_h)
            .map(|_| u8::from(rng.chance(density)))
            .collect();

        let mut cells = vec![0u8; geom.cells()];
        for y in 0..geom.h {
            for x in 0..geom.w {
                let bx = x / geom.scale;
                let by = y / geom.scale;
                cells[geom.idx(x, y)] = base[by * base_w + bx];
            }
        }

        let mut w = World {
            geom,
            cells,
            coarse: vec![0.0; geom.blocks()],
            resolved: vec![true; geom.blocks()],
        };
        w.sync_coarse_from_cells();
        w
    }

    /// Recompute every block density from its cells.
    pub fn sync_coarse_from_cells(&mut self) {
        for b in 0..self.geom.blocks() {
            self.coarse[b] = self.density_from_cells(b);
        }
    }

    /// Mean occupancy of a block, read from its cells.
    pub fn density_from_cells(&self, b: usize) -> f64 {
        let (x0, y0, x1, y1) = self.geom.block_bounds(b);
        let mut sum = 0u32;
        for y in y0..y1 {
            for x in x0..x1 {
                sum += self.cells[self.geom.idx(x, y)] as u32;
            }
        }
        let n = ((x1 - x0) * (y1 - y0)) as f64;
        if n == 0.0 { 0.0 } else { sum as f64 / n }
    }

    /// What a cell contributes to a neighbour's count.
    ///
    /// Inside a resolved block that is the cell itself. Inside an unresolved
    /// one there is no cell to read, so the block's density stands in for it:
    /// the expected occupancy, which is all a coarse region can offer. This
    /// is where the cost saving of lazy rendering is actually taken, and
    /// where its error enters.
    #[inline]
    pub fn sample(&self, x: usize, y: usize) -> f64 {
        let b = self.geom.block_of(x, y);
        if self.resolved[b] {
            self.cells[self.geom.idx(x, y)] as f64
        } else {
            self.coarse[b]
        }
    }

    /// Occupancy summed over the whole world, in cells.
    pub fn live_fraction(&self) -> f64 {
        let mut sum = 0.0;
        let mut n = 0.0;
        for b in 0..self.geom.blocks() {
            let c = self.geom.block_cells(b) as f64;
            sum += self.coarse[b] * c;
            n += c;
        }
        if n == 0.0 { 0.0 } else { sum / n }
    }

    /// The macro density field: the world downsampled to `m * m`.
    ///
    /// This is the *logging threshold* made concrete. A parent's observer does
    /// not see cells; it sees aggregates at this scale. Two universes are
    /// compared here and nowhere else, which is what makes runs at different
    /// internal resolutions comparable at all.
    pub fn macro_field(&self, m: usize) -> Vec<f64> {
        let mut sum = vec![0.0f64; m * m];
        let mut count = vec![0.0f64; m * m];
        for b in 0..self.geom.blocks() {
            let (x0, y0, x1, y1) = self.geom.block_bounds(b);
            let d = self.coarse[b];
            for y in y0..y1 {
                let my = y * m / self.geom.h;
                for x in x0..x1 {
                    let mx = x * m / self.geom.w;
                    sum[my * m + mx] += d;
                    count[my * m + mx] += 1.0;
                }
            }
        }
        sum.iter()
            .zip(count.iter())
            .map(|(s, c)| if *c == 0.0 { 0.0 } else { s / c })
            .collect()
    }

    /// Bytes a resource-honest implementation would hold: cells only for
    /// resolved blocks, plus one density and one flag per block.
    ///
    /// Reported separately from [`Self::allocated_bytes`] because this crate
    /// keeps the full cell buffer allocated for simplicity. Claiming the
    /// smaller number as measured RSS would be a lie; claiming the larger one
    /// would hide what the optimization is for.
    pub fn live_state_bytes(&self) -> usize {
        let resolved_cells: usize = (0..self.geom.blocks())
            .filter(|b| self.resolved[*b])
            .map(|b| self.geom.block_cells(b))
            .sum();
        resolved_cells + self.geom.blocks() * (size_of::<f64>() + size_of::<bool>())
    }

    /// Bytes this implementation really holds.
    pub fn allocated_bytes(&self) -> usize {
        self.cells.len()
            + self.coarse.len() * size_of::<f64>()
            + self.resolved.len() * size_of::<bool>()
    }

    /// The entire state reduced to one number.
    ///
    /// Every field is folded in, densities by their exact bit pattern rather
    /// than by any rounded decimal, so two worlds agreeing here agree
    /// completely. Folding uses this crate's own generator rather than a
    /// standard-library hasher, for the same reason the generator is
    /// hand-written at all: `DefaultHasher` makes no promise of stability
    /// across versions or platforms, and this number's whole job is to be
    /// compared across platforms.
    pub fn fingerprint(&self) -> u64 {
        let mut acc = Rng::derive(
            0x554E_4956_4552_5345,
            self.geom.w as u64,
            self.geom.h as u64,
            self.geom.block as u64,
        );
        let mut h = acc.next_u64();
        for (i, c) in self.cells.iter().enumerate() {
            h = Rng::derive(h, *c as u64, i as u64, 1).next_u64();
        }
        for (i, d) in self.coarse.iter().enumerate() {
            h = Rng::derive(h, d.to_bits(), i as u64, 2).next_u64();
        }
        for (i, r) in self.resolved.iter().enumerate() {
            h = Rng::derive(h, u64::from(*r), i as u64, 3).next_u64();
        }
        acc = Rng::new(h);
        acc.next_u64()
    }
}

/// Mean absolute difference between two macro fields.
///
/// Zero means two universes are indistinguishable to an observer working at
/// the logging threshold, however different their internals were.
pub fn macro_divergence(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len(), "macro fields must share a resolution");
    if a.is_empty() {
        return 0.0;
    }
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).abs())
        .sum::<f64>()
        / a.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geom() -> Geometry {
        Geometry::new(32, 32, 1, 8)
    }

    #[test]
    fn geometry_partitions_the_grid() {
        let g = geom();
        assert_eq!(g.cells(), 1024);
        assert_eq!(g.blocks(), 16);
        let total: usize = (0..g.blocks()).map(|b| g.block_cells(b)).sum();
        assert_eq!(total, g.cells(), "blocks must tile the grid exactly");
    }

    #[test]
    fn ragged_blocks_still_tile() {
        // 30 does not divide by 8; the partition must still cover every cell
        // exactly once.
        let g = Geometry::new(30, 30, 1, 8);
        let total: usize = (0..g.blocks()).map(|b| g.block_cells(b)).sum();
        assert_eq!(total, g.cells());
    }

    #[test]
    fn block_of_agrees_with_block_bounds() {
        let g = Geometry::new(30, 30, 1, 8);
        for y in 0..g.h {
            for x in 0..g.w {
                let b = g.block_of(x, y);
                let (x0, y0, x1, y1) = g.block_bounds(b);
                assert!(x >= x0 && x < x1 && y >= y0 && y < y1);
            }
        }
    }

    #[test]
    fn wrap_makes_a_torus() {
        let g = geom();
        assert_eq!(g.wrap_x(-1), g.w - 1);
        assert_eq!(g.wrap_y(-1), g.h - 1);
        assert_eq!(g.wrap_x(g.w as isize), 0);
    }

    #[test]
    fn seeding_is_deterministic() {
        let a = World::seed(geom(), 7, 0.3);
        let b = World::seed(geom(), 7, 0.3);
        assert_eq!(a.cells, b.cells);
    }

    #[test]
    fn subdivision_preserves_the_macro_configuration() {
        // The fairness guarantee for the resolution comparison.
        let coarse = World::seed(Geometry::new(32, 32, 1, 8), 7, 0.3);
        let fine = World::seed(Geometry::new(32, 32, 2, 16), 7, 0.3);
        let d = macro_divergence(&coarse.macro_field(8), &fine.macro_field(8));
        assert!(d < 1e-12, "upsampled world should start identical, got {d}");
    }

    #[test]
    fn unresolved_blocks_are_sampled_as_density() {
        let mut w = World::seed(geom(), 7, 0.5);
        w.resolved[0] = false;
        w.coarse[0] = 0.25;
        assert_eq!(w.sample(0, 0), 0.25);
        // A resolved block still reads its cell.
        let b = w.geom.block_of(31, 31);
        assert!(w.resolved[b]);
        assert_eq!(w.sample(31, 31), w.cells[w.geom.idx(31, 31)] as f64);
    }

    #[test]
    fn live_state_bytes_drop_when_blocks_collapse() {
        let mut w = World::seed(geom(), 7, 0.5);
        let full = w.live_state_bytes();
        for b in 0..w.geom.blocks() {
            w.resolved[b] = false;
        }
        assert!(w.live_state_bytes() < full);
        assert_eq!(
            w.allocated_bytes(),
            World::seed(geom(), 7, 0.5).allocated_bytes()
        );
    }

    #[test]
    fn divergence_is_zero_for_identical_fields() {
        let w = World::seed(geom(), 7, 0.3);
        assert_eq!(macro_divergence(&w.macro_field(4), &w.macro_field(4)), 0.0);
    }
}
