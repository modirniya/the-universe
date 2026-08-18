//! The creator's runtime input channel.
//!
//! Every non-deterministic choice inside a universe enters through here. The
//! generator is SplitMix64, written out in full rather than pulled from a
//! crate, because determinism is load-bearing for this project: the value
//! stream must be identical across machines, compiler versions and future
//! targets (WASM), and it must be auditable by reading one file.
//!
//! The key operation is [`Rng::derive`]: a *positional* stream. A block that
//! is rendered at tick 40 must receive the same detail whether or not any
//! other block was rendered first. Pulling from a single shared stream would
//! make the result depend on visit order; deriving from coordinates does not.
//!
//! Falsified within the model if: two runs with the same seed and config
//! produce different macro observables (see `tests/determinism.rs`).

/// A SplitMix64 state. Cheap to copy, cheap to derive.
#[derive(Clone, Copy, Debug)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// The root stream. In the fiction: the seed the creator supplied at launch.
    pub fn new(seed: u64) -> Self {
        Rng { state: seed }
    }

    /// A positional sub-stream, independent of visit order.
    ///
    /// Mixing is done with distinct odd multipliers so that adjacent
    /// coordinates do not produce correlated streams.
    pub fn derive(seed: u64, a: u64, b: u64, c: u64) -> Self {
        let mut s = seed;
        s = mix(s ^ a.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        s = mix(s ^ b.wrapping_mul(0xBF58_476D_1CE4_E5B9));
        s = mix(s ^ c.wrapping_mul(0x94D0_49BB_1331_11EB));
        Rng { state: s }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        mix(self.state)
    }

    /// Uniform in `[0, 1)`, using the top 53 bits.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// A coin weighted by `p`. `p <= 0` is never, `p >= 1` is always.
    pub fn chance(&mut self, p: f64) -> bool {
        self.next_f64() < p
    }
}

fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_stream() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn different_seed_different_stream() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(43);
        let diff = (0..64).filter(|_| a.next_u64() != b.next_u64()).count();
        assert!(diff > 60, "streams should not track each other");
    }

    #[test]
    fn derive_is_order_independent() {
        // The whole point: rendering block (3,4) at tick 9 gives the same
        // detail regardless of what was rendered before it.
        let mut x = Rng::derive(7, 3, 4, 9);
        let mut y = Rng::derive(7, 3, 4, 9);
        assert_eq!(x.next_u64(), y.next_u64());

        let mut z = Rng::derive(7, 4, 3, 9);
        assert_ne!(Rng::derive(7, 3, 4, 9).next_u64(), z.next_u64());
    }

    #[test]
    fn uniform_enough() {
        let mut r = Rng::new(1);
        let n = 100_000;
        let mean: f64 = (0..n).map(|_| r.next_f64()).sum::<f64>() / n as f64;
        assert!((mean - 0.5).abs() < 0.01, "mean was {mean}");
    }

    #[test]
    fn f64_in_unit_interval() {
        let mut r = Rng::new(99);
        for _ in 0..10_000 {
            let v = r.next_f64();
            assert!((0.0..1.0).contains(&v));
        }
    }
}
