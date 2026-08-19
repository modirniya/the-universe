//! Theory 3: black holes as serializing pipes. Theory 4's measurable half.
//!
//! A **pipe** is a one-way channel between layers. What goes in does not come
//! back, and what comes out bears no resemblance to what went in — which is the
//! behaviour of a serializing write, not of a door. The **horizon** is the
//! write surface: a region of the child universe whose contents are folded into
//! a single message each tick. The singularity is not a place inside the child
//! at all; it is outside the child's address space, which is why the child's
//! physics reports it as a division by zero rather than as a location.
//!
//! The framework's claim is that content structure is destroyed but *timing and
//! magnitude* may survive. That is testable, and this module tests it rather
//! than assuming it: [`Relay::content_avalanche`] measures whether the
//! arrangement survives, and [`Relay::magnitude_correlation`] measures whether
//! the amount and the moment do.
//!
//! # Mutual blindness is enforced by the compiler
//!
//! The child holds a [`WriteEnd`], which has `write` and nothing else — no
//! method returns anything about the far side, so a child cannot learn whether
//! anything received it, or what is over there. The parent holds a [`ReadEnd`],
//! which has no way to write. [`WriteEnd::seal`] consumes the write end to
//! produce the read end, and there is no path back.
//!
//! This is deliberate. In a project whose stated reason for choosing Rust is
//! that a strict compiler substitutes for human language expertise, mutual
//! blindness should be a thing the type system refuses to let you violate, not
//! a convention in a comment.
//!
//! # The logging threshold
//!
//! A parent's observer does not watch a child; it watches a dashboard. The
//! **logging threshold** is the magnitude below which nothing registers at all.
//! [`ReadEnd::above`] is the whole of the parent's access to the child, and
//! [`Relay::visible_fraction`] reports how much of a child's history clears it.
//!
//! Falsified within the model if: content structure survives serialization
//! (the digest would track the arrangement instead of scattering), or timing
//! and magnitude do *not* survive (the parent's view would be uncorrelated with
//! the child's behaviour, and the pipe would carry nothing at all).

use crate::config::Config;
use crate::constraints::{Constraints, Resolved};
use crate::observer::observe;
use crate::physics::tick;
use crate::rng::Rng;
use crate::space::{Geometry, World};
use serde::Deserialize;

/// One serialized write. This is everything that crosses.
///
/// A faithful description of the horizon's contents would need one bit per
/// cell. This is 128 bits regardless of how large the horizon is, which is
/// what makes the channel a bottleneck rather than a window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Message {
    /// When it crossed. Timing is the cheapest thing to preserve and, per the
    /// theory, among the likeliest to survive.
    pub tick: u64,
    /// How much crossed: occupancy of the horizon, in `[0, 1]`.
    pub magnitude: f64,
    /// The arrangement, folded to 64 bits. Position-sensitive, so it depends on
    /// the pattern — and avalanching, so it cannot be read back as one.
    pub digest: u64,
}

/// The write surface: a region of the child universe, in cells.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Horizon {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    /// The logging threshold: magnitude below which nothing registers on the
    /// far side. Used by the boot chain to decide whether a parent noticed its
    /// child's existence at all.
    #[serde(default = "default_threshold")]
    pub threshold: f64,
}

fn default_threshold() -> f64 {
    0.05
}

impl Default for Horizon {
    /// Offset from the origin so that a default horizon overlaps both observed
    /// and unobserved ground. What crosses a pipe is subject to the child's own
    /// optimizations, and pretending otherwise would flatter the model.
    fn default() -> Self {
        Horizon {
            x: 16,
            y: 16,
            width: 32,
            height: 32,
            threshold: default_threshold(),
        }
    }
}

impl Horizon {
    /// Bits a faithful description of one tick's contents would take.
    pub fn content_bits(&self) -> usize {
        self.width * self.height
    }

    /// Bits one message actually carries: magnitude and digest.
    pub const MESSAGE_BITS: usize = 128;

    /// Share of the horizon's information the channel can carry at all.
    pub fn compression_ratio(&self) -> f64 {
        Self::MESSAGE_BITS as f64 / self.content_bits().max(1) as f64
    }
}

