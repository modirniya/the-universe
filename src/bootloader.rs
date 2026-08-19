//! Theory 5: bootloader life.
//!
//! The framework's least sentimental claim. Complexity emerges, some of it
//! becomes agentic, and agents build computers — so on this reading life's
//! structural function in the chain is not to persist or to understand but to
//! be the mechanism by which a layer instantiates the layer below it. A
//! **bootloader** is any emergent pattern whose effect is to start computation
//! one layer down.
//!
//! Full artificial life is out of reach here and pretending otherwise would be
//! the dishonest version of this milestone. What is in reach is the thing a
//! bootloader has to be able to do first: **move computation somewhere it was
//! not**. A pattern that persists, stays localized, and translates across the
//! grid is transporting structure rather than merely existing — the cellular
//! automaton's version of getting something to another machine. That is what
//! this module finds, and the report is careful to call it a precondition
//! rather than an achievement.
//!
//! The measurement is component tracking: connected clusters of live cells are
//! found each tick, matched to the previous tick by proximity, and followed. A
//! track that lives long enough and travels far enough without dispersing is a
//! bootloader. Conway's glider is the canonical case, and
//! `a_glider_is_a_bootloader` checks that the detector finds one.
//!
//! Falsified within the model if: bootloaders appear everywhere in the rule
//! space, which would make them unremarkable and disconnect Theory 5 from
//! Theory 6; or if no rule produces them, which would mean the model cannot
//! boot anything and the chain in [`crate::layer`] is inert by construction.

use crate::budget::{Budget, Degradation};
use crate::config::Config;
use crate::constraints::{Constraints, Resolved};
use crate::layer::{self, LayerSpec};
use crate::observer::observe;
use crate::physics::{Rules, tick};
use crate::rng::Rng;
use crate::space::{Geometry, World};

/// Smallest cluster worth calling a structure.
pub const MIN_SIZE: usize = 3;
/// Largest. Beyond this it is a region, not a pattern going somewhere.
pub const MAX_SIZE: usize = 40;
/// How close two centroids must be to count as the same thing, one tick on.
pub const MATCH_RADIUS: f64 = 3.0;
/// How long a track must survive to be a bootloader.
pub const MIN_LIFETIME: u64 = 20;
/// How far it must travel, in cells.
pub const MIN_DISPLACEMENT: f64 = 4.0;

/// A cluster of live cells at one instant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cluster {
    pub cx: f64,
    pub cy: f64,
    pub size: usize,
}

/// One structure followed through time.
#[derive(Clone, Debug)]
pub struct Track {
    pub first_tick: u64,
    pub last_tick: u64,
    /// Distance travelled, accumulated step by step so that wrapping around the
    /// torus counts as movement rather than as a jump back to the start.
    pub displacement: f64,
    pub mean_size: f64,
    last: Cluster,
    steps: u64,
    size_total: f64,
}

impl Track {
    pub fn lifetime(&self) -> u64 {
        self.last_tick.saturating_sub(self.first_tick) + 1
    }

    /// Whether this track transported structure across the grid.
    pub fn is_bootloader(&self) -> bool {
        self.lifetime() >= MIN_LIFETIME
            && self.displacement >= MIN_DISPLACEMENT
            && (MIN_SIZE..=MAX_SIZE).contains(&(self.mean_size.round() as usize))
    }
}

/// What one universe produced.
#[derive(Clone, Debug, PartialEq)]
pub struct BootSurvey {
    pub tracks: usize,
    pub bootloaders: usize,
    /// Total distance travelled by bootloading structures, in cells.
    pub transport: f64,
    /// Longest-lived bootloader, in ticks.
    pub longest_lifetime: u64,
}

impl BootSurvey {
    /// Whether this universe can boot anything at all.
    pub fn can_boot(&self) -> bool {
        self.bootloaders > 0
    }
}

