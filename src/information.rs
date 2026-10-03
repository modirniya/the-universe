//! What crosses the pipe, measured in bits rather than in correlation.
//!
//! The pipe (`crate::pipe`) folds a region of the child into one message per
//! tick. v0.9 measured what survives as a Pearson correlation between the
//! message's magnitude and the child's occupancy, and found that two to six
//! bits a tick keep most of it. The audit (`docs/audit.md` §3.1) pointed out
//! that both series are slowly varying aggregates of one field, so that result
//! is a property of quantising a smooth scalar, not of the mechanism; and that
//! "what information survives" has to be asked *per task* and *per encoding*
//! before it means anything.
//!
//! So this module asks it that way.
//!
//! # Encodings
//!
//! Each turns one tick of the child into one symbol the parent reads, using at
//! most the stated number of bits. [`Encoding::Uniform`] is the shipped
//! channel: the horizon's occupancy rounded onto evenly spaced levels. The
//! others are alternatives a creator could have built instead:
//!
//! - [`Encoding::Adaptive`] — levels spread over the range the horizon's
//!   occupancy actually took in the first half of the run. A representation
//!   the parent learns from the child's history.
//! - [`Encoding::Hash`] — the top bits of the position-sensitive digest, and
//!   no magnitude at all.
//! - [`Encoding::Projection`] — sign bits of random ±1 projections of the
//!   horizon's cells, centred on the horizon's mean. A sketch of the
//!   arrangement that carries no occupancy on purpose.
//! - [`Encoding::Quadrants`] — one bit per quadrant of the horizon saying
//!   whether it is above its own median density. A spatial summary.
//!
//! A parent's resolution saturates at 64 levels in every encoding, because a
//! joint table with more cells than ticks estimates nothing.
//!
//! # Tasks
//!
//! What the parent might want to know about the child, as a class per tick:
//! the horizon's own occupancy (what was sent), the whole child's occupancy
//! now and [`FUTURE_LAG`] ticks ahead, which quadrant of the horizon is
//! densest, and the four-bit pattern of which quadrants are above their
//! medians. Continuous targets are cut into eight quantile bins.
//!
//! # Measures
//!
//! For every encoding and task: the entropy of the symbol stream (the bits the
//! channel actually used), the entropy of the target, the plug-in mutual
//! information, the same under [`NULL_SHUFFLES`] permutations of time (the
//! estimator's bias), their difference, and a leave-one-out **predictive**
//! information: the bits by which knowing the symbol lowers the log-loss of
//! predicting each tick's target, with the code learned from every other tick.
//! The plug-in number says what the symbol and the target have in common; the
//! predictive number says what a parent who has to learn the code from a
//! finite history can actually use.
//!
//! # Controls
//!
//! - The shipped encoding applied to a window elsewhere in the child. If it
//!   carries as much about the whole child as the horizon does, the horizon is
//!   a window, not a mechanism.
//! - Random symbols. The estimator must read about zero.
//! - Bit flips at several rates on the shipped encoding: capacity under noise.
//!
//! # What is designed in
//!
//! That the message carries a magnitude and a hashed digest is a definition,
//! and so is the fact that the hash carries nothing about occupancy. What had
//! to be run is how many bits each task survives on under each encoding, and
//! whether the horizon does anything a window elsewhere would not.
//!
//! Falsified within the model if: no encoding carries information about any
//! task beyond the shuffle null (the pipe is noise), or the shipped encoding
//! carries information the window control does not (the mechanism would be
//! doing something the analysis had not accounted for).

use crate::config::Config;
use crate::constraints::{Constraints, Resolved};
use crate::observer::observe;
use crate::physics::tick;
use crate::pipe::{Horizon, Message, serialize};
use crate::rng::Rng;
use crate::space::{Geometry, World};
use crate::stats;

/// Ticks ahead for the future-occupancy task.
pub const FUTURE_LAG: usize = 10;
/// Quantile bins for continuous targets.
pub const TARGET_BINS: usize = 8;
/// Most levels a parent's reading can have, in any encoding.
pub const MAX_LEVELS_LOG2: u32 = 6;
/// Permutations of time used to estimate the plug-in estimator's bias.
pub const NULL_SHUFFLES: usize = 20;
/// Sign bits of random projections recorded per tick.
const PROJECTION_BITS: u32 = 8;
const PROJECTION_SEED: u64 = 0x0050_524F_4A45_4354;

