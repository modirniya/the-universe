//! The laws. Pure functions: state in, state out, no side effects.
//!
//! Nothing in this module allocates a logger, reads a clock, touches the
//! filesystem or mutates its input. That is not decoration. It is what lets
//! the same law run at four resolutions and two fidelities and still be the
//! same law, and it is why the laws are the part of this codebase that is
//! actually tested.
//!
//! The rule is stated as a *density band* rather than a neighbour count, so
//! that it survives a change of resolution. At radius 1 on a Moore
//! neighbourhood the default bands reduce exactly to Conway's B3/S23 — see
//! `tests` below, which check a blinker and a block. At larger radii the same
//! bands describe the same law over a bigger neighbourhood.
//!
//! Falsified within the model if: the bands stop reducing to B3/S23 at radius
//! 1, or a step mutates its input world.

use crate::constraints::{CoarseRule, Resolved};
use crate::space::World;
use serde::Deserialize;

/// A life-like rule written in densities instead of counts.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    pub birth_lo: f64,
    pub birth_hi: f64,
    pub survive_lo: f64,
    pub survive_hi: f64,
}

impl Default for Rules {
    /// Conway B3/S23, expressed so that only 3/8 falls in the birth band and
    /// only 2/8 and 3/8 fall in the survival band.
    fn default() -> Self {
        Rules {
            birth_lo: 0.3125,
            birth_hi: 0.4375,
            survive_lo: 0.1875,
            survive_hi: 0.4375,
        }
    }
}

impl Rules {
    #[inline]
    pub fn born(&self, d: f64) -> bool {
        d >= self.birth_lo && d <= self.birth_hi
    }

    #[inline]
    pub fn survives(&self, d: f64) -> bool {
        d >= self.survive_lo && d <= self.survive_hi
    }

    /// The law, as one branch. `alive` is the cell's own state; `d` is the
    /// mean occupancy of its neighbourhood.
    #[inline]
    pub fn next(&self, alive: bool, d: f64) -> bool {
        if alive {
            self.survives(d)
        } else {
            self.born(d)
        }
    }
}

/// What one step cost, in units that do not depend on the machine.
///
/// Wall time is reported elsewhere and is not reproducible; these counters
/// are, which makes them the honest basis for any claim about cost.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Work {
    /// Cells updated at full fidelity.
    pub cell_updates: u64,
    /// Blocks updated as a single density.
    pub block_updates: u64,
    /// Neighbour samples taken. The dominant real cost.
    pub neighbor_visits: u64,
    /// Cells written by an observation forcing a region into existence.
    pub cells_rendered: u64,
}

impl Work {
    pub fn add(&mut self, o: Work) {
        self.cell_updates += o.cell_updates;
        self.block_updates += o.block_updates;
        self.neighbor_visits += o.neighbor_visits;
        self.cells_rendered += o.cells_rendered;
    }
}

/// Mean occupancy around a cell, excluding the cell itself.
///
/// Reads through [`World::sample`], so a neighbour lying in an unresolved
/// block contributes that block's density instead of a cell. The speed cap is
/// the radius: it is the only thing bounding how far influence reaches in one
/// substep.
pub fn neighborhood_density(w: &World, x: usize, y: usize, radius: usize) -> (f64, u64) {
    let r = radius as isize;
    let mut sum = 0.0;
    let mut n = 0u64;
    for dy in -r..=r {
        for dx in -r..=r {
            if dx == 0 && dy == 0 {
                continue;
            }
            let nx = w.geom.wrap_x(x as isize + dx);
            let ny = w.geom.wrap_y(y as isize + dy);
            sum += w.sample(nx, ny);
            n += 1;
        }
    }
    if n == 0 {
        (0.0, 0)
    } else {
        (sum / n as f64, n)
    }
}

/// Mean density of the blocks around a block, excluding itself.
fn block_neighborhood_density(w: &World, b: usize) -> (f64, u64) {
    let bw = w.geom.bw as isize;
    let bh = w.geom.bh as isize;
    let bx = (b % w.geom.bw) as isize;
    let by = (b / w.geom.bw) as isize;
    let mut sum = 0.0;
    let mut n = 0u64;
    for dy in -1..=1isize {
        for dx in -1..=1isize {
            if dx == 0 && dy == 0 {
                continue;
            }
            let nx = (bx + dx).rem_euclid(bw) as usize;
            let ny = (by + dy).rem_euclid(bh) as usize;
            sum += w.coarse[ny * w.geom.bw + nx];
            n += 1;
        }
    }
    if n == 0 {
        (0.0, 0)
    } else {
        (sum / n as f64, n)
    }
}