/// Connected clusters of live cells, 8-connected, on the torus.
///
/// Only resolved ground is searched: a coarse-grained region has no cells to
/// belong to a cluster, which is the honest reading of what it contains.
pub fn clusters(w: &World) -> Vec<Cluster> {
    let geom = &w.geom;
    let n = geom.cells();
    let mut seen = vec![false; n];
    let mut out = Vec::new();
    let mut stack: Vec<(usize, usize)> = Vec::new();

    for y0 in 0..geom.h {
        for x0 in 0..geom.w {
            let start = geom.idx(x0, y0);
            if seen[start] || !live_at(w, x0, y0) {
                continue;
            }

            // Accumulate positions relative to the seed so a cluster straddling
            // the seam has a sensible centroid instead of one in the middle of
            // the world.
            let mut sum_dx = 0.0;
            let mut sum_dy = 0.0;
            let mut size = 0usize;

            stack.push((x0, y0));
            seen[start] = true;

            while let Some((x, y)) = stack.pop() {
                let dx = wrapped_delta(x as f64 - x0 as f64, geom.w as f64);
                let dy = wrapped_delta(y as f64 - y0 as f64, geom.h as f64);
                sum_dx += dx;
                sum_dy += dy;
                size += 1;

                for oy in -1..=1isize {
                    for ox in -1..=1isize {
                        if ox == 0 && oy == 0 {
                            continue;
                        }
                        let nx = geom.wrap_x(x as isize + ox);
                        let ny = geom.wrap_y(y as isize + oy);
                        let ni = geom.idx(nx, ny);
                        if !seen[ni] && live_at(w, nx, ny) {
                            seen[ni] = true;
                            stack.push((nx, ny));
                        }
                    }
                }
            }

            if (MIN_SIZE..=MAX_SIZE).contains(&size) {
                let k = size as f64;
                out.push(Cluster {
                    cx: (x0 as f64 + sum_dx / k).rem_euclid(geom.w as f64),
                    cy: (y0 as f64 + sum_dy / k).rem_euclid(geom.h as f64),
                    size,
                });
            }
        }
    }

    out
}

fn live_at(w: &World, x: usize, y: usize) -> bool {
    let b = w.geom.block_of(x, y);
    w.resolved[b] && w.cells[w.geom.idx(x, y)] == 1
}

/// Shortest signed distance on a circle.
fn wrapped_delta(d: f64, n: f64) -> f64 {
    let mut d = d % n;
    if d > n / 2.0 {
        d -= n;
    } else if d < -n / 2.0 {
        d += n;
    }
    d
}

fn toroidal_distance(a: &Cluster, b: &Cluster, w: f64, h: f64) -> f64 {
    let dx = wrapped_delta(a.cx - b.cx, w);
    let dy = wrapped_delta(a.cy - b.cy, h);
    (dx * dx + dy * dy).sqrt()
}

/// Follow clusters through a run and report what travelled.
pub fn survey(cfg: &Config, rules: &Rules, constraints: Constraints) -> BootSurvey {
    let res = Resolved::new(&constraints, &cfg.params);
    let geom = Geometry::new(
        cfg.world.width,
        cfg.world.height,
        res.subdivision,
        res.block_size,
    );
    let (fw, fh) = (geom.w as f64, geom.h as f64);
    let mut world = World::seed(geom, cfg.world.seed, cfg.world.init_density);

    let mut open: Vec<Track> = Vec::new();
    let mut done: Vec<Track> = Vec::new();

    for t in 0..cfg.world.ticks {
        let (observed, _) = observe(&world, &cfg.observer, t, cfg.world.seed, res.lazy);
        let (advanced, _) = tick(&observed, rules, &res);

        let found = clusters(&advanced);
        let mut claimed = vec![false; found.len()];
        let mut still_open: Vec<Track> = Vec::new();

        // Extend each open track with its nearest unclaimed cluster.
        for mut track in open.drain(..) {
            let best = found
                .iter()
                .enumerate()
                .filter(|(i, _)| !claimed[*i])
                .map(|(i, c)| (i, toroidal_distance(&track.last, c, fw, fh)))
                .filter(|(_, d)| *d <= MATCH_RADIUS)
                .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

            match best {
                Some((i, d)) => {
                    claimed[i] = true;
                    track.displacement += d;
                    track.last = found[i];
                    track.last_tick = t;
                    track.steps += 1;
                    track.size_total += found[i].size as f64;
                    track.mean_size = track.size_total / track.steps as f64;
                    still_open.push(track);
                }
                None => done.push(track),
            }
        }

        // Anything unclaimed starts a new track.
        for (i, c) in found.iter().enumerate() {
            if !claimed[i] {
                still_open.push(Track {
                    first_tick: t,
                    last_tick: t,
                    displacement: 0.0,
                    mean_size: c.size as f64,
                    last: *c,
                    steps: 1,
                    size_total: c.size as f64,
                });
            }
        }

        open = still_open;
        world = advanced;
    }

    done.extend(open);

    let boots: Vec<&Track> = done.iter().filter(|t| t.is_bootloader()).collect();
    BootSurvey {
        tracks: done.len(),
        bootloaders: boots.len(),
        // `+ 0.0` normalises negative zero. Rust folds float sums from -0.0,
        // because that is the true additive identity, so a universe that
        // transported nothing reports "-0.0" without this.
        transport: boots.iter().map(|t| t.displacement).sum::<f64>() + 0.0,
        longest_lifetime: boots.iter().map(|t| t.lifetime()).max().unwrap_or(0),
    }
}