/// The child's end. It can write, and it can do nothing else.
///
/// There is deliberately no `read`, no `peek`, and no return value carrying
/// anything from the far side. A universe on this end cannot discover that it
/// is being read, or by what.
#[derive(Debug, Default)]
pub struct WriteEnd {
    messages: Vec<Message>,
}

impl WriteEnd {
    pub fn new() -> Self {
        WriteEnd {
            messages: Vec::new(),
        }
    }

    /// Push one serialized write across. Returns nothing, on purpose.
    pub fn write(&mut self, m: Message) {
        self.messages.push(m);
    }

    /// Close the write surface and hand what crossed to the other side.
    ///
    /// Consumes the write end, so the same code cannot hold both halves.
    pub fn seal(self) -> ReadEnd {
        ReadEnd {
            messages: self.messages,
        }
    }
}

/// The parent's end. It can read what arrived, and it cannot reach back.
#[derive(Debug, Clone)]
pub struct ReadEnd {
    messages: Vec<Message>,
}

impl ReadEnd {
    /// Everything that crossed, whether or not anyone noticed it.
    pub fn all(&self) -> &[Message] {
        &self.messages
    }

    /// What a parent observing at this logging threshold actually sees.
    ///
    /// This is the parent's entire access to the child. Below the threshold,
    /// nothing registers — not quietly, not in aggregate, not at all.
    pub fn above(&self, threshold: f64) -> Vec<Message> {
        self.messages
            .iter()
            .copied()
            .filter(|m| m.magnitude >= threshold)
            .collect()
    }
}

/// Fold the horizon's contents into one message. Pure.
///
/// Reads through [`World::sample`], so a horizon lying over unrendered regions
/// sees their density rather than their detail — the child's own optimizations
/// apply to what crosses, which is the honest behaviour.
pub fn serialize(w: &World, h: &Horizon, tick: u64) -> Message {
    let mut live = 0u64;
    let mut total = 0u64;
    // Position-sensitive fold: the digest depends on the arrangement, not just
    // the count, or "content structure was destroyed" would be vacuous.
    let mut acc = Rng::derive(0x484F52495A4F4E, tick, 0, 0).next_u64();

    for row in 0..h.height {
        for col in 0..h.width {
            let x = w.geom.wrap_x((h.x + col) as isize);
            let y = w.geom.wrap_y((h.y + row) as isize);
            let bit = u64::from(w.sample(x, y) >= 0.5);
            live += bit;
            total += 1;
            acc = Rng::derive(acc, bit, col as u64, row as u64).next_u64();
        }
    }

    Message {
        tick,
        magnitude: if total == 0 {
            0.0
        } else {
            live as f64 / total as f64
        },
        digest: acc,
    }
}

/// What one child's transmission looked like from both sides.
#[derive(Clone, Debug)]
pub struct Relay {
    pub horizon: Horizon,
    /// What the parent received.
    pub received: ReadEnd,
    /// The child's global occupancy each tick — the ground truth the parent
    /// does *not* have, kept here only so the experiment can ask what was
    /// recoverable.
    pub child_truth: Vec<f64>,
    /// Mean fraction of digest bits that flip when a single cell of the
    /// horizon is changed.
    pub content_avalanche: f64,
}

impl Relay {
    /// Pearson correlation between what crossed and what the child was doing.
    ///
    /// The parent sees a keyhole. This asks how much of the whole universe's
    /// behaviour that keyhole tracks.
    pub fn magnitude_correlation(&self) -> f64 {
        let seen: Vec<f64> = self.received.all().iter().map(|m| m.magnitude).collect();
        correlation(&seen, &self.child_truth)
    }

    /// Share of the child's ticks that clear a logging threshold.
    pub fn visible_fraction(&self, threshold: f64) -> f64 {
        let n = self.received.all().len();
        if n == 0 {
            return 0.0;
        }
        self.received.above(threshold).len() as f64 / n as f64
    }