/// Share of a cell's neighbour reads that land inside its own block, for a
/// block `edge` cells wide and a neighbourhood of this `radius`.
///
/// Averaged over every cell of the block along one axis and squared for two.
/// The cell's own exclusion from its neighbourhood is ignored, which
/// overstates the share by under `1 / (2r+1)^2`. Integer arithmetic and one
/// division, so it is the same number on every target.
pub fn own_share(edge: usize, radius: usize) -> f64 {
    if edge == 0 {
        return 0.0;
    }
    let span = 2 * radius + 1;
    let inside: usize = (0..edge)
        .map(|i| {
            let lo = i.saturating_sub(radius);
            let hi = (i + radius).min(edge - 1);
            hi - lo + 1
        })
        .sum();
    let axis = inside as f64 / (edge * span) as f64;
    axis * axis
}

/// Probability that a binomial count of `n` independent neighbours, each alive
/// with probability `p`, has a density inside the band `[lo, hi]`.
///
/// Written with multiplications and one running binomial coefficient rather
/// than `powf` or a log-gamma, because transcendental functions are not
/// guaranteed bit-identical across targets and this number feeds the golden
/// fingerprint.
pub fn band_probability(n: u64, p: f64, lo: f64, hi: f64) -> f64 {
    if n == 0 {
        return 0.0;
    }
    let p = p.clamp(0.0, 1.0);
    let q = 1.0 - p;
    // p^k and q^(n-k) by repeated multiplication.
    let mut pk = vec![1.0f64; n as usize + 1];
    let mut qk = vec![1.0f64; n as usize + 1];
    for k in 1..=n as usize {
        pk[k] = pk[k - 1] * p;
        qk[k] = qk[k - 1] * q;
    }
    let mut total = 0.0;
    let mut choose = 1.0f64;
    for k in 0..=n {
        if k > 0 {
            choose = choose * (n - k + 1) as f64 / k as f64;
        }
        let d = k as f64 / n as f64;
        if d >= lo && d <= hi {
            total += choose * pk[k as usize] * qk[(n - k) as usize];
        }
    }
    total.clamp(0.0, 1.0)
}

/// Expected next density of a block under the binomial mean field.
///
/// `d` is the block's own density, `nd` the mean of its neighbouring blocks,
/// and `share` the fraction of neighbour reads that stay inside the block, so
/// the effective neighbour density is a weighted mix of the two.
pub fn binomial_next(rules: &Rules, d: f64, nd: f64, share: f64, neighbours: u64) -> f64 {
    let p = share * d + (1.0 - share) * nd;
    let born = band_probability(neighbours, p, rules.birth_lo, rules.birth_hi);
    let survives = band_probability(neighbours, p, rules.survive_lo, rules.survive_hi);
    ((1.0 - d) * born + d * survives).clamp(0.0, 1.0)
}

/// The shipped v0.1–v0.9 closure: the rule's indicator applied to the
/// neighbouring blocks' mean. See [`CoarseRule::Indicator`] for why it is
/// kept and why it is no longer the default.
pub fn indicator_next(rules: &Rules, d: f64, nd: f64) -> f64 {
    let mut nd_new = 0.0;
    if rules.born(nd) {
        nd_new += 1.0 - d;
    }
    if rules.survives(nd) {
        nd_new += d;
    }
    nd_new.clamp(0.0, 1.0)
}

/// One substep of physics over the whole world.
///
/// Resolved blocks advance cell by cell. Unresolved blocks advance as a single
/// density under the closure [`Resolved::coarse_rule`] names — a binomial mean
/// field by default, the indicator closure the first nine milestones used, or
/// a frozen density. Which closure stands in for unobserved ground is an
/// assumption, and the Theory 1 experiment runs every lazy setting under all
/// three so that no finding about lazy rendering rests on one of them unseen.
pub fn step(w: &World, rules: &Rules, res: &Resolved) -> (World, Work) {
    let mut next = w.clone();
    let mut work = Work::default();
    let share = own_share(w.geom.block, res.radius);
    let neighbours = res.neighbours();

    for b in 0..w.geom.blocks() {
        if w.resolved[b] {
            let (x0, y0, x1, y1) = w.geom.block_bounds(b);
            for y in y0..y1 {
                for x in x0..x1 {
                    let (d, visits) = neighborhood_density(w, x, y, res.radius);
                    let alive = w.cells[w.geom.idx(x, y)] == 1;
                    next.cells[w.geom.idx(x, y)] = u8::from(rules.next(alive, d));
                    work.cell_updates += 1;
                    work.neighbor_visits += visits;
                }
            }
            next.coarse[b] = next.density_from_cells(b);
        } else {
            let d = w.coarse[b];
            work.block_updates += 1;
            next.coarse[b] = match res.coarse_rule {
                CoarseRule::Frozen => d,
                CoarseRule::Indicator => {
                    let (nd, visits) = block_neighborhood_density(w, b);
                    work.neighbor_visits += visits;
                    indicator_next(rules, d, nd)
                }
                CoarseRule::Binomial => {
                    let (nd, visits) = block_neighborhood_density(w, b);
                    work.neighbor_visits += visits;
                    binomial_next(rules, d, nd, share, neighbours)
                }
            };
        }
    }

    (next, work)
}