/// Everything about one child run the analysis needs, tick by tick.
#[derive(Clone, Debug)]
pub struct ChildTrace {
    pub horizon_occupancy: Vec<f64>,
    pub global_occupancy: Vec<f64>,
    /// Density of each quadrant of the horizon.
    pub quadrants: Vec<[f64; 4]>,
    pub digest: Vec<u64>,
    /// Sign bits of random ±1 projections of the horizon's centred cells.
    pub projections: Vec<u32>,
    /// Occupancy of a window the same size as the horizon, half a world away.
    pub window_occupancy: Vec<f64>,
}

impl ChildTrace {
    pub fn ticks(&self) -> usize {
        self.horizon_occupancy.len()
    }
}

fn region_occupancy(w: &World, x0: usize, y0: usize, width: usize, height: usize) -> f64 {
    let mut live = 0.0;
    let mut n = 0.0;
    for row in 0..height {
        for col in 0..width {
            let x = w.geom.wrap_x((x0 + col) as isize);
            let y = w.geom.wrap_y((y0 + row) as isize);
            live += f64::from(w.sample(x, y) >= 0.5);
            n += 1.0;
        }
    }
    if n == 0.0 { 0.0 } else { live / n }
}

fn quadrants(w: &World, h: &Horizon) -> [f64; 4] {
    let (hw, hh) = (h.width / 2, h.height / 2);
    [
        region_occupancy(w, h.x, h.y, hw, hh),
        region_occupancy(w, h.x + hw, h.y, h.width - hw, hh),
        region_occupancy(w, h.x, h.y + hh, hw, h.height - hh),
        region_occupancy(w, h.x + hw, h.y + hh, h.width - hw, h.height - hh),
    ]
}

fn projections(w: &World, h: &Horizon) -> u32 {
    let mean = region_occupancy(w, h.x, h.y, h.width, h.height);
    let mut bits = 0u32;
    for k in 0..PROJECTION_BITS {
        let mut acc = 0.0;
        for row in 0..h.height {
            for col in 0..h.width {
                let x = w.geom.wrap_x((h.x + col) as isize);
                let y = w.geom.wrap_y((h.y + row) as isize);
                let bit = f64::from(w.sample(x, y) >= 0.5) - mean;
                let sign = if Rng::derive(PROJECTION_SEED, u64::from(k), col as u64, row as u64)
                    .chance(0.5)
                {
                    1.0
                } else {
                    -1.0
                };
                acc += sign * bit;
            }
        }
        if acc >= 0.0 {
            bits |= 1 << k;
        }
    }
    bits
}

/// Run the child (every limit in force, as the pipe does) and record the trace.
pub fn run_trace(cfg: &Config, horizon: &Horizon) -> ChildTrace {
    let res = Resolved::new(&Constraints::ALL_ON, &cfg.params);
    let geom = Geometry::new(
        cfg.world.width,
        cfg.world.height,
        res.subdivision,
        res.block_size,
    );
    let mut world = World::seed(geom, cfg.world.seed, cfg.world.init_density);
    let n = cfg.world.ticks as usize;
    let mut t_ = ChildTrace {
        horizon_occupancy: Vec::with_capacity(n),
        global_occupancy: Vec::with_capacity(n),
        quadrants: Vec::with_capacity(n),
        digest: Vec::with_capacity(n),
        projections: Vec::with_capacity(n),
        window_occupancy: Vec::with_capacity(n),
    };
    let far = Horizon {
        x: (horizon.x + geom.w / 2) % geom.w,
        y: (horizon.y + geom.h / 2) % geom.h,
        ..*horizon
    };
    for t in 0..cfg.world.ticks {
        let (observed, _) = observe(&world, &cfg.observer, t, cfg.world.seed, res.lazy);
        let (advanced, _) = tick(&observed, &cfg.rules, &res);
        let m = serialize(&advanced, horizon, t);
        t_.horizon_occupancy.push(m.magnitude());
        t_.digest.push(m.digest());
        t_.global_occupancy.push(advanced.live_fraction());
        t_.quadrants.push(quadrants(&advanced, horizon));
        t_.projections.push(projections(&advanced, horizon));
        t_.window_occupancy.push(region_occupancy(
            &advanced, far.x, far.y, far.width, far.height,
        ));
        world = advanced;
    }
    t_
}

