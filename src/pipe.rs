//! Theory 3: the horizon as a serializing pipe. Theory 4's measurable half.
//!
//! A **pipe** is a one-way channel between layers. What goes in does not come
//! back, and what comes out bears no resemblance to what went in — which is the
//! behaviour of a serializing write, not of a door. The **horizon** is the
//! write surface: a region of the child universe whose contents are folded into
//! a single message each tick.
//!
//! The framework's inspiration for this is the information-loss behaviour
//! associated with black holes. Nothing here models one: the universe has no
//! gravity, no horizon in the physical sense and no singularity, and the word
//! "horizon" names a rectangle of cells. The analogy is a physical hypothesis
//! and lives in `docs/philosophy.md`; this module is a channel abstraction and
//! is measured as one, in [`crate::information`].
//!
//! # What is designed in, and what is measured
//!
//! The framework's claim is that content structure is destroyed but *timing
//! and magnitude* survive. Most of that is a definition here, not a result. A
//! [`Message`] has a slot per tick and a field for magnitude, so timing and
//! magnitude cross because the message was built to carry them; the digest is
//! a hash, so it avalanches because hashes do. [`Relay::content_avalanche`]
//! checks that the fold was built correctly, and nothing more.
//!
//! What has to be run to be known is *how much* of the child the parent can
//! still track, and how narrow the channel can get before that is lost. The
//! channel's width is a dial ([`Horizon::bits`]), and [`Relay::width_sweep`]
//! reads one child through every width in [`WIDTHS`] at once. That curve is
//! this module's result.
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
//! Falsified within the model if: the parent's view is uncorrelated with the
//! child's behaviour at every width (the pipe would carry nothing at all), or
//! the channel needs nearly the full horizon to track the child (it would be a
//! window rather than a bottleneck). The avalanche failing would falsify the
//! fold, not the theory.

use crate::config::Config;
use crate::constraints::{Constraints, Resolved};
use crate::observer::observe;
use crate::physics::tick;
use crate::rng::Rng;
use crate::space::{Geometry, World};
use serde::Deserialize;

/// One serialized write. This is everything that crosses.
///
/// A message is a payload of at most [`MAX_BITS`] bits and nothing else. The
/// fields are private and the payload is all that is stored, so a parent
/// reading one gets back what the channel's width allows and cannot reach the
/// value the child started from. Timing is not in the payload: one message
/// crosses per tick, so *when* is the slot it arrives in.
///
/// Bits are spent on magnitude first and the digest gets whatever is left:
///
/// - **magnitude** takes `min(bits, 64)`. At 53 bits or more it is the exact
///   `f64` (53 is the mantissa, so nothing is lost); below that it is rounded
///   onto `2^bits` evenly spaced levels over `[0, 1]`. Uniform levels are the
///   least tuned encoding available: they favour no particular occupancy.
/// - **digest** takes `bits - 64` when that is positive, truncated from the
///   64-bit fold. Below 65 bits nothing about the arrangement crosses at all.
///
/// So 128 bits carries everything this pipe has ever carried, and every
/// narrower width is a strict loss from it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Message {
    tick: u64,
    bits: u32,
    payload: u128,
}

/// Widest channel the encoding supports: an exact magnitude and a full digest.
pub const MAX_BITS: u32 = 128;

/// Mantissa width of an `f64`. At or above this a magnitude crosses exactly.
const EXACT_BITS: u32 = 53;

impl Message {
    /// Encode one tick's magnitude and digest into a channel `bits` wide.
    pub fn pack(tick: u64, magnitude: f64, digest: u64, bits: u32) -> Message {
        let bits = bits.clamp(1, MAX_BITS);
        let m_bits = Self::magnitude_bits(bits);
        let d_bits = Self::digest_bits(bits);
        let m = if m_bits >= EXACT_BITS {
            u128::from(magnitude.to_bits())
        } else {
            let levels = (1u64 << m_bits) - 1;
            u128::from((magnitude.clamp(0.0, 1.0) * levels as f64).round() as u64)
        };
        let d = if d_bits == 0 {
            0
        } else {
            u128::from(digest >> (64 - d_bits))
        };
        Message {
            tick,
            bits,
            payload: m | (d << 64),
        }
    }

