//! Principle 8, made checkable across platforms.
//!
//! Run with `wasm-pack test --node crates/universe-web`.
//!
//! The native suite asserts `GOLDEN_FINGERPRINT` in `universe_core::golden`.
//! This asserts the same constant after compiling the same code to WebAssembly.
//! Neither target ever sees the other's answer — they agree by both matching the
//! constant, or they do not agree at all.
//!
//! If this fails while the native test passes, something in the value stream is
//! platform-dependent: a float that rounds differently, an integer that wraps
//! differently, an iteration order that is not fixed. That is a finding about
//! the model's first rule, not a flaky test.

use universe_core::golden::{GOLDEN_FINGERPRINT, golden_fingerprint};
use wasm_bindgen_test::*;

#[wasm_bindgen_test]
fn the_wasm_target_matches_the_pinned_constant() {
    assert_eq!(
        golden_fingerprint(),
        GOLDEN_FINGERPRINT,
        "the same seed produced a different universe under WebAssembly"
    );
}

#[wasm_bindgen_test]
fn the_fingerprint_is_stable_within_the_wasm_run() {
    assert_eq!(golden_fingerprint(), golden_fingerprint());
}

#[wasm_bindgen_test]
fn a_simulation_steps_and_stays_deterministic() {
    // The viewer's own path, not just the golden loop: two Sims with the same
    // settings must agree after the same number of ticks.
    let mut a = universe_web::Sim::new(48, 48, 20260818.0, 0.3, 16);
    let mut b = universe_web::Sim::new(48, 48, 20260818.0, 0.3, 16);
    for _ in 0..24 {
        a.step();
        b.step();
    }
    assert_eq!(a.fingerprint(), b.fingerprint());
    assert_eq!(a.tick_count(), 24);
}

#[wasm_bindgen_test]
fn lazy_rendering_leaves_most_blocks_coarse() {
    // If the browser build silently resolved everything, the viewer would be
    // drawing a different universe from the one the CLI reports on.
    let mut sim = universe_web::Sim::new(64, 64, 7.0, 0.3, 16);
    for _ in 0..8 {
        sim.step();
    }
    assert!(
        sim.resolved_blocks() < sim.total_blocks(),
        "{} of {} blocks resolved",
        sim.resolved_blocks(),
        sim.total_blocks()
    );
}
