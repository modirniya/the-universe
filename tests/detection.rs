//! Detection end to end, using the shipped `configs/detect.toml`.
//!
//! Scale matters here in a way it does not elsewhere. An inhabitant measures
//! the speed of influence by watching how far new life appears from old, and
//! that takes a universe with enough room and enough history to actually
//! produce the extreme case. The unit tests run on a small world and can only
//! check the bound; these run on the shipped one and can check what is reached.

use std::path::Path;

use universe_core::config::Config;
use universe_core::constraints::Constraints;
use universe_core::detector::{self, Gaze, Inhabitant};
use universe_core::report;

fn cfg() -> Config {
    Config::load(Path::new("configs/detect.toml")).expect("shipped config must load")
}

/// Straddling the observed region and the coarse ground beyond it.
fn who(c: &Config) -> Inhabitant {
    Inhabitant {
        x: c.observer.x + c.observer.width / 2,
        y: c.observer.y + c.observer.height / 2,
        width: c.observer.width,
        height: c.observer.height,
    }
}

fn speed(c: &Config, k: Constraints) -> f64 {
    detector::investigate(c, k, &who(c), Gaze::Rendering).influence_speed
}

#[test]
fn the_speed_cap_is_findable_from_inside() {
    let c = cfg();
    let mut loose = Constraints::ALL_ON;
    loose.speed_cap = false;
    assert!(
        speed(&c, loose) > speed(&c, Constraints::ALL_ON),
        "a loosened cap should be measurable"
    );
}

#[test]
fn coarse_time_and_a_loose_cap_read_identically() {
    // The v0.1 coupling as a limit on knowledge. Influence reaches
    // `radius * substeps` cells per tick, and an inhabitant measuring that
    // distance cannot factor it: three substeps of radius one and one substep
    // of radius three are the same number.
    let mut c = cfg();
    c.params.substeps = 3;
    c.params.uncapped_radius = 3;

    let mut coarse_time = Constraints::ALL_ON;
    coarse_time.discrete_time = false;
    let mut fast_light = Constraints::ALL_ON;
    fast_light.speed_cap = false;

    assert_eq!(
        speed(&c, coarse_time),
        speed(&c, fast_light),
        "3x1 and 1x3 are the same product and must read the same"
    );
}

#[test]
fn the_reading_never_exceeds_its_bound() {
    // `radius * substeps` is a ceiling and influence cannot beat it, however
    // the dials are set.
    //
    // Whether the ceiling is *reached* is a different question and not a stable
    // one: influence needs a live chain to carry it, so how close a reading
    // gets depends on the world's size, its history, its density, and where the
    // inhabitant happens to be standing. An earlier version of this test
    // asserted that a generous bound goes unreached, which held at one
    // inhabitant placement and failed at another. The ceiling is the invariant;
    // saturation is a local observation and is reported as one.
    let mut c = cfg();
    for substeps in [1usize, 2, 3, 4] {
        c.params.substeps = substeps;
        let mut coarse_time = Constraints::ALL_ON;
        coarse_time.discrete_time = false;
        let observed = speed(&c, coarse_time);
        assert!(
            observed <= substeps as f64,
            "{substeps} substeps of radius 1 read {observed}, above the ceiling"
        );
    }
}

#[test]
fn pixelation_leaves_no_fingerprint() {
    let c = cfg();
    let findings = detector::investigate_all(&c, &who(&c), Gaze::Rendering);
    let space = findings
        .iter()
        .find(|f| f.limit == "discrete_space")
        .expect("discrete_space must be surveyed");
    assert!(
        !space.detectable,
        "the cell is the ruler; subdividing space should leave it unchanged"
    );
}

#[test]
fn looking_conceals_lazy_rendering() {
    // The observer effect as a measurement. The same inhabitant on the same
    // ground finds coarse-graining when it can read without rendering, and
    // none when its looking renders.
    let c = cfg();
    let rendering = detector::investigate(&c, Constraints::ALL_ON, &who(&c), Gaze::Rendering);
    let passive = detector::investigate(&c, Constraints::ALL_ON, &who(&c), Gaze::Passive);
    assert!(
        passive.smoothness > rendering.smoothness,
        "passive {} should exceed rendering {}",
        passive.smoothness,
        rendering.smoothness
    );
}

#[test]
fn the_survey_is_reproducible() {
    let c = cfg();
    let a = report::detect_to_csv(&detector::investigate_all(&c, &who(&c), Gaze::Rendering));
    let b = report::detect_to_csv(&detector::investigate_all(&c, &who(&c), Gaze::Rendering));
    assert_eq!(a, b);
}

#[test]
fn the_report_covers_both_gazes() {
    let c = cfg();
    let r = detector::investigate_all(&c, &who(&c), Gaze::Rendering);
    let p = detector::investigate_all(&c, &who(&c), Gaze::Passive);
    let text = report::detect_report(&r, &p);
    assert!(text.contains("looking renders") || text.contains("whose looking renders"));
    assert!(text.contains("what the difference shows"));
}

#[test]
fn the_report_refuses_the_bigger_claim() {
    let c = cfg();
    let r = detector::investigate_all(&c, &who(&c), Gaze::Rendering);
    let text = report::detect_summary(&r);
    assert!(
        text.contains("none of this tells an inhabitant whether it is simulated"),
        "detection must not be presented as evidence of simulation"
    );
}