    fn magnitude_bits(bits: u32) -> u32 {
        bits.min(64)
    }

    fn digest_bits(bits: u32) -> u32 {
        bits.saturating_sub(64)
    }

    /// When it crossed.
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// How wide the channel was.
    pub fn bits(&self) -> u32 {
        self.bits
    }

    /// How much crossed, as far as the channel's width preserves it.
    pub fn magnitude(&self) -> f64 {
        let m_bits = Self::magnitude_bits(self.bits);
        let raw = (self.payload & u128::from(u64::MAX)) as u64;
        if m_bits >= EXACT_BITS {
            f64::from_bits(raw)
        } else {
            raw as f64 / ((1u64 << m_bits) - 1) as f64
        }
    }

    /// What survives of the arrangement: the top `bits - 64` bits of the fold,
    /// or nothing at all on a channel 64 bits wide or narrower.
    pub fn digest(&self) -> u64 {
        let d_bits = Self::digest_bits(self.bits);
        if d_bits == 0 {
            return 0;
        }
        ((self.payload >> 64) as u64) << (64 - d_bits)
    }

    /// Bits of the digest this message actually carries.
    pub fn digest_width(&self) -> u32 {
        Self::digest_bits(self.bits)
    }
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
    /// Width of the channel, in bits per tick. 128 carries an exact magnitude
    /// and a full 64-bit digest; anything narrower loses some of one or both.
    /// See [`Message`] for how the bits are spent.
    #[serde(default = "default_bits")]
    pub bits: u32,
}

fn default_threshold() -> f64 {
    0.05
}

fn default_bits() -> u32 {
    MAX_BITS
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
            bits: default_bits(),
        }
    }
}

impl Horizon {
    /// Bits a faithful description of one tick's contents would take.
    pub fn content_bits(&self) -> usize {
        self.width * self.height
    }

    /// Bits one message actually carries.
    pub fn message_bits(&self) -> usize {
        self.bits as usize
    }

    /// Share of the horizon's information the channel can carry at all.
    pub fn compression_ratio(&self) -> f64 {
        self.message_bits() as f64 / self.content_bits().max(1) as f64
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
            .filter(|m| m.magnitude() >= threshold)
            .collect()
    }
}

/// Fold the horizon's contents into one message. Pure.
///
/// Reads through [`World::sample`], so a horizon lying over unrendered regions
/// sees their density rather than their detail — the child's own optimizations
/// apply to what crosses, which is the honest behaviour.
pub fn serialize(w: &World, h: &Horizon, tick: u64) -> Message {
    let (magnitude, digest) = fold(w, h, tick);
    Message::pack(tick, magnitude, digest, h.bits)
}

/// The horizon's occupancy and its position-sensitive digest, before either is
/// squeezed into a channel. Only the child side ever holds these.
fn fold(w: &World, h: &Horizon, tick: u64) -> (f64, u64) {
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

    let magnitude = if total == 0 {
        0.0
    } else {
        live as f64 / total as f64
    };
    (magnitude, acc)
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
    /// The same child, read through channels of other widths at once. Each is
    /// its own pipe with its own sealed read end; the child writes to all of
    /// them and can tell none of them apart.
    pub by_width: Vec<ReadEnd>,
}