    /// Correlation still available to a parent that only records events above
    /// a threshold, measured against the child's behaviour at those same ticks.
    ///
    /// Returns `NaN` below [`MIN_CORRELATION_SAMPLES`]. Pearson on two points
    /// is always exactly ±1, so a high threshold that admits a handful of
    /// events would otherwise report a perfect correlation and look like the
    /// strongest row in the table while being pure noise.
    pub fn correlation_above(&self, threshold: f64) -> f64 {
        let kept = self.received.above(threshold);
        let seen: Vec<f64> = kept.iter().map(|m| m.magnitude).collect();
        let truth: Vec<f64> = kept
            .iter()
            .filter_map(|m| self.child_truth.get(m.tick as usize).copied())
            .collect();
        if seen.len().min(truth.len()) < MIN_CORRELATION_SAMPLES {
            return f64::NAN;
        }
        correlation(&seen, &truth)
    }

    /// How many of the child's ticks register above a threshold.
    pub fn registered(&self, threshold: f64) -> usize {
        self.received.above(threshold).len()
    }
}

/// Pearson correlation. Returns `NaN` when either series never varies, which
/// is honest: a constant series has no correlation to report.
pub fn correlation(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len());
    if n < 2 {
        return f64::NAN;
    }
    let (a, b) = (&a[..n], &b[..n]);
    let ma = a.iter().sum::<f64>() / n as f64;
    let mb = b.iter().sum::<f64>() / n as f64;
    let mut num = 0.0;
    let mut da = 0.0;
    let mut db = 0.0;
    for i in 0..n {
        let (x, y) = (a[i] - ma, b[i] - mb);
        num += x * y;
        da += x * x;
        db += y * y;
    }
    if da == 0.0 || db == 0.0 {
        return f64::NAN;
    }
    num / (da * db).sqrt()
}

/// Mean fraction of digest bits that flip when one cell of the horizon changes.
///
/// Near 0.5 means the fold behaves like a hash: a one-cell difference produces
/// an unrelated digest, so no amount of comparing digests recovers the
/// arrangement. Near 0 would mean the structure survived, and Theory 3 would be
/// wrong in the direction that matters.
pub fn avalanche(w: &World, h: &Horizon, tick: u64, samples: usize) -> f64 {
    let base = serialize(w, h, tick);
    let mut probe = w.clone();
    let cells = h.width * h.height;
    if cells == 0 || samples == 0 {
        return 0.0;
    }

    let mut total = 0.0;
    let mut taken = 0;
    // Walk a stride through the horizon rather than a random sample, so the
    // measurement is deterministic like everything else here.
    let stride = (cells / samples).max(1);
    for i in (0..cells).step_by(stride) {
        let x = w.geom.wrap_x((h.x + i % h.width) as isize);
        let y = w.geom.wrap_y((h.y + i / h.width) as isize);
        let idx = w.geom.idx(x, y);
        let b = w.geom.block_of(x, y);
        if !w.resolved[b] {
            continue;
        }
        probe.cells[idx] ^= 1;
        let flipped = serialize(&probe, h, tick);
        probe.cells[idx] ^= 1;
        total += (base.digest ^ flipped.digest).count_ones() as f64 / 64.0;
        taken += 1;
    }

    if taken == 0 {
        0.0
    } else {
        total / taken as f64
    }
}

/// Fewest events from which a correlation is worth reporting.
///
/// Pearson on two points is always exactly ±1; on three or four it is barely
/// better. A threshold sweep runs straight into this, because the whole point
/// of a high threshold is that very little clears it.
pub const MIN_CORRELATION_SAMPLES: usize = 5;

/// One row of the logging-threshold sweep.
#[derive(Clone, Copy, Debug)]
pub struct ThresholdRow {
    pub threshold: f64,
    /// Share of the child's ticks that register at all.
    pub visible_fraction: f64,
    /// How many events that share amounts to.
    pub samples: usize,
    /// Correlation still available from what did register, or `NaN` when too
    /// few events registered for the number to mean anything.
    pub correlation: f64,
}

impl Relay {
    /// How the parent's view degrades as it stops bothering with small events.
    pub fn threshold_sweep(&self, thresholds: &[f64]) -> Vec<ThresholdRow> {
        thresholds
            .iter()
            .map(|t| ThresholdRow {
                threshold: *t,
                visible_fraction: self.visible_fraction(*t),
                samples: self.registered(*t),
                correlation: self.correlation_above(*t),
            })
            .collect()
    }
}

