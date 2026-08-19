//! The project's first rule, checked end to end: same seed, same universe.
//!
//! The unit tests cover determinism per component. These run the whole
//! experiment — every constraint setting, the control, the report — and
//! compare the artefacts a user would actually look at.
//!
//! Wall time is excluded throughout. It is a measurement, not a counter, and
//! it is the one number in the output that is *supposed* to vary.

use the_universe::budget::Degradation;
use the_universe::config::{Config, ReportCfg, WorldCfg};
use the_universe::constraints::{Constraints, Params};
use the_universe::experiment::{run, run_all};
use the_universe::observer::Probe;
use the_universe::physics::Rules;
use the_universe::report;

fn cfg(seed: u64) -> Config {
    Config {
        world: WorldCfg {
            width: 48,
            height: 48,
            ticks: 25,
            seed,
            init_density: 0.3,
        },
        rules: Rules::default(),
        constraints: Constraints::ALL_ON,
        params: Params {
            block_size: 8,
            ..Params::default()
        },
        observer: Probe {
            x: 8,
            y: 8,
            width: 24,
            height: 24,
        },
        report: ReportCfg {
            macro_grid: 8,
            out_dir: "out".into(),
        },
        nesting: Degradation::default(),
        horizon: the_universe::pipe::Horizon::default(),
    }
}

/// Drop the two wall-time-derived columns: `wall_ms` and `time_ratio`.
fn without_timing(csv: &str) -> String {
    const WALL_MS: usize = 11;
    const TIME_RATIO: usize = 15;
    csv.lines()
        .map(|line| {
            line.split(',')
                .enumerate()
                .filter(|(i, _)| *i != WALL_MS && *i != TIME_RATIO)
                .map(|(_, f)| f)
                .collect::<Vec<_>>()
                .join(",")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_whole_experiment_is_reproducible() {
    let a = run_all(&cfg(42), |_| {});
    let b = run_all(&cfg(42), |_| {});

    assert_eq!(a.chaos_floor, b.chaos_floor);
    assert_eq!(a.chaos_floor_live, b.chaos_floor_live);
    assert_eq!(a.reference.work, b.reference.work);
    assert_eq!(a.reference.macro_trace, b.reference.macro_trace);
    assert_eq!(a.reference.live_trace, b.reference.live_trace);

    assert_eq!(a.comparisons.len(), b.comparisons.len());
    for (x, y) in a.comparisons.iter().zip(b.comparisons.iter()) {
        assert_eq!(x.run.label, y.run.label);
        assert_eq!(x.run.work, y.run.work, "work differed for {}", x.run.label);
        assert_eq!(x.run.macro_trace, y.run.macro_trace);
        assert_eq!(x.run.peak_live_bytes, y.run.peak_live_bytes);
        assert_eq!(x.mean_divergence, y.mean_divergence);
        assert_eq!(x.live_delta, y.live_delta);
    }
}

#[test]
fn the_csv_is_byte_identical_apart_from_timing() {
    let a = report::to_csv(&run_all(&cfg(42), |_| {}));
    let b = report::to_csv(&run_all(&cfg(42), |_| {}));
    assert_eq!(without_timing(&a), without_timing(&b));
}

#[test]
fn a_different_seed_gives_a_different_universe() {
    let a = report::to_csv(&run_all(&cfg(42), |_| {}));
    let b = report::to_csv(&run_all(&cfg(99), |_| {}));
    assert_ne!(
        without_timing(&a),
        without_timing(&b),
        "the seed is the creator's only intervention; it must matter"
    );
}

#[test]
fn every_constraint_setting_is_reproducible_on_its_own() {
    let c = cfg(7);
    let mut settings = Constraints::singles();
    settings.push(Constraints::ALL_ON);
    settings.push(Constraints::ALL_OFF);
    for s in settings {
        let x = run(&c, s);
        let y = run(&c, s);
        assert_eq!(x.work, y.work, "{}", s.label());
        assert_eq!(x.macro_trace, y.macro_trace, "{}", s.label());
    }
}

#[test]
fn timing_columns_are_the_only_ones_allowed_to_move() {
    // Guards the guard: if the column layout shifts, `without_timing` would
    // silently start masking a real field.
    let csv = report::to_csv(&run_all(&cfg(42), |_| {}));
    let header: Vec<&str> = csv.lines().next().unwrap().split(',').collect();
    assert_eq!(header[11], "wall_ms");
    assert_eq!(header[15], "time_ratio");
}