impl Relay {
    /// Pearson correlation between what crossed and what the child was doing.
    ///
    /// The parent sees a keyhole. This asks how much of the whole universe's
    /// behaviour that keyhole tracks.
    pub fn magnitude_correlation(&self) -> f64 {
        let seen: Vec<f64> = self.received.all().iter().map(|m| m.magnitude()).collect();
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
        let seen: Vec<f64> = kept.iter().map(|m| m.magnitude()).collect();
        let truth: Vec<f64> = kept
            .iter()
            .filter_map(|m| self.child_truth.get(m.tick() as usize).copied())
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
/// This is a check on the fold, not a finding about pipes. A digest built by
/// chaining a hash over every cell is expected to avalanche, and near 0.5 says
/// it does: a one-cell difference produces an unrelated digest. Near 0 would
/// mean the fold was badly built, which would undermine the claim that content
/// does not cross in a readable form. On a channel too narrow to carry any
/// digest there is nothing to measure, and the answer is `NaN`.
pub fn avalanche(w: &World, h: &Horizon, tick: u64, samples: usize) -> f64 {
    let base = serialize(w, h, tick);
    let width = f64::from(base.digest_width());
    if width == 0.0 {
        return f64::NAN;
    }
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
        total += (base.digest() ^ flipped.digest()).count_ones() as f64 / width;
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
    let mut widths: Vec<WriteEnd> = WIDTHS.iter().map(|_| WriteEnd::new()).collect();
    let mut child_truth = Vec::with_capacity(cfg.world.ticks as usize);

    let mut avalanche_total = 0.0;
    let mut avalanche_samples = 0u32;
    // Sampling a handful of ticks rather than every one: avalanche costs a
    // full re-serialization per cell probed, and it does not drift.
    let stride = (cfg.world.ticks / 8).max(1);

    for t in 0..cfg.world.ticks {
        let (observed, _) = observe(&world, &cfg.observer, t, cfg.world.seed, res.lazy);
        let (advanced, _) = tick(&observed, &cfg.rules, &res);

        let (magnitude, digest) = fold(&advanced, horizon, t);
        write.write(Message::pack(t, magnitude, digest, horizon.bits));
        for (end, &bits) in widths.iter_mut().zip(WIDTHS) {
            end.write(Message::pack(t, magnitude, digest, bits));
        }
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
        by_width: widths.into_iter().map(WriteEnd::seal).collect(),
    }
}

/// Channel widths the relay always measures, narrowest first.
pub const WIDTHS: &[u32] = &[1, 2, 3, 4, 6, 8, 16, 32, 64, 128];

/// One row of the channel-width sweep.
#[derive(Clone, Copy, Debug)]
pub struct WidthRow {
    pub bits: u32,
    /// Distinct magnitudes the parent actually received at this width.
    pub levels_seen: usize,
    /// Correlation between what crossed and what the child was doing, or
    /// `NaN` when everything that crossed was the same value.
    pub correlation: f64,
}

impl Relay {
    /// How much of the child's behaviour survives as the channel narrows.
    ///
    /// This is the measurable part of Theory 3. That *some* magnitude crosses
    /// is designed into [`Message`]; how much of the child it still tracks at
    /// a given width is not, and has to be run to be known.
    pub fn width_sweep(&self) -> Vec<WidthRow> {
        WIDTHS
            .iter()
            .zip(&self.by_width)
            .map(|(&bits, end)| {
                let seen: Vec<f64> = end.all().iter().map(|m| m.magnitude()).collect();
                let mut distinct: Vec<u64> = seen.iter().map(|v| v.to_bits()).collect();
                distinct.sort_unstable();
                distinct.dedup();
                WidthRow {
                    bits,
                    levels_seen: distinct.len(),
                    correlation: correlation(&seen, &self.child_truth),
                }
            })
            .collect()
    }

    /// Narrowest width at which the parent's view tracks the child at least
    /// as well as `share` of what the widest channel manages.
    pub fn width_for(&self, share: f64) -> Option<u32> {
        let rows = self.width_sweep();
        let best = rows.last()?.correlation;
        if !best.is_finite() {
            return None;
        }
        rows.iter()
            .find(|r| r.correlation.is_finite() && r.correlation >= share * best)
            .map(|r| r.bits)
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
            bits: default_bits(),
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
            bits: default_bits(),
        };
        for y in 0..8 {
            for x in 0..8 {
                let i = w.geom.idx(x, y);
                w.cells[i] = 1;
            }
        }
        assert!((serialize(&w, &h, 0).magnitude() - 1.0).abs() < 1e-12);

        for y in 0..8 {
            for x in 0..8 {
                let i = w.geom.idx(x, y);
                w.cells[i] = 0;
            }
        }
        assert_eq!(serialize(&w, &h, 0).magnitude(), 0.0);
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
            bits: default_bits(),
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
        assert!(
            (ma.magnitude() - mb.magnitude()).abs() < 1e-12,
            "same amount"
        );
        assert_ne!(ma.digest(), mb.digest(), "different arrangement");
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
            assert_eq!(serialize(&w, &horizon(), tick).tick(), tick);
        }
    }

