//! Theory 6 end to end, using the shipped `configs/sweep.toml`.

use std::path::Path;

use universe_core::config::Config;
use universe_core::report;
use universe_core::sweep::{self, CONWAY_BIRTH_CENTRE, CONWAY_SURVIVE_CENTRE, TOLERANCE};

fn cfg() -> Config {
    Config::load(Path::new("configs/sweep.toml")).expect("shipped config must load")
}

/// Coarser than the shipped run. The claims here are about the shape of the
/// result, not its exact resolution, and 441 universes is a lot for a test.
const STEPS: usize = 9;

fn swept() -> sweep::Sweep {
    sweep::run_sweep(&cfg(), STEPS, 0.05, 0.65, |_, _| {})
}

#[test]
fn the_reference_clears_the_bar_it_sets() {
    // If Conway fails its own calibration the measurement is broken, and every
    // other number in the sweep is meaningless.
    let s = swept();
    assert!(s.reference_is_admitted(), "reference: {:?}", s.reference);
}

#[test]
fn chaos_is_not_counted_as_complexity() {
    // Complexity sits between order and chaos. A rule that rewrites the world
    // every tick must not clear a bar meant for Conway.
    let c = cfg();
    let (bar, conway) = sweep::calibrate(&c);
    let churner = sweep::evaluate(&c, 0.05, 0.05, Some(&bar));
    assert!(churner.activity > conway.activity * TOLERANCE);
    assert!(!churner.complex);
}

#[test]
fn empty_and_saturated_rules_are_not_complex() {
    let c = cfg();
    let (bar, _) = sweep::calibrate(&c);
    let dead = sweep::evaluate(&c, 1.5, -0.5, Some(&bar));
    assert_eq!(dead.final_live, 0.0);
    assert!(!dead.complex);
}

#[test]
fn the_productive_band_is_a_minority() {
    // The core of Theory 6. It does not require the band to be a sliver -- the
    // measured answer is what it is -- but if most laws were productive the
    // fine-tuning claim would simply be false here, and the report says so.
    let s = swept();
    assert!(
        s.productive_rule_fraction() < 0.5,
        "productive fraction was {:.3}",
        s.productive_rule_fraction()
    );
    assert!(
        s.productive_rule_fraction() > 0.0,
        "if nothing is productive the bar is wrong, not the universe"
    );
}

#[test]
fn the_grid_collapses_onto_far_fewer_laws() {
    // Only k/8 densities occur, so a continuous sweep is coarser than it looks.
    let s = swept();
    assert!(
        s.distinct_rules() < s.grid.len(),
        "{} laws from {} settings",
        s.distinct_rules(),
        s.grid.len()
    );
}

#[test]
fn the_conway_point_is_productive() {
    let c = cfg();
    let (bar, _) = sweep::calibrate(&c);
    let o = sweep::evaluate(&c, CONWAY_BIRTH_CENTRE, CONWAY_SURVIVE_CENTRE, Some(&bar));
    assert!(o.complex, "the reference law must be productive: {o:?}");
}

#[test]
fn a_sweep_is_reproducible() {
    assert_eq!(
        report::sweep_to_csv(&swept()),
        report::sweep_to_csv(&swept())
    );
}

#[test]
fn the_csv_has_one_row_per_setting() {
    let s = swept();
    assert_eq!(report::sweep_to_csv(&s).lines().count(), s.grid.len() + 1);
}

#[test]
fn the_summary_states_what_it_cannot_show() {
    let text = report::sweep_summary(&swept());
    assert!(text.contains("a measure of resemblance, not of worth"));
    assert!(text.contains("area is the resolution of the sweep"));
}