// ---------------------------------------------------------------------------
// Encodings
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    /// The shipped channel: occupancy on evenly spaced levels.
    Uniform(u32),
    /// Levels over the range seen in the first half of the run.
    Adaptive(u32),
    /// Top bits of the digest, no magnitude.
    Hash(u32),
    /// Sign bits of random projections of the arrangement.
    Projection(u32),
    /// One bit per quadrant: above its own median or not.
    Quadrants,
}

/// Every encoding the analysis runs.
pub const ENCODINGS: [Encoding; 13] = [
    Encoding::Uniform(1),
    Encoding::Uniform(2),
    Encoding::Uniform(3),
    Encoding::Uniform(4),
    Encoding::Uniform(6),
    Encoding::Uniform(128),
    Encoding::Adaptive(2),
    Encoding::Adaptive(4),
    Encoding::Hash(4),
    Encoding::Hash(6),
    Encoding::Projection(4),
    Encoding::Projection(6),
    Encoding::Quadrants,
];

impl Encoding {
    pub fn label(&self) -> String {
        match self {
            Encoding::Uniform(b) => format!("uniform:{b}"),
            Encoding::Adaptive(b) => format!("adaptive:{b}"),
            Encoding::Hash(b) => format!("hash:{b}"),
            Encoding::Projection(b) => format!("projection:{b}"),
            Encoding::Quadrants => "quadrants:4".to_string(),
        }
    }

    /// Bits per tick the channel is allowed.
    pub fn bits(&self) -> u32 {
        match self {
            Encoding::Uniform(b)
            | Encoding::Adaptive(b)
            | Encoding::Hash(b)
            | Encoding::Projection(b) => *b,
            Encoding::Quadrants => 4,
        }
    }

    /// Distinct symbols the parent can read: `2^min(bits, 6)`.
    pub fn levels(&self) -> usize {
        1 << self.bits().min(MAX_LEVELS_LOG2)
    }