    #[test]
    fn a_child_cannot_read_its_own_pipe() {
        // Enforced by the type system: `WriteEnd` exposes no way to observe
        // anything. This test exists to state the intent; the compiler is what
        // actually holds the line, since adding a read method here would be
        // the only way to break it.
        let mut w = WriteEnd::new();
        w.write(Message::pack(0, 0.5, 1, MAX_BITS));
        let r = w.seal();
        assert_eq!(r.all().len(), 1);
    }

    #[test]
    fn the_logging_threshold_hides_small_events() {
        let mut w = WriteEnd::new();
        for (tick, magnitude) in [(0, 0.01), (1, 0.4), (2, 0.02), (3, 0.9)] {
            w.write(Message::pack(tick, magnitude, tick, MAX_BITS));
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
            w.write(Message::pack(tick, magnitude, tick, MAX_BITS));
        }
        let relay = Relay {
            horizon: horizon(),
            received: w.seal(),
            child_truth: vec![0.5, 0.4, 0.6, 0.3, 0.35, 0.45],
            content_avalanche: 0.5,
            by_width: Vec::new(),
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
    fn the_full_width_carries_magnitude_and_digest_exactly() {
        let m = Message::pack(7, 0.123_456_789, 0xDEAD_BEEF_0123_4567, MAX_BITS);
        assert_eq!(m.tick(), 7);
        assert_eq!(m.magnitude(), 0.123_456_789);
        assert_eq!(m.digest(), 0xDEAD_BEEF_0123_4567);
        assert_eq!(m.digest_width(), 64);
    }

    #[test]
    fn magnitude_is_paid_for_first_and_the_digest_gets_the_rest() {
        let d = 0xFFFF_0000_FFFF_0000u64;
        // 96 bits: an exact magnitude and the top 32 bits of the digest.
        let m = Message::pack(0, 0.3, d, 96);
        assert_eq!(m.magnitude(), 0.3);
        assert_eq!(m.digest(), 0xFFFF_0000_0000_0000);
        // 64 bits and below: no arrangement crosses at all.
        let m = Message::pack(0, 0.3, d, 64);
        assert_eq!(m.magnitude(), 0.3);
        assert_eq!((m.digest(), m.digest_width()), (0, 0));
    }

    #[test]
    fn a_narrow_channel_rounds_magnitude_onto_even_levels() {
        // 8 bits: 255 steps over [0, 1], so the error is at most half a step.
        for v in [0.0, 0.01, 0.2, 0.5, 0.999, 1.0] {
            let m = Message::pack(0, v, 0, 8);
            assert!((m.magnitude() - v).abs() <= 0.5 / 255.0 + 1e-12, "{v}");
        }
        // 1 bit: one threshold, at one half.
        assert_eq!(Message::pack(0, 0.49, 0, 1).magnitude(), 0.0);
        assert_eq!(Message::pack(0, 0.51, 0, 1).magnitude(), 1.0);
    }

    #[test]
    fn widths_outside_the_encoding_are_clamped() {
        assert_eq!(Message::pack(0, 0.5, 0, 0).bits(), 1);
        assert_eq!(Message::pack(0, 0.5, 0, 500).bits(), MAX_BITS);
    }

    #[test]
    fn there_is_no_avalanche_to_measure_without_a_digest() {
        let h = Horizon {
            bits: 64,
            ..horizon()
        };
        assert!(avalanche(&world(), &h, 3, 64).is_nan());
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
            bits: default_bits(),
        };
        let m = serialize(&w, &seam, 0);
        assert!((0.0..=1.0).contains(&m.magnitude()));
    }
}
