//! Theory 3 end to end, using the shipped `configs/pipe.toml`.
//!
//! The claim has two halves and they pull in opposite directions: content
//! structure must *not* survive serialization, and timing and magnitude must.
//! A pipe that preserved everything would not be a pipe, and one that preserved
//! nothing would carry no signal at all. Both halves are checked here.

use std::path::Path;

use universe_core::config::Config;
use universe_core::pipe::{self, Relay};
use universe_core::report;

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
            .map(|m| m.digest())
            .collect::<Vec<_>>(),
        b.received
            .all()
            .iter()
            .map(|m| m.digest())
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

#[test]
fn the_widest_channel_is_the_pipe_as_it_always_was() {
    // 128 bits is an exact magnitude and a full digest. The width sweep's
    // widest row and the relay's own read end must agree exactly.
    let r = relay(&cfg());
    let rows = r.width_sweep();
    assert_eq!(rows.len(), pipe::WIDTHS.len());
    let widest = rows.last().unwrap();
    assert_eq!(widest.bits, pipe::MAX_BITS);
    assert_eq!(widest.correlation, r.magnitude_correlation());
}

#[test]
fn a_few_bits_carry_most_of_what_crosses() {
    // Measured, not designed: how narrow the channel can get before the
    // parent's view stops tracking the child.
    let r = relay(&cfg());
    let bits = r.width_for(0.9).expect("some width keeps 90%");
    assert!(bits <= 8, "needed {bits} bits for 90% of the correlation");
    let full = r.magnitude_correlation();
    for w in r.width_sweep().iter().filter(|w| w.bits >= 8) {
        assert!(
            (w.correlation - full).abs() < 0.01,
            "{} bits: {} against {full}",
            w.bits,
            w.correlation
        );
    }
}

#[test]
fn one_bit_carries_nothing_at_these_occupancies() {
    // The horizon is never more than half full, so a one-bit channel sends
    // the same value every tick and there is no correlation to report.
    let r = relay(&cfg());
    let one = r.width_sweep()[0];
    assert_eq!(one.bits, 1);
    assert_eq!(one.levels_seen, 1);
    assert!(one.correlation.is_nan());
}

#[test]
fn the_width_curve_is_a_result_and_is_not_assumed_monotone() {
    // v0.9 pinned "three bits track the child worse than two at seed 42". That
    // was a fact about one seed and one horizon placement, and it stopped being
    // true when the horizon was moved to where the config said it was. What is
    // invariant: every row is a correlation in [-1, 1] or NaN, and the widest
    // row is the pipe's own. Whether the curve dips is read from the artifacts.
    let r = relay(&cfg());
    for w in r.width_sweep() {
        assert!(w.correlation.is_nan() || (-1.0..=1.0).contains(&w.correlation));
        assert!(w.levels_seen >= 1);
    }
}