/// A whole tick: [`Resolved::substeps`] substeps of [`step`].
///
/// With discrete time in force this is exactly one substep. Relaxing it buys
/// finer temporal resolution and pays for it linearly — and, as noted on
/// [`Resolved`], raises the distance influence covers per tick unless space is
/// refined to match.
pub fn tick(w: &World, rules: &Rules, res: &Resolved) -> (World, Work) {
    let mut cur = w.clone();
    let mut work = Work::default();
    for _ in 0..res.substeps.max(1) {
        let (next, sub) = step(&cur, rules, res);
        cur = next;
        work.add(sub);
    }
    (cur, work)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::space::Geometry;

    fn full_res(radius: usize) -> Resolved {
        Resolved {
            subdivision: 1,
            substeps: 1,
            radius,
            block_size: 8,
            lazy: false,
            coarse_rule: CoarseRule::Binomial,
        }
    }

    /// Build a fully resolved world from an explicit cell pattern.
    fn world_from(w: usize, h: usize, live: &[(usize, usize)]) -> World {
        let geom = Geometry::new(w, h, 1, 8);
        let mut world = World::seed(geom, 0, 0.0);
        world.cells.iter_mut().for_each(|c| *c = 0);
        for (x, y) in live {
            let i = world.geom.idx(*x, *y);
            world.cells[i] = 1;
        }
        world.sync_coarse_from_cells();
        world
    }

    fn live_cells(w: &World) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for y in 0..w.geom.h {
            for x in 0..w.geom.w {
                if w.cells[w.geom.idx(x, y)] == 1 {
                    out.push((x, y));
                }
            }
        }
        out.sort_unstable();
        out
    }

    #[test]
    fn default_bands_are_conway() {
        let r = Rules::default();
        // Birth on exactly 3 of 8 neighbours.
        for k in 0..=8 {
            let d = k as f64 / 8.0;
            assert_eq!(r.born(d), k == 3, "birth at {k}/8");
            assert_eq!(r.survives(d), k == 2 || k == 3, "survival at {k}/8");
        }
    }

    #[test]
    fn blinker_oscillates_with_period_two() {
        let w = world_from(16, 16, &[(5, 4), (5, 5), (5, 6)]);
        let res = full_res(1);
        let rules = Rules::default();

        let (a, _) = step(&w, &rules, &res);
        assert_eq!(live_cells(&a), vec![(4, 5), (5, 5), (6, 5)], "horizontal");

        let (b, _) = step(&a, &rules, &res);
        assert_eq!(
            live_cells(&b),
            vec![(5, 4), (5, 5), (5, 6)],
            "back to vertical"
        );
    }

    #[test]
    fn block_is_a_still_life() {
        let cells = [(4, 4), (4, 5), (5, 4), (5, 5)];
        let w = world_from(16, 16, &cells);
        let (next, _) = step(&w, &Rules::default(), &full_res(1));
        assert_eq!(live_cells(&next), cells.to_vec());
    }

    #[test]
    fn empty_space_stays_empty() {
        let w = world_from(16, 16, &[]);
        let (next, _) = step(&w, &Rules::default(), &full_res(1));
        assert!(live_cells(&next).is_empty());
    }

    #[test]
    fn glider_returns_to_its_shape_translated() {
        let w = world_from(16, 16, &[(1, 0), (2, 1), (0, 2), (1, 2), (2, 2)]);
        let rules = Rules::default();
        let res = full_res(1);
        let mut cur = w.clone();
        for _ in 0..4 {
            cur = step(&cur, &rules, &res).0;
        }
        let start = live_cells(&w);
        let end = live_cells(&cur);
        let shifted: Vec<_> = start.iter().map(|(x, y)| (x + 1, y + 1)).collect();
        assert_eq!(
            end, shifted,
            "a glider should move one cell diagonally per 4 steps"
        );
    }

    #[test]
    fn step_does_not_mutate_its_input() {
        // The purity claim, checked rather than asserted in a comment.
        let w = world_from(16, 16, &[(5, 4), (5, 5), (5, 6)]);
        let before = w.clone();
        let _ = step(&w, &Rules::default(), &full_res(1));
        assert_eq!(w.cells, before.cells);
        assert_eq!(w.coarse, before.coarse);
        assert_eq!(w.resolved, before.resolved);
    }

    #[test]
    fn step_is_deterministic() {
        let w = world_from(32, 32, &[(5, 4), (5, 5), (5, 6), (10, 10), (11, 10)]);
        let a = step(&w, &Rules::default(), &full_res(1)).0;
        let b = step(&w, &Rules::default(), &full_res(1)).0;
        assert_eq!(a.cells, b.cells);
    }

    #[test]
    fn wider_radius_costs_more_neighbour_visits() {
        let w = world_from(32, 32, &[(5, 5)]);
        let rules = Rules::default();
        let (_, cheap) = step(&w, &rules, &full_res(1));
        let (_, dear) = step(&w, &rules, &full_res(3));
        assert_eq!(cheap.neighbor_visits, 32 * 32 * 8);
        assert_eq!(dear.neighbor_visits, 32 * 32 * 48);
        assert!(dear.neighbor_visits > cheap.neighbor_visits);
    }

    #[test]
    fn unresolved_blocks_cost_one_update_not_many() {
        let mut w = world_from(32, 32, &[(5, 5)]);
        for b in 0..w.geom.blocks() {
            w.resolved[b] = false;
        }
        let (_, work) = step(&w, &Rules::default(), &full_res(1));
        assert_eq!(work.cell_updates, 0);
        assert_eq!(work.block_updates, w.geom.blocks() as u64);
        assert_eq!(work.neighbor_visits, w.geom.blocks() as u64 * 8);
    }

    #[test]
    fn a_frozen_block_reads_no_neighbours_and_keeps_its_density() {
        let mut w = world_from(32, 32, &[(5, 5)]);
        for b in 0..w.geom.blocks() {
            w.resolved[b] = false;
            w.coarse[b] = 0.37;
        }
        let mut res = full_res(1);
        res.coarse_rule = CoarseRule::Frozen;
        let (next, work) = step(&w, &Rules::default(), &res);
        assert_eq!(work.neighbor_visits, 0);
        assert_eq!(work.block_updates, w.geom.blocks() as u64);
        assert!(next.coarse.iter().all(|d| *d == 0.37));
    }

    #[test]
    fn band_probability_is_a_probability_and_sums_to_one_over_the_full_band() {
        for p in [0.0, 0.1, 0.3, 0.5, 0.9, 1.0] {
            let all = band_probability(8, p, 0.0, 1.0);
            assert!((all - 1.0).abs() < 1e-12, "p={p}: {all}");
            let some = band_probability(8, p, 0.3125, 0.4375);
            assert!((0.0..=1.0).contains(&some));
        }
        // Exactly three of eight at p = 0.5 is C(8,3) / 256.
        let three = band_probability(8, 0.5, 0.3125, 0.4375);
        assert!((three - 56.0 / 256.0).abs() < 1e-12);
    }

    #[test]
    fn the_binomial_mean_field_is_smooth_where_the_indicator_jumps() {
        // The audit's complaint, pinned. Nudging the neighbour density across
        // the birth band's edge moves the indicator closure from "keep d" to
        // "saturate"; the binomial closure barely moves.
        let r = Rules::default();
        let (lo, hi) = (indicator_next(&r, 0.3, 0.31), indicator_next(&r, 0.3, 0.32));
        assert_eq!(lo, 0.3);
        assert_eq!(hi, 1.0);
        let (blo, bhi) = (
            binomial_next(&r, 0.3, 0.31, 0.8, 8),
            binomial_next(&r, 0.3, 0.32, 0.8, 8),
        );
        assert!((blo - bhi).abs() < 0.01, "{blo} vs {bhi}");
    }

    #[test]
    fn the_binomial_mean_field_of_life_has_the_known_fixed_point() {
        // The mean-field approximation of B3/S23 is known to settle near 0.37
        // rather than decaying as Life does. That it does so here is a check on
        // the arithmetic; that it differs from Life is the approximation's
        // documented failure, and is what the Theory 1 experiment measures.
        let r = Rules::default();
        let mut d = 0.3;
        for _ in 0..200 {
            d = binomial_next(&r, d, d, 1.0, 8);
        }
        assert!((0.35..0.40).contains(&d), "fixed point at {d}");
    }

    #[test]
    fn own_share_is_a_fraction_that_grows_with_the_block() {
        assert_eq!(own_share(0, 1), 0.0);
        let small = own_share(4, 1);
        let big = own_share(32, 1);
        assert!(small > 0.0 && small < big && big < 1.0, "{small} {big}");
        assert!(own_share(16, 1) > own_share(16, 3));
    }

    #[test]
    fn substeps_multiply_the_work() {
        let w = world_from(32, 32, &[(5, 4), (5, 5), (5, 6)]);
        let rules = Rules::default();
        let one = full_res(1);
        let mut two = full_res(1);
        two.substeps = 2;
        let (_, w1) = tick(&w, &rules, &one);
        let (_, w2) = tick(&w, &rules, &two);
        assert_eq!(w2.cell_updates, 2 * w1.cell_updates);
    }
}
