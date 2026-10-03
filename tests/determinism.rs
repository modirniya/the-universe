//! The project's first rule, checked end to end: same seed, same universe.
//!
//! The unit tests cover determinism per component. These run the whole
//! Theory 1 experiment — every setting, every null, the ablation, the report —
//! and compare the artefacts a user would actually look at.
//!
//! Wall time is excluded throughout. It is a measurement, not a counter, and
//! it is the one number in the output that is *supposed* to vary.

use universe_core::budget::Degradation;
use universe_core::config::{Config, ReportCfg, WorldCfg};
use universe_core::constraints::{Constraints, Params};
use universe_core::experiment::run;
use universe_core::limits::{run_factorial, settings};
use universe_core::observer::Probe;
use universe_core::physics::Rules;
use universe_core::report;

fn cfg(seed: u64) -> Config {
    Config {
        world: WorldCfg {
            width: 48,
            height: 48,
            ticks: 25,
            seed,
            init_density: 0.3,
            seeds: 1,
            seed_stride: 1000,
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
        horizon: universe_core::pipe::Horizon::default(),
    }
}

/// Drop the two wall-time-derived columns.
fn without_timing(csv: &str) -> String {
    let header: Vec<&str> = csv.lines().next().unwrap_or("").split(',').collect();
    let skip: Vec<usize> = header
        .iter()
        .enumerate()
        .filter(|(_, h)| **h == "wall_ms" || **h == "time_ratio")
        .map(|(i, _)| i)
        .collect();
    csv.lines()
        .map(|line| {
            line.split(',')
                .enumerate()
                .filter(|(i, _)| !skip.contains(i))
                .map(|(_, f)| f)
                .collect::<Vec<_>>()
                .join(",")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_whole_experiment_is_reproducible() {
    let a = run_factorial(&cfg(42), |_| {});
    let b = run_factorial(&cfg(42), |_| {});

    assert_eq!(a.reference_profile, b.reference_profile);
    for (x, y) in a.cells.iter().zip(&b.cells) {
        assert_eq!(x.label, y.label);
        assert_eq!(x.work, y.work, "work differed for {}", x.label);
        assert_eq!(x.div_half, y.div_half);
        assert_eq!(x.div_final, y.div_final);
        assert_eq!(x.profile, y.profile);
        assert_eq!(x.peak_live_bytes, y.peak_live_bytes);
        assert_eq!(x.trace, y.trace);
    }
    for (x, y) in a.nulls.iter().zip(&b.nulls) {
        assert_eq!(x.div_half, y.div_half, "null {}", x.label);
        assert_eq!(x.trace, y.trace);
    }
    for (x, y) in a.ablation.iter().zip(&b.ablation) {
        assert_eq!(x.div_half, y.div_half, "ablation {}", x.label);
        assert_eq!(x.profile, y.profile);
    }
}

#[test]
fn the_csv_is_byte_identical_apart_from_timing() {
    let a = report::limits::to_csv(&run_factorial(&cfg(42), |_| {}));
    let b = report::limits::to_csv(&run_factorial(&cfg(42), |_| {}));
    assert_eq!(without_timing(&a), without_timing(&b));
}

#[test]
fn a_different_seed_gives_a_different_universe() {
    let a = report::limits::to_csv(&run_factorial(&cfg(42), |_| {}));
    let b = report::limits::to_csv(&run_factorial(&cfg(99), |_| {}));
    assert_ne!(
        without_timing(&a),
        without_timing(&b),
        "the seed is the creator's only intervention; it must matter"
    );
}

#[test]
fn every_constraint_setting_is_reproducible_on_its_own() {
    let c = cfg(7);
    for s in settings() {
        let x = run(&c, s);
        let y = run(&c, s);
        assert_eq!(x.work, y.work, "{}", s.label());
        assert_eq!(x.macro_trace, y.macro_trace, "{}", s.label());
        assert_eq!(x.profile, y.profile, "{}", s.label());
    }
}

#[test]
fn timing_columns_are_the_only_ones_allowed_to_move() {
    // Guards the guard: the two columns `without_timing` drops must exist, and
    // nothing else in the CSV may differ between runs.
    let csv = report::limits::to_csv(&run_factorial(&cfg(42), |_| {}));
    let header: Vec<&str> = csv.lines().next().unwrap().split(',').collect();
    assert!(header.contains(&"wall_ms"));
    assert!(header.contains(&"time_ratio"));
    let kept = without_timing(&csv);
    assert_eq!(
        kept.lines().next().unwrap().split(',').count(),
        header.len() - 2
    );
}