/// Thresholds the report sweeps by default.
pub const THRESHOLDS: &[f64] = &[0.0, 0.02, 0.05, 0.10, 0.15, 0.20, 0.30, 0.50];

/// Run a child universe and transmit its horizon, one message per tick.
///
/// The child is an ordinary universe with every constraint in force. It is not
/// told it is being read, and nothing here gives it a way to find out.
pub fn run_relay(cfg: &Config, horizon: &Horizon) -> Relay {
    let res = Resolved::new(&Constraints::ALL_ON, &cfg.params);
    let geom = Geometry::new(
        cfg.world.width,
        cfg.world.height,
        res.subdivision,
        res.block_size,
    );

    let mut world = World::seed(geom, cfg.world.seed, cfg.world.init_density);
    let mut write = WriteEnd::new();
    let mut child_truth = Vec::with_capacity(cfg.world.ticks as usize);

    let mut avalanche_total = 0.0;
    let mut avalanche_samples = 0u32;
    // Sampling a handful of ticks rather than every one: avalanche costs a
    // full re-serialization per cell probed, and it does not drift.
    let stride = (cfg.world.ticks / 8).max(1);

    for t in 0..cfg.world.ticks {
        let (observed, _) = observe(&world, &cfg.observer, t, cfg.world.seed, res.lazy);
        let (advanced, _) = tick(&observed, &cfg.rules, &res);

        write.write(serialize(&advanced, horizon, t));
        child_truth.push(advanced.live_fraction());

        if t % stride == 0 {
            avalanche_total += avalanche(&advanced, horizon, t, 32);
            avalanche_samples += 1;
        }

        world = advanced;
    }

    Relay {
        horizon: *horizon,
        received: write.seal(),
        child_truth,
        content_avalanche: if avalanche_samples == 0 {
            0.0
        } else {
            avalanche_total / avalanche_samples as f64
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::space::Geometry;

    fn world() -> World {
        World::seed(Geometry::new(64, 64, 1, 16), 42, 0.3)
    }

    fn horizon() -> Horizon {
        Horizon {
            x: 8,
            y: 8,
            width: 16,
            height: 16,
            threshold: default_threshold(),
        }
    }

    #[test]
    fn the_channel_is_a_bottleneck_not_a_window() {
        let h = horizon();
        assert_eq!(h.content_bits(), 256);
        assert!(
            h.compression_ratio() <= 0.5,
            "128 bits should not describe a 256-cell region"
        );
        let big = Horizon {
            width: 64,
            height: 64,
            ..h
        };
        assert!(big.compression_ratio() < h.compression_ratio());
    }

    #[test]
    fn serialization_is_deterministic() {
        let w = world();
        assert_eq!(serialize(&w, &horizon(), 7), serialize(&w, &horizon(), 7));
    }

    #[test]
    fn magnitude_is_the_horizons_occupancy() {
        let mut w = world();
        let h = Horizon {
            x: 0,
            y: 0,
            width: 8,
            height: 8,
            threshold: default_threshold(),
        };
        for y in 0..8 {
            for x in 0..8 {
                let i = w.geom.idx(x, y);
                w.cells[i] = 1;
            }
        }
        assert!((serialize(&w, &h, 0).magnitude - 1.0).abs() < 1e-12);

        for y in 0..8 {
            for x in 0..8 {
                let i = w.geom.idx(x, y);
                w.cells[i] = 0;
            }
        }
        assert_eq!(serialize(&w, &h, 0).magnitude, 0.0);
    }

    #[test]
    fn the_digest_depends_on_arrangement_not_just_amount() {
        // Two patterns with identical occupancy must not produce the same
        // digest, or the fold would be recording only the count.
        let mut a = world();
        let h = Horizon {
            x: 0,
            y: 0,
            width: 8,
            height: 8,
            threshold: default_threshold(),
        };
        for y in 0..8 {
            for x in 0..8 {
                let i = a.geom.idx(x, y);
                a.cells[i] = u8::from(x < 4);
            }
        }
        let mut b = a.clone();
        for y in 0..8 {
            for x in 0..8 {
                let i = b.geom.idx(x, y);
                b.cells[i] = u8::from(y < 4);
            }
        }
        let (ma, mb) = (serialize(&a, &h, 0), serialize(&b, &h, 0));
        assert!((ma.magnitude - mb.magnitude).abs() < 1e-12, "same amount");
        assert_ne!(ma.digest, mb.digest, "different arrangement");
    }

    #[test]
    fn content_structure_does_not_survive() {
        // The core of Theory 3. One changed cell should scatter the digest, so
        // that comparing digests tells a parent nothing about the pattern.
        let w = world();
        let a = avalanche(&w, &horizon(), 3, 64);
        assert!(
            (0.35..=0.65).contains(&a),
            "expected roughly half the bits to flip, got {a}"
        );
    }

    #[test]
    fn timing_is_carried_exactly() {
        let w = world();
        for tick in [0u64, 1, 99, 4096] {
            assert_eq!(serialize(&w, &horizon(), tick).tick, tick);
        }
    }

    #[test]
    fn a_child_cannot_read_its_own_pipe() {
        // Enforced by the type system: `WriteEnd` exposes no way to observe
        // anything. This test exists to state the intent; the compiler is what
        // actually holds the line, since adding a read method here would be
        // the only way to break it.
        let mut w = WriteEnd::new();
        w.write(Message {
            tick: 0,
            magnitude: 0.5,
            digest: 1,
        });
        let r = w.seal();
        assert_eq!(r.all().len(), 1);
    }

    #[test]
    fn the_logging_threshold_hides_small_events() {
        let mut w = WriteEnd::new();
        for (tick, magnitude) in [(0, 0.01), (1, 0.4), (2, 0.02), (3, 0.9)] {
            w.write(Message {
                tick,
                magnitude,
                digest: tick,
            });
        }
        let r = w.seal();
        assert_eq!(r.all().len(), 4);
        assert_eq!(r.above(0.0).len(), 4);
        assert_eq!(r.above(0.3).len(), 2);
        assert_eq!(r.above(1.0).len(), 0, "nothing registers at all");
    }

    #[test]
    fn a_correlation_from_too_few_events_is_not_reported() {
        // Two points always correlate perfectly. A sweep must not present that
        // as a finding.
        let mut w = WriteEnd::new();
        let history = [
            (0u64, 0.9),
            (1, 0.1),
            (2, 0.95),
            (3, 0.1),
            (4, 0.2),
            (5, 0.15),
        ];
        for (tick, magnitude) in history {
            w.write(Message {
                tick,
                magnitude,
                digest: tick,
            });
        }
        let relay = Relay {
            horizon: horizon(),
            received: w.seal(),
            child_truth: vec![0.5, 0.4, 0.6, 0.3, 0.35, 0.45],
            content_avalanche: 0.5,
        };

        assert_eq!(relay.registered(0.5), 2);
        assert!(
            relay.correlation_above(0.5).is_nan(),
            "two events must not report a correlation"
        );
        assert_eq!(relay.registered(0.0), history.len());
        assert!(
            !relay.correlation_above(0.0).is_nan(),
            "at or above the minimum, a correlation is reportable"
        );
    }

    #[test]
    fn correlation_is_one_for_a_series_with_itself() {
        let a = [0.1, 0.5, 0.2, 0.9, 0.4];
        assert!((correlation(&a, &a) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn correlation_is_minus_one_when_inverted() {
        let a = [0.1, 0.5, 0.2, 0.9];
        let b: Vec<f64> = a.iter().map(|v| 1.0 - v).collect();
        assert!((correlation(&a, &b) + 1.0).abs() < 1e-12);
    }

    #[test]
    fn a_flat_series_has_no_correlation_to_report() {
        let flat = [0.5, 0.5, 0.5, 0.5];
        let varying = [0.1, 0.2, 0.3, 0.4];
        assert!(correlation(&flat, &varying).is_nan());
    }

    #[test]
    fn the_horizon_wraps_with_the_world() {
        // The world is a torus; a horizon straddling the seam must still read
        // cells rather than falling off an edge that does not exist.
        let w = world();
        let seam = Horizon {
            x: 60,
            y: 60,
            width: 8,
            height: 8,
            threshold: default_threshold(),
        };
        let m = serialize(&w, &seam, 0);
        assert!((0.0..=1.0).contains(&m.magnitude));
    }
}