/// Derive a child universe's seed from what crossed the horizon.
///
/// This is the loop the whole framework describes, closed. A layer's emergent
/// structures drive what crosses its horizon; what crosses is all the next
/// layer ever receives; and that is what its universe is seeded from. The child
/// is booted from inside the parent, through a channel neither end can see
/// through.
///
/// Returns `None` when nothing cleared the logging threshold. A layer that
/// produces nothing worth transmitting boots nothing, and the chain stops —
/// which is a second limit on depth, independent of the budget in
/// [`crate::budget`].
pub fn boot_seed(messages: &[crate::pipe::Message]) -> Option<u64> {
    if messages.is_empty() {
        return None;
    }
    let mut acc = Rng::new(0x1F1E_B00D_10AD_E123);
    for m in messages {
        acc = Rng::derive(acc.next_u64(), m.tick, m.digest, m.magnitude.to_bits());
    }
    Some(acc.next_u64())
}

/// One layer of a chain booted from inside.
#[derive(Clone, Debug)]
pub struct BootLayer {
    pub depth: usize,
    pub spec: LayerSpec,
    pub budget: Budget,
    /// What this universe was seeded from. Layer 1 gets the creator's seed;
    /// every layer below gets whatever its parent managed to push through the
    /// horizon.
    pub seed: u64,
    pub survey: BootSurvey,
    /// Messages that cleared the logging threshold.
    pub crossed: usize,
    pub booted_child: bool,
}

/// A chain in which every layer is booted by the one above it.
#[derive(Clone, Debug)]
pub struct BootChain {
    pub layers: Vec<BootLayer>,
    /// Why it stopped. The interesting part: a chain can run out of money or
    /// run out of life, and which comes first is not decided in advance.
    pub ended_because: &'static str,
}

impl BootChain {
    pub fn depth(&self) -> usize {
        self.layers.len()
    }
}

/// Build a chain where each layer is seeded by what crossed its parent's
/// horizon.
///
/// This is the whole framework closed into a loop. A layer's emergent
/// structures drive its activity; that activity is what crosses the horizon;
/// what crosses is all the child ever receives; and the child's universe is
/// seeded from it. Nobody on either side of the pipe can see through it, and
/// the child is booted anyway.
///
/// The chain ends for whichever reason arrives first — the budget falls below
/// what a universe costs, no world small enough remains viable, nothing clears
/// the logging threshold, or the layer simply produces no bootloaders. The last
/// of these is Theory 5's own limit on depth, and it is independent of the
/// budget limit in [`crate::budget`].
pub fn run_boot_chain(
    cfg: &Config,
    root_budget: Budget,
    deg: &Degradation,
    mut on_layer: impl FnMut(usize, &LayerSpec),
) -> BootChain {
    let root = LayerSpec {
        width: cfg.world.width,
        height: cfg.world.height,
        ticks: cfg.world.ticks,
    };

    let mut layers: Vec<BootLayer> = Vec::new();
    let mut budget = root_budget;
    let mut seed = cfg.world.seed;
    let mut max_edge = root.height;
    let mut depth = 1;

    let ended_because = loop {
        let Some(spec) = layer::fit_spec(budget, &root, max_edge, deg, cfg) else {
            break "no world small enough was still viable";
        };

        on_layer(depth, &spec);

        let mut layer_cfg = cfg.clone();
        layer_cfg.world.width = spec.width;
        layer_cfg.world.height = spec.height;
        layer_cfg.world.ticks = spec.ticks;
        layer_cfg.world.seed = seed;
        layer_cfg.observer = layer::scale_probe(&cfg.observer, &root, &spec);

        // Every layer runs with the limits in force: a universe on a fraction
        // of its host's resources takes every optimization going.
        let constraints = Constraints::ALL_ON;
        let survey = survey(&layer_cfg, &layer_cfg.rules, constraints);

        // What this layer pushed through its own horizon.
        let horizon = scale_horizon(&cfg.horizon, &root, &spec);
        let relay = crate::pipe::run_relay(&layer_cfg, &horizon);
        let crossed = relay.received.above(cfg.horizon.threshold);

        let child_seed = if survey.can_boot() {
            boot_seed(&crossed)
        } else {
            // A layer with no bootloaders transports nothing, whatever its
            // horizon happened to register.
            None
        };

        layers.push(BootLayer {
            depth,
            spec,
            budget,
            seed,
            survey: survey.clone(),
            crossed: crossed.len(),
            booted_child: child_seed.is_some(),
        });

        let Some(next_seed) = child_seed else {
            break if survey.can_boot() {
                "nothing cleared the logging threshold, so no seed reached the next layer"
            } else {
                "the layer produced no bootloader, so there was nothing to boot with"
            };
        };

        let Some(next_budget) = deg.child_of(budget) else {
            break "the budget fell below what a universe costs";
        };

        seed = next_seed;
        budget = next_budget;
        max_edge = spec.height;
        depth += 1;
    };

    BootChain {
        layers,
        ended_because,
    }
}

