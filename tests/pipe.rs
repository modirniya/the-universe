//! Theory 3 end to end, using the shipped `configs/pipe.toml`.
//!
//! The claim has two halves and they pull in opposite directions: content
//! structure must *not* survive serialization, and timing and magnitude must.
//! A pipe that preserved everything would not be a pipe, and one that preserved
//! nothing would carry no signal at all. Both halves are checked here.

use std::path::Path;

use the_universe::config::Config;
use the_universe::pipe::{self, Relay};
use the_universe::report;

fn cfg() -> Config {
    Config::load(Path::new("configs/pipe.toml")).expect("shipped config must load")
}

fn relay(c: &Config) -> Relay {
    pipe::run_relay(c, &c.horizon)
}

#[test]
fn content_structure_does_not_survive_the_crossing() {
    let r = relay(&cfg());
    assert!(
        (0.35..=0.65).contains(&r.content_avalanche),
        "expected a one-cell change to scatter about half the digest bits, got {}",
        r.content_avalanche
    );
}

#[test]
fn timing_and_magnitude_do_survive_it() {
    let r = relay(&cfg());
    let c = r.magnitude_correlation();
    assert!(
        c >= 0.5,
        "what crossed should still track the child; correlation was {c}"
    );
}

#[test]
fn the_channel_is_genuinely_narrow() {
    // If the pipe carried most of the horizon's information, "what survives a
    // bottleneck" would not be a question worth asking.
    let c = cfg();
    assert!(
        c.horizon.compression_ratio() < 0.10,
        "channel carries {:.1}% of the content",
        c.horizon.compression_ratio() * 100.0
    );
}

#[test]
fn one_message_crosses_per_tick() {
    let c = cfg();
    assert_eq!(relay(&c).received.all().len(), c.world.ticks as usize);
}

#[test]
fn raising_the_threshold_never_reveals_more() {
    let r = relay(&cfg());
    let sweep = r.threshold_sweep(pipe::THRESHOLDS);
    for pair in sweep.windows(2) {
        assert!(
            pair[1].visible_fraction <= pair[0].visible_fraction,
            "threshold {} showed more than {}",
            pair[1].threshold,
            pair[0].threshold
        );
    }
}

#[test]
fn a_high_enough_threshold_hides_the_child_completely() {
    let r = relay(&cfg());
    assert_eq!(r.visible_fraction(1.01), 0.0, "nothing registers at all");
}

#[test]
fn thin_rows_report_no_correlation() {
    // The sweep must never present a two-point correlation as a finding.
    let r = relay(&cfg());
    for row in r.threshold_sweep(pipe::THRESHOLDS) {
        if row.samples < pipe::MIN_CORRELATION_SAMPLES {
            assert!(
                row.correlation.is_nan(),
                "threshold {} reported {} from {} events",
                row.threshold,
                row.correlation,
                row.samples
            );
        }
    }
}

#[test]
fn a_relay_is_reproducible() {
    let c = cfg();
    assert_eq!(
        report::pipe_to_csv(&relay(&c)),
        report::pipe_to_csv(&relay(&c))
    );
}

#[test]
fn a_different_seed_sends_a_different_history() {
    let c = cfg();
    let mut other = c.clone();
    other.world.seed = c.world.seed.wrapping_add(1);
    let (a, b) = (relay(&c), relay(&other));
    assert_ne!(
        a.received
            .all()
            .iter()
            .map(|m| m.digest)
            .collect::<Vec<_>>(),
        b.received
            .all()
            .iter()
            .map(|m| m.digest)
            .collect::<Vec<_>>()
    );
}

#[test]
fn the_csv_has_one_row_per_threshold() {
    let csv = report::pipe_to_csv(&relay(&cfg()));
    assert_eq!(csv.lines().count(), pipe::THRESHOLDS.len() + 1);
}

#[test]
fn the_json_is_balanced() {
    let json = report::pipe_to_json(&relay(&cfg()));
    assert_eq!(
        json.chars().filter(|c| *c == '{').count(),
        json.chars().filter(|c| *c == '}').count()
    );
    assert!(json.contains("compression_ratio"));
}

#[test]
fn the_summary_says_the_blindness_is_compiler_enforced() {
    let s = report::pipe_summary(&relay(&cfg()));
    assert!(s.contains("enforced by the compiler"));
}
