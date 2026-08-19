//! One fixed universe, run identically everywhere, reduced to one number.
//!
//! Determinism is the project's first rule, and until now it was checked only
//! on the machine doing the checking. A browser target makes that insufficient:
//! "same seed, same universe" has to survive a change of platform, or the rule
//! is really "same seed, same universe, on this laptop".
//!
//! So this module defines a universe with every parameter pinned, runs it for a
//! fixed number of ticks, and reduces the result to a single `u64`. The native
//! test suite asserts that number. The WebAssembly test suite asserts the same
//! constant. If the two ever disagree, something in the value stream is
//! platform-dependent and the first rule is broken — which is exactly the
//! failure this exists to catch, and it will be recorded as a finding rather
//! than papered over.
//!
//! The pinned constants below are load-bearing. Changing any of them changes
//! the fingerprint, and the constant in the tests has to change with it. That
//! is deliberate friction: a silent change to the reference universe would make
//! the check meaningless.
//!
//! Falsified within the model if: the same seed and the same tick count produce
//! different states on different targets.

use crate::budget::Degradation;
use crate::config::{Config, ReportCfg, WorldCfg};
use crate::constraints::{Constraints, Params, Resolved};
use crate::observer::{Probe, observe};
use crate::physics::{Rules, tick};
use crate::pipe::Horizon;
use crate::space::{Geometry, World};

/// What the reference universe reduces to.
///
/// Asserted by the native test suite and, separately, by the WebAssembly one.
/// The two targets never compare notes at runtime; they each compare against
/// this constant, which is what makes disagreement detectable rather than
/// merely possible.
///
/// If a deliberate change to the reference universe moves this number, update
/// it here and say so in the commit. If it moves without a deliberate change,
/// that is the finding.
pub const GOLDEN_FINGERPRINT: u64 = 6_900_610_681_785_451_805;

/// The reference universe's seed.
pub const GOLDEN_SEED: u64 = 20_260_818;
/// How long it runs.
pub const GOLDEN_TICKS: u64 = 64;
/// Its edge, in base cells. Small enough that a browser can run the check
/// instantly, large enough to exercise several blocks and a partial probe.
pub const GOLDEN_EDGE: usize = 48;

/// The reference universe. Every field pinned; nothing defaulted.
pub fn golden_config() -> Config {
    Config {
        world: WorldCfg {
            width: GOLDEN_EDGE,
            height: GOLDEN_EDGE,
            ticks: GOLDEN_TICKS,
            seed: GOLDEN_SEED,
            init_density: 0.3,
        },
        rules: Rules::default(),
        constraints: Constraints::ALL_ON,
        params: Params {
            subdivision: 2,
            substeps: 2,
            capped_radius: 1,
            uncapped_radius: 3,
            block_size: 16,
        },
        // Deliberately smaller than the world and offset from the origin, so
        // the run exercises rendering, collapse, and the boundary between
        // resolved and coarse ground rather than a uniformly observed world.
        observer: Probe {
            x: 8,
            y: 8,
            width: 24,
            height: 24,
        },
        report: ReportCfg {
            macro_grid: 12,
            out_dir: "out".to_string(),
        },
        nesting: Degradation::default(),
        horizon: Horizon::default(),
    }
}

/// Run the reference universe and reduce it to one number.
///
/// Deliberately does not go through [`crate::experiment::run`]: that records
/// traces and timings which are not part of what is being compared. This is the
/// smallest loop that still exercises observation, rendering, collapse and
/// physics.
pub fn golden_fingerprint() -> u64 {
    let cfg = golden_config();
    let res = Resolved::new(&cfg.constraints, &cfg.params);
    let geom = Geometry::new(
        cfg.world.width,
        cfg.world.height,
        res.subdivision,
        res.block_size,
    );

    let mut world = World::seed(geom, cfg.world.seed, cfg.world.init_density);
    for t in 0..cfg.world.ticks {
        let (observed, _) = observe(&world, &cfg.observer, t, cfg.world.seed, res.lazy);
        let (advanced, _) = tick(&observed, &cfg.rules, &res);
        world = advanced;
    }
    world.fingerprint()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fingerprint_is_stable_within_a_run() {
        assert_eq!(golden_fingerprint(), golden_fingerprint());
    }

    #[test]
    fn the_native_target_matches_the_pinned_constant() {
        // The WebAssembly suite asserts this same constant. Neither target ever
        // sees the other's answer; they agree by both matching this, or they do
        // not agree at all.
        assert_eq!(
            golden_fingerprint(),
            GOLDEN_FINGERPRINT,
            "the reference universe changed; if that was deliberate, update the constant"
        );
    }

    #[test]
    fn the_fingerprint_notices_a_changed_state() {
        // A hash that never disagrees would pass the cross-target test for the
        // wrong reason.
        let cfg = golden_config();
        let res = Resolved::new(&cfg.constraints, &cfg.params);
        let geom = Geometry::new(
            cfg.world.width,
            cfg.world.height,
            res.subdivision,
            res.block_size,
        );
        let a = World::seed(geom, GOLDEN_SEED, 0.3);
        let mut b = a.clone();
        assert_eq!(a.fingerprint(), b.fingerprint());

        b.cells[0] ^= 1;
        assert_ne!(a.fingerprint(), b.fingerprint(), "one cell must move it");

        let mut c = a.clone();
        c.coarse[0] += 1e-12;
        assert_ne!(
            a.fingerprint(),
            c.fingerprint(),
            "densities fold in by bit pattern, so a tiny change must move it"
        );

        let mut d = a.clone();
        d.resolved[0] = !d.resolved[0];
        assert_ne!(
            a.fingerprint(),
            d.fingerprint(),
            "fidelity is part of state"
        );
    }

    #[test]
    fn a_different_seed_gives_a_different_fingerprint() {
        let cfg = golden_config();
        let res = Resolved::new(&cfg.constraints, &cfg.params);
        let geom = Geometry::new(
            cfg.world.width,
            cfg.world.height,
            res.subdivision,
            res.block_size,
        );
        let a = World::seed(geom, GOLDEN_SEED, 0.3);
        let b = World::seed(geom, GOLDEN_SEED + 1, 0.3);
        assert_ne!(a.fingerprint(), b.fingerprint());
    }

    #[test]
    fn the_reference_universe_actually_exercises_lazy_rendering() {
        // If the probe covered everything, the golden run would never render or
        // collapse a block and the check would miss a whole class of platform
        // difference.
        let cfg = golden_config();
        assert!(cfg.observer.width < cfg.world.width);
        assert!(
            cfg.observer.coverage(cfg.world.width, cfg.world.height) < 0.5,
            "most of the reference world should be coarse-grained"
        );
    }
}