/// Keep the horizon covering the same share of a smaller world.
fn scale_horizon(
    h: &crate::pipe::Horizon,
    root: &LayerSpec,
    spec: &LayerSpec,
) -> crate::pipe::Horizon {
    let sx = spec.width as f64 / root.width as f64;
    let sy = spec.height as f64 / root.height as f64;
    crate::pipe::Horizon {
        x: ((h.x as f64 * sx).round() as usize).min(spec.width.saturating_sub(1)),
        y: ((h.y as f64 * sy).round() as usize).min(spec.height.saturating_sub(1)),
        width: ((h.width as f64 * sx).round() as usize).clamp(1, spec.width),
        height: ((h.height as f64 * sy).round() as usize).clamp(1, spec.height),
        threshold: h.threshold,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::Degradation;
    use crate::config::{ReportCfg, WorldCfg};
    use crate::constraints::Params;
    use crate::observer::Probe;
    use crate::pipe::{Horizon, Message};

    fn cfg() -> Config {
        Config {
            world: WorldCfg {
                width: 64,
                height: 64,
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
                width: 64,
                height: 64,
            },
            report: ReportCfg {
                macro_grid: 16,
                out_dir: "out".into(),
            },
            nesting: Degradation::default(),
            horizon: Horizon::default(),
        }
    }

    fn world_with(live: &[(usize, usize)]) -> World {
        let geom = Geometry::new(32, 32, 1, 16);
        let mut w = World::seed(geom, 0, 0.0);
        w.cells.iter_mut().for_each(|c| *c = 0);
        for (x, y) in live {
            let i = w.geom.idx(*x, *y);
            w.cells[i] = 1;
        }
        w.sync_coarse_from_cells();
        w
    }

    #[test]
    fn a_lone_cluster_is_found_with_its_centroid() {
        let w = world_with(&[(10, 10), (11, 10), (10, 11)]);
        let cs = clusters(&w);
        assert_eq!(cs.len(), 1);
        assert_eq!(cs[0].size, 3);
        assert!((cs[0].cx - 10.333).abs() < 0.01, "cx was {}", cs[0].cx);
        assert!((cs[0].cy - 10.333).abs() < 0.01, "cy was {}", cs[0].cy);
    }

    #[test]
    fn separate_clusters_are_not_merged() {
        let w = world_with(&[(5, 5), (6, 5), (5, 6), (20, 20), (21, 20), (20, 21)]);
        assert_eq!(clusters(&w).len(), 2);
    }

    #[test]
    fn a_cluster_straddling_the_seam_has_a_sensible_centroid() {
        // Cells at x = 31, 0, 1 are adjacent on a torus. A naive mean would put
        // the centroid at x = 10.7, in the middle of empty space.
        let w = world_with(&[(31, 10), (0, 10), (1, 10)]);
        let cs = clusters(&w);
        assert_eq!(cs.len(), 1, "the seam must not split a cluster");
        let cx = cs[0].cx;
        assert!(
            !(1.5..=30.5).contains(&cx),
            "centroid should sit near the seam, got {cx}"
        );
    }

    #[test]
    fn oversized_regions_are_not_structures() {
        let big: Vec<(usize, usize)> = (0..10).flat_map(|y| (0..10).map(move |x| (x, y))).collect();
        assert!(clusters(&world_with(&big)).is_empty());
    }

    #[test]
    fn a_glider_is_a_bootloader() {
        // The canonical case. A Life glider persists, stays small, and travels
        // one cell diagonally every four ticks -- structure transported to
        // somewhere it was not.
        let mut c = cfg();
        c.world.init_density = 0.0;
        c.world.ticks = 80;

        let res = Resolved::new(&Constraints::ALL_ON, &c.params);
        let geom = Geometry::new(
            c.world.width,
            c.world.height,
            res.subdivision,
            res.block_size,
        );
        let mut w = World::seed(geom, 0, 0.0);
        w.cells.iter_mut().for_each(|v| *v = 0);
        for (x, y) in [(1, 0), (2, 1), (0, 2), (1, 2), (2, 2)] {
            let i = w.geom.idx(x, y);
            w.cells[i] = 1;
        }
        w.sync_coarse_from_cells();

        // Follow it by hand: `survey` seeds its own world, so this checks the
        // tracking logic on a world we control.
        let mut open: Option<Track> = None;
        let (fw, fh) = (w.geom.w as f64, w.geom.h as f64);
        for t in 0..c.world.ticks {
            let (next, _) = tick(&w, &c.rules, &res);
            let found = clusters(&next);
            assert_eq!(
                found.len(),
                1,
                "the glider should stay one cluster at tick {t}"
            );
            let cl = found[0];
            open = Some(match open {
                None => Track {
                    first_tick: t,
                    last_tick: t,
                    displacement: 0.0,
                    mean_size: cl.size as f64,
                    last: cl,
                    steps: 1,
                    size_total: cl.size as f64,
                },
                Some(mut tr) => {
                    tr.displacement += toroidal_distance(&tr.last, &cl, fw, fh);
                    tr.last = cl;
                    tr.last_tick = t;
                    tr.steps += 1;
                    tr.size_total += cl.size as f64;
                    tr.mean_size = tr.size_total / tr.steps as f64;
                    tr
                }
            });
            w = next;
        }

        let track = open.expect("the glider should have been tracked");
        assert!(track.lifetime() >= MIN_LIFETIME);
        assert!(
            track.displacement >= MIN_DISPLACEMENT,
            "a glider should travel: {}",
            track.displacement
        );
        assert!(track.is_bootloader());
    }

    #[test]
    fn a_still_life_is_not_a_bootloader() {
        // A block persists forever and goes nowhere. Persistence alone is not
        // transport, and the distinction is the whole point.
        let mut c = cfg();
        c.world.init_density = 0.0;
        let res = Resolved::new(&Constraints::ALL_ON, &c.params);
        let geom = Geometry::new(
            c.world.width,
            c.world.height,
            res.subdivision,
            res.block_size,
        );
        let mut w = World::seed(geom, 0, 0.0);
        w.cells.iter_mut().for_each(|v| *v = 0);
        for (x, y) in [(4, 4), (4, 5), (5, 4), (5, 5)] {
            let i = w.geom.idx(x, y);
            w.cells[i] = 1;
        }
        w.sync_coarse_from_cells();

        let mut displacement = 0.0;
        let mut last = clusters(&w)[0];
        let (fw, fh) = (w.geom.w as f64, w.geom.h as f64);
        for _ in 0..40 {
            let (next, _) = tick(&w, &c.rules, &res);
            let cl = clusters(&next)[0];
            displacement += toroidal_distance(&last, &cl, fw, fh);
            last = cl;
            w = next;
        }
        assert!(displacement < MIN_DISPLACEMENT, "a block should not travel");
    }

    #[test]
    fn a_survey_is_deterministic() {
        let c = cfg();
        let a = survey(&c, &c.rules, Constraints::ALL_ON);
        let b = survey(&c, &c.rules, Constraints::ALL_ON);
        assert_eq!(a.bootloaders, b.bootloaders);
        assert_eq!(a.transport, b.transport);
    }

    #[test]
    fn a_dead_universe_boots_nothing() {
        let mut c = cfg();
        c.world.init_density = 0.0;
        let s = survey(&c, &c.rules, Constraints::ALL_ON);
        assert_eq!(s.bootloaders, 0);
        assert!(!s.can_boot());
        assert!(
            s.transport.is_sign_positive(),
            "an empty float sum folds from -0.0 in Rust; a report should not print it"
        );
    }

    #[test]
    fn nothing_crossing_means_no_child() {
        assert!(boot_seed(&[]).is_none());
    }

    #[test]
    fn the_child_seed_depends_on_what_crossed() {
        let a = [Message {
            tick: 1,
            magnitude: 0.4,
            digest: 99,
        }];
        let b = [Message {
            tick: 1,
            magnitude: 0.4,
            digest: 100,
        }];
        assert_ne!(boot_seed(&a), boot_seed(&b));
        assert_eq!(boot_seed(&a), boot_seed(&a));
    }
}