    /// One symbol per tick.
    pub fn symbols(&self, trace: &ChildTrace) -> Vec<u32> {
        let levels = self.levels() as f64;
        let bin = |v: f64| ((v * levels).floor() as u32).min(self.levels() as u32 - 1);
        match self {
            Encoding::Uniform(b) => trace
                .horizon_occupancy
                .iter()
                .enumerate()
                .map(|(t, m)| bin(Message::pack(t as u64, *m, 0, *b).magnitude()))
                .collect(),
            Encoding::Adaptive(b) => {
                let half = trace.ticks() / 2;
                let seen = &trace.horizon_occupancy[..half.max(1)];
                let lo = seen.iter().copied().fold(f64::INFINITY, f64::min);
                let hi = seen.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                let span = if hi > lo { hi - lo } else { 1.0 };
                let fine = (1u64 << (*b).min(52)) - 1;
                trace
                    .horizon_occupancy
                    .iter()
                    .map(|m| {
                        let u = ((m - lo) / span).clamp(0.0, 1.0);
                        // Round onto 2^b levels first, then onto what the parent reads.
                        let q = (u * fine as f64).round() / fine as f64;
                        bin(q)
                    })
                    .collect()
            }
            Encoding::Hash(b) => {
                let keep = (*b).min(MAX_LEVELS_LOG2);
                trace
                    .digest
                    .iter()
                    .map(|d| (d >> (64 - keep)) as u32)
                    .collect()
            }
            Encoding::Projection(b) => {
                let keep = (*b).min(MAX_LEVELS_LOG2).min(PROJECTION_BITS);
                trace
                    .projections
                    .iter()
                    .map(|p| p & ((1u32 << keep) - 1))
                    .collect()
            }
            Encoding::Quadrants => {
                let medians: Vec<f64> = (0..4)
                    .map(|q| {
                        let mut v: Vec<f64> = trace.quadrants.iter().map(|qs| qs[q]).collect();
                        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
                        stats::percentile(&v, 0.5)
                    })
                    .collect();
                trace
                    .quadrants
                    .iter()
                    .map(|qs| (0..4).map(|q| u32::from(qs[q] > medians[q]) << q).sum())
                    .collect()
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tasks
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Task {
    HorizonNow,
    GlobalNow,
    GlobalFuture,
    DensestQuadrant,
    QuadrantPattern,
}

pub const TASKS: [Task; 5] = [
    Task::HorizonNow,
    Task::GlobalNow,
    Task::GlobalFuture,
    Task::DensestQuadrant,
    Task::QuadrantPattern,
];

impl Task {
    pub fn label(&self) -> &'static str {
        match self {
            Task::HorizonNow => "horizon_now",
            Task::GlobalNow => "global_now",
            Task::GlobalFuture => "global_future",
            Task::DensestQuadrant => "densest_quadrant",
            Task::QuadrantPattern => "quadrant_pattern",
        }
    }

    /// The target class at every tick the task is defined for, and how many
    /// leading symbols line up with it (the future task drops the last
    /// [`FUTURE_LAG`] symbols).
    pub fn targets(&self, trace: &ChildTrace) -> Vec<u32> {
        match self {
            Task::HorizonNow => quantile_bins(&trace.horizon_occupancy, TARGET_BINS),
            Task::GlobalNow => quantile_bins(&trace.global_occupancy, TARGET_BINS),
            Task::GlobalFuture => {
                let n = trace.ticks();
                if n <= FUTURE_LAG {
                    return Vec::new();
                }
                quantile_bins(&trace.global_occupancy[FUTURE_LAG..], TARGET_BINS)
            }
            Task::DensestQuadrant => trace
                .quadrants
                .iter()
                .map(|qs| {
                    let mut best = 0;
                    for q in 1..4 {
                        if qs[q] > qs[best] {
                            best = q;
                        }
                    }
                    best as u32
                })
                .collect(),
            Task::QuadrantPattern => Encoding::Quadrants.symbols(trace),
        }
    }
}

/// Cut a series into `bins` quantile bins by rank, ties broken by position.
pub fn quantile_bins(values: &[f64], bins: usize) -> Vec<u32> {
    let n = values.len();
    if n == 0 {
        return Vec::new();
    }
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|a, b| values[*a].partial_cmp(&values[*b]).unwrap().then(a.cmp(b)));
    let mut out = vec![0u32; n];
    for (rank, i) in order.into_iter().enumerate() {
        out[i] = ((rank * bins) / n) as u32;
    }
    out
}

// ---------------------------------------------------------------------------
// Measures
// ---------------------------------------------------------------------------

/// Everything measured for one encoding on one task.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measure {
    /// Entropy of the symbol stream: bits the channel actually carried.
    pub h_symbol: f64,
    pub h_target: f64,
    /// Plug-in mutual information.
    pub mi: f64,
    /// Mean plug-in MI under permutations of time.
    pub mi_null: f64,
    /// Bits by which knowing the symbol lowers the leave-one-out log-loss of
    /// predicting the target, code learned from every other tick.
    pub predictive: f64,
    /// Ticks the measure rests on.
    pub n: usize,
}

impl Measure {
    /// Plug-in MI less its shuffle null.
    pub fn mi_corrected(&self) -> f64 {
        self.mi - self.mi_null
    }
}

fn entropy(xs: &[u32]) -> f64 {
    let max = xs.iter().copied().max().map_or(0, |m| m as usize + 1);
    let mut counts = vec![0usize; max];
    for x in xs {
        counts[*x as usize] += 1;
    }
    stats::entropy_bits(&counts)
}

fn plug_in_mi(symbols: &[u32], targets: &[u32]) -> f64 {
    let n = symbols.len().min(targets.len());
    if n == 0 {
        return 0.0;
    }
    let (s, t) = (&symbols[..n], &targets[..n]);
    let joint: Vec<u32> = s
        .iter()
        .zip(t)
        .map(|(a, b)| a * (t.iter().max().unwrap() + 1) + b)
        .collect();
    entropy(s) + entropy(t) - entropy(&joint)
}

/// Measure one symbol stream against one target stream.
pub fn measure(symbols: &[u32], targets: &[u32], seed: u64) -> Measure {
    let n = symbols.len().min(targets.len());
    if n < 4 {
        return Measure {
            h_symbol: f64::NAN,
            h_target: f64::NAN,
            mi: f64::NAN,
            mi_null: f64::NAN,
            predictive: f64::NAN,
            n,
        };
    }
    let (s, t) = (&symbols[..n], &targets[..n]);
    let mi = plug_in_mi(s, t);
    let mut null = 0.0;
    for k in 0..NULL_SHUFFLES {
        let mut shuffled = t.to_vec();
        let mut rng = Rng::derive(seed, k as u64, n as u64, 0x4D49_4E55_4C4C);
        for i in (1..n).rev() {
            let j = (rng.next_u64() % (i as u64 + 1)) as usize;
            shuffled.swap(i, j);
        }
        null += plug_in_mi(s, &shuffled);
    }
    Measure {
        h_symbol: entropy(s),
        h_target: entropy(t),
        mi,
        mi_null: null / NULL_SHUFFLES as f64,
        predictive: predictive_bits(s, t),
        n,
    }
}

/// Leave-one-out predictive information, in bits per tick.
///
/// For every tick, the code `p(target | symbol)` and the marginal `p(target)`
/// are estimated from every *other* tick with add-`1/K` smoothing, and the
/// tick's own target is scored under both. The mean difference in log-loss is
/// how much a parent who has to learn the code from the rest of the history
/// gains by reading the symbol. Leave-one-out rather than a split in time,
/// because the child drifts: a code learned on the first half meets levels in
/// the second half it never saw, and a marginal learned on the first half can
/// be badly wrong about the second, which would turn a drifting target into a
/// spurious gain. Negative values mean the learned code hurt.
fn predictive_bits(s: &[u32], t: &[u32]) -> f64 {
    let n = s.len();
    if n < 4 {
        return f64::NAN;
    }
    let classes = *t.iter().max().unwrap() as usize + 1;
    let symbols = *s.iter().max().unwrap() as usize + 1;
    let alpha = 1.0 / classes as f64;
    let mut marginal = vec![0.0f64; classes];
    let mut joint = vec![vec![0.0f64; classes]; symbols];
    let mut row_total = vec![0.0f64; symbols];
    for i in 0..n {
        marginal[t[i] as usize] += 1.0;
        joint[s[i] as usize][t[i] as usize] += 1.0;
        row_total[s[i] as usize] += 1.0;
    }
    let smooth = alpha * classes as f64;
    let mut gain = 0.0;
    for i in 0..n {
        let (sym, cls) = (s[i] as usize, t[i] as usize);
        let p_marg = (marginal[cls] - 1.0 + alpha) / ((n - 1) as f64 + smooth);
        let others = row_total[sym] - 1.0;
        let p_cond = if others <= 0.0 {
            p_marg
        } else {
            (joint[sym][cls] - 1.0 + alpha) / (others + smooth)
        };
        gain += p_cond.log2() - p_marg.log2();
    }
    gain / n as f64
}

/// Flip each of the low `bits` bits of every symbol independently with
/// probability `eps`.
pub fn corrupt(symbols: &[u32], bits: u32, eps: f64, seed: u64) -> Vec<u32> {
    let mut rng = Rng::derive(seed, bits.into(), (eps * 1e6) as u64, 0x004E_4F49_5345);
    symbols
        .iter()
        .map(|s| {
            let mut v = *s;
            for b in 0..bits {
                if rng.chance(eps) {
                    v ^= 1 << b;
                }
            }
            v
        })
        .collect()
}

/// Noise rates swept on the shipped 4-bit encoding.
pub const NOISE_RATES: [f64; 4] = [0.0, 0.02, 0.1, 0.3];

/// One encoding on one task.
#[derive(Clone, Debug)]
pub struct Row {
    pub encoding: Encoding,
    pub task: Task,
    pub measure: Measure,
}

/// The whole analysis for one seed.
#[derive(Clone, Debug)]
pub struct Analysis {
    pub seed: u64,
    pub ticks: usize,
    pub rows: Vec<Row>,
    /// `uniform:4` on `global_now` under bit flips, by rate.
    pub noise: Vec<(f64, Measure)>,
    /// The shipped encoding, at each width, applied to a window elsewhere,
    /// against `global_now`.
    pub window_control: Vec<(u32, Measure)>,
    /// Four random bits a tick against `global_now`.
    pub noise_control: Measure,
    /// Entropy of the horizon's occupancy at 64 levels: the most the channel
    /// could carry about what was sent.
    pub input_entropy: f64,
}

impl Analysis {
    pub fn row(&self, encoding: Encoding, task: Task) -> Option<&Row> {
        self.rows
            .iter()
            .find(|r| r.encoding == encoding && r.task == task)
    }
}

/// Analyse one child.
pub fn analyse(cfg: &Config, horizon: &Horizon) -> Analysis {
    let trace = run_trace(cfg, horizon);
    analyse_trace(&trace, cfg.world.seed)
}

/// Analyse an already-recorded trace.
pub fn analyse_trace(trace: &ChildTrace, seed: u64) -> Analysis {
    let mut rows = Vec::with_capacity(ENCODINGS.len() * TASKS.len());
    for enc in ENCODINGS {
        let symbols = enc.symbols(trace);
        for task in TASKS {
            let targets = task.targets(trace);
            rows.push(Row {
                encoding: enc,
                task,
                measure: measure(&symbols, &targets, seed),
            });
        }
    }
    let global = Task::GlobalNow.targets(trace);
    let four = Encoding::Uniform(4).symbols(trace);
    let noise = NOISE_RATES
        .iter()
        .map(|eps| (*eps, measure(&corrupt(&four, 4, *eps, seed), &global, seed)))
        .collect();
    let far = ChildTrace {
        horizon_occupancy: trace.window_occupancy.clone(),
        ..trace.clone()
    };
    let window_control = [2u32, 4, 6, 128]
        .iter()
        .map(|b| {
            (
                *b,
                measure(&Encoding::Uniform(*b).symbols(&far), &global, seed),
            )
        })
        .collect();
    let mut rng = Rng::derive(seed, 0x5241_4E44, trace.ticks() as u64, 0);
    let random: Vec<u32> = (0..trace.ticks())
        .map(|_| (rng.next_u64() % 16) as u32)
        .collect();
    Analysis {
        seed,
        ticks: trace.ticks(),
        rows,
        noise,
        window_control,
        noise_control: measure(&random, &global, seed),
        input_entropy: entropy(&Encoding::Uniform(128).symbols(trace)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic(n: usize) -> ChildTrace {
        // A slowly varying occupancy with a known quadrant structure.
        let mut rng = Rng::new(9);
        let mut occ = 0.3;
        let mut t = ChildTrace {
            horizon_occupancy: Vec::new(),
            global_occupancy: Vec::new(),
            quadrants: Vec::new(),
            digest: Vec::new(),
            projections: Vec::new(),
            window_occupancy: Vec::new(),
        };
        for i in 0..n {
            occ = (occ + 0.02 * (rng.next_f64() - 0.5)).clamp(0.05, 0.6);
            t.horizon_occupancy.push(occ);
            t.global_occupancy.push(occ * 0.9 + 0.01 * rng.next_f64());
            let dense = i % 4;
            let mut q = [occ * 0.8; 4];
            q[dense] = occ * 1.4;
            t.quadrants.push(q);
            t.digest.push(rng.next_u64());
            t.projections.push((rng.next_u64() & 0xFF) as u32);
            t.window_occupancy.push(0.5 - occ * 0.5);
        }
        t
    }

    #[test]
    fn quantile_bins_are_balanced_and_ordered() {
        let v: Vec<f64> = (0..80).map(|i| (i * 37 % 80) as f64).collect();
        let b = quantile_bins(&v, 8);
        let mut counts = [0usize; 8];
        for x in &b {
            counts[*x as usize] += 1;
        }
        assert!(counts.iter().all(|c| *c == 10));
        let hi = v.iter().position(|x| *x == 79.0).unwrap();
        let lo = v.iter().position(|x| *x == 0.0).unwrap();
        assert_eq!((b[lo], b[hi]), (0, 7));
    }

    #[test]
    fn a_symbol_stream_tells_most_about_what_it_encodes() {
        let tr = synthetic(400);
        let a = analyse_trace(&tr, 1);
        let own = a
            .row(Encoding::Uniform(6), Task::HorizonNow)
            .unwrap()
            .measure;
        let hash = a.row(Encoding::Hash(6), Task::HorizonNow).unwrap().measure;
        assert!(own.mi_corrected() > 1.5, "{own:?}");
        assert!(own.predictive > 1.0, "{own:?}");
        assert!(own.predictive <= own.h_target + 1e-9, "{own:?}");
        assert!(
            hash.mi_corrected().abs() < 0.3,
            "a hash carries no magnitude: {hash:?}"
        );
        assert!(hash.predictive < 0.2, "{hash:?}");
    }

    #[test]
    fn levels_saturate_and_one_bit_carries_at_most_one() {
        let tr = synthetic(300);
        assert_eq!(Encoding::Uniform(128).levels(), 64);
        assert_eq!(Encoding::Uniform(1).levels(), 2);
        let one = measure(
            &Encoding::Uniform(1).symbols(&tr),
            &Task::GlobalNow.targets(&tr),
            1,
        );
        assert!(one.h_symbol <= 1.0 + 1e-9);
        assert_eq!(Encoding::Quadrants.levels(), 16);
        assert!(Encoding::Quadrants.symbols(&tr).iter().all(|s| *s < 16));
    }

    #[test]
    fn the_quadrant_summary_finds_the_densest_quadrant() {
        let tr = synthetic(400);
        let a = analyse_trace(&tr, 1);
        let q = a
            .row(Encoding::Quadrants, Task::DensestQuadrant)
            .unwrap()
            .measure;
        let u = a
            .row(Encoding::Uniform(6), Task::DensestQuadrant)
            .unwrap()
            .measure;
        assert!(q.mi_corrected() > 0.4, "{q:?}");
        assert!(
            u.mi_corrected() < 0.3,
            "occupancy says nothing about arrangement: {u:?}"
        );
    }

    #[test]
    fn random_symbols_carry_nothing_and_noise_degrades_the_channel() {
        let tr = synthetic(400);
        let a = analyse_trace(&tr, 1);
        assert!(
            a.noise_control.mi_corrected().abs() < 0.2,
            "{:?}",
            a.noise_control
        );
        assert!(
            a.noise_control.predictive < 0.2,
            "random symbols must not predict: {:?}",
            a.noise_control
        );
        let clean = a.noise[0].1.mi_corrected();
        let noisy = a.noise.last().unwrap().1.mi_corrected();
        assert!(
            noisy < clean,
            "flipping bits must lose information: {clean} -> {noisy}"
        );
    }

    #[test]
    fn corruption_is_deterministic_and_touches_only_the_low_bits() {
        let s: Vec<u32> = (0..64).map(|i| i % 16).collect();
        let a = corrupt(&s, 4, 0.3, 7);
        assert_eq!(a, corrupt(&s, 4, 0.3, 7));
        assert!(a.iter().all(|v| *v < 16));
        assert_ne!(a, s);
        assert_eq!(corrupt(&s, 4, 0.0, 7), s);
    }

    #[test]
    fn measures_are_undefined_on_too_little_data() {
        let m = measure(&[1, 2], &[0, 1], 1);
        assert!(m.mi.is_nan());
        assert_eq!(m.n, 2);
    }

    #[test]
    fn the_future_task_shortens_the_series_by_the_lag() {
        let tr = synthetic(100);
        assert_eq!(Task::GlobalFuture.targets(&tr).len(), 100 - FUTURE_LAG);
        assert_eq!(Task::GlobalNow.targets(&tr).len(), 100);
    }
}
