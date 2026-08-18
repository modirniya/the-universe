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

use crate::constraints::Resolved;
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

/// One substep of physics over the whole world.
///
/// Resolved blocks advance cell by cell. Unresolved blocks advance as a single
/// density under a mean-field version of the same rule: the expected outcome
/// if the block's occupants were spread evenly. That approximation is crude on
/// purpose — coarse graining is supposed to lose something, and how much it
/// loses is precisely what the experiment measures.
pub fn step(w: &World, rules: &Rules, res: &Resolved) -> (World, Work) {
    let mut next = w.clone();
    let mut work = Work::default();

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
            let (nd, visits) = block_neighborhood_density(w, b);
            let d = w.coarse[b];
            let mut nd_new = 0.0;
            if rules.born(nd) {
                nd_new += 1.0 - d;
            }
            if rules.survives(nd) {
                nd_new += d;
            }
            next.coarse[b] = nd_new.clamp(0.0, 1.0);
            work.block_updates += 1;
            work.neighbor_visits += visits;
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
