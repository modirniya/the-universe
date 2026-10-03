//! Detection end to end, using the shipped `configs/detect.toml` at a reduced
//! seed count.
//!
//! Scale matters here in a way it does not elsewhere. An inhabitant measures
//! the speed of influence by watching how far new life appears from old, and
//! that takes a universe with enough room and enough history to actually
//! produce the extreme case. The unit tests run on a small world and can only
//! check the bound; these run on the shipped one and can check what is reached.
//!
//! What is pinned here is what follows from the definitions, plus the
//! machinery's own invariants. What had to be run — power, false-positive rate,
//! the exploratory edge statistic — is read from the artifacts and the claims
//! ledger, not asserted here, because a test that pins a measured number is a
//! test that the number has not changed, not that it is right.

use std::path::Path;

use universe_core::config::Config;
use universe_core::constraints::Constraints;
use universe_core::detector::{self, Condition, Direction, Gaze, Inhabitant, Standing};
use universe_core::report;

fn cfg() -> Config {
    let mut c = Config::load(Path::new("configs/detect.toml")).expect("shipped config must load");
    // Eight seeds: four calibrate, four evaluate. Enough to exercise every path
    // in a debug build; the shipped run uses forty.
    c.world.seeds = 8;
    c
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
    detector::investigate(
        c,
        Condition {
            constraints: k,
            gaze: Gaze::Rendering,
            resample: false,
        },
        &who(c),
    )
    .influence_speed
}

#[test]
fn the_speed_cap_is_findable_from_inside() {
    let c = cfg();
    assert!(speed(&c, detector::without("speed_cap")) > speed(&c, Constraints::ALL_ON));
}

#[test]
fn coarse_time_and_a_loose_cap_read_identically() {
    // The v0.1 coupling as a limit on knowledge: the inhabitant measures the
    // product radius * substeps and cannot factor it. A consequence, checked.
    let mut c = cfg();
    c.params.substeps = 3;
    c.params.uncapped_radius = 3;
    assert_eq!(
        speed(&c, detector::without("discrete_time")),
        speed(&c, detector::without("speed_cap"))
    );
}

#[test]
fn the_reading_never_exceeds_its_bound() {
    let mut c = cfg();
    for substeps in [1usize, 2, 3, 4] {
        c.params.substeps = substeps;
        let observed = speed(&c, detector::without("discrete_time"));
        assert!(
            observed <= substeps as f64,
            "{substeps} substeps read {observed}"
        );
    }
}

#[test]
fn the_shape_of_the_lattice_reads_sqrt_two_at_both_scales() {
    // Geometry, not a discovery: the corner of a Moore neighbourhood is sqrt(2)
    // further than its edge whatever the lattice spacing. What had to be run is
    // that natural births reach the corner at all.
    let c = cfg();
    let s = detector::survey(&c, &who(&c), |_| {});
    let iso = s.isotropy.expect("the pinned seed must be measured");
    assert!((iso.lattice - std::f64::consts::SQRT_2).abs() < 1e-9);
    assert!((iso.finer - std::f64::consts::SQRT_2).abs() < 1e-9);
    assert_eq!(iso.continuum, 1.0);
}

#[test]
fn the_survey_calibrates_and_evaluates_on_disjoint_seeds() {
    let c = cfg();
    let s = detector::survey(&c, &who(&c), |_| {});
    assert_eq!(s.evidence.len(), 8);
    assert_eq!(s.n_cal, 4);
    assert_eq!(s.n_eval(), 4);
    for t in &s.tests {
        assert!(t.rule.n_cal <= s.n_cal);
        assert!(t.outcome.n_h0 <= s.n_eval() && t.outcome.n_h1 <= s.n_eval());
    }
}

#[test]
fn consequences_are_labelled_and_exact() {
    // The speed cap and discrete time move influence_speed by construction, and
    // a statistic that is the radius cannot miss or false-alarm.
    let c = cfg();
    let s = detector::survey(&c, &who(&c), |_| {});
    for limit in ["speed_cap", "discrete_time"] {
        let t = s.test(limit, "influence_speed", Gaze::Rendering).unwrap();
        assert!(matches!(t.standing, Standing::Consequence(_)));
        assert_eq!(t.mean_h1, 1.0, "{limit}: capped reach is 1 by construction");
        assert_eq!(t.outcome.tpr(), 1.0, "{limit}");
        // The false-positive rate is read from the artifacts: whether the
        // uncapped reach saturates is region-dependent, not a consequence.
    }
    // The lattice's scale cannot be read by its shape: the same sqrt(2) on both
    // sides gives a rule with no direction.
    let t = s
        .test("discrete_space", "anisotropy", Gaze::Rendering)
        .unwrap();
    assert!(matches!(t.standing, Standing::Consequence(_)));
    assert_eq!(t.rule.direction, Direction::None);
    assert_eq!(t.outcome.tpr(), 0.0);
}

#[test]
fn the_edge_statistic_is_marked_exploratory() {
    let c = cfg();
    let s = detector::survey(&c, &who(&c), |_| {});
    let t = s
        .test("lazy_rendering", "edge_excess", Gaze::Rendering)
        .unwrap();
    assert!(matches!(t.standing, Standing::Exploratory(_)));
}

#[test]
fn the_survey_is_reproducible() {
    let c = cfg();
    let a = report::detect::to_csv(&detector::survey(&c, &who(&c), |_| {}));
    let b = report::detect::to_csv(&detector::survey(&c, &who(&c), |_| {}));
    assert_eq!(a, b);
}

#[test]
fn the_report_refuses_the_bigger_claim() {
    let c = cfg();
    let text = report::detect::summary(&detector::survey(&c, &who(&c), |_| {}));
    assert!(
        text.contains("none of this tells an inhabitant whether it is simulated"),
        "detection must not be presented as evidence of simulation"
    );
    assert!(text.contains("negative control"));
}
