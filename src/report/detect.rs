//! Detection's report: every test with its error rates, the controls, and what
//! none of it shows.

use super::{Written, json_f64, seed_list};
use crate::detector::{Direction, Gaze, LIMITS, STATISTICS, Survey, Test};
use std::fmt::Write as _;
use std::io;
use std::path::{Path, PathBuf};

/// The sentence every detection verdict ends with. A test enforces it.
pub const FRAMING: &str = "none of this tells an inhabitant whether it is simulated. it tells it \
which of its own laws have the shape of an optimization, and with what error rate.\n";

fn interval(k: usize, n: usize) -> String {
    if n == 0 {
        return "n/a".to_string();
    }
    let (lo, hi) = crate::stats::wilson(k, n);
    format!("{k}/{n} [{lo:.2}, {hi:.2}]")
}

fn direction(d: Direction) -> &'static str {
    match d {
        Direction::Above => "above",
        Direction::Below => "below",
        Direction::None => "none",
    }
}

fn test_row(t: &Test) -> String {
    format!(
        "{:<16} {:<16} {:>9.4} {:>9.4} {:<6} {:>9} {:>16} {:>16} {:>6.2} {:<12}",
        t.limit,
        t.statistic,
        t.mean_h0,
        t.mean_h1,
        direction(t.rule.direction),
        if t.rule.threshold.is_finite() {
            format!("{:.4}", t.rule.threshold)
        } else {
            "-".to_string()
        },
        interval(t.outcome.false_positives, t.outcome.n_h0),
        interval(t.outcome.true_positives, t.outcome.n_h1),
        t.outcome.balanced_accuracy(),
        t.standing.label(),
    )
}

fn header() -> String {
    format!(
        "{:<16} {:<16} {:>9} {:>9} {:<6} {:>9} {:>16} {:>16} {:>6} {:<12}",
        "limit",
        "statistic",
        "mean H0",
        "mean H1",
        "dir",
        "threshold",
        "FPR k/n [95%]",
        "power k/n [95%]",
        "acc",
        "standing"
    )
}

/// The whole survey, both gazes, with the controls.
pub fn summary(s: &Survey) -> String {
    let mut out = String::new();
    let seeds: Vec<u64> = s.evidence.iter().map(|e| e.seed).collect();
    let _ = writeln!(
        out,
        "seeds: {} ({}); the first {} calibrate each rule, the remaining {} evaluate it",
        seeds.len(),
        seed_list(&seeds),
        s.n_cal,
        s.n_eval()
    );
    let _ = writeln!(
        out,
        "H0 is the universe with one limit relaxed, H1 the universe with every limit in force.\n\
         a rule flags H1 when the statistic lies beyond the most extreme H0 value seen in\n\
         calibration, so its expected false-positive rate is 1/(n_cal+1) = {:.2}. FPR and power\n\
         are measured on seeds the rule never saw; 'acc' is the balanced accuracy of a blinded\n\
         classifier, against 0.50 for chance.\n",
        if s.n_cal == 0 {
            f64::NAN
        } else {
            1.0 / (s.n_cal + 1) as f64
        }
    );

    for gaze in Gaze::ALL {
        let _ = writeln!(out, "== {} ==", gaze.label());
        let _ = writeln!(out, "{}", header());
        let _ = writeln!(out, "{}", "-".repeat(132));
        for limit in LIMITS {
            for (name, _) in STATISTICS {
                if let Some(t) = s.test(limit, name, gaze) {
                    let _ = writeln!(out, "{}", test_row(t));
                }
            }
        }
        let _ = writeln!(out, "{}", "-".repeat(132));
        let _ = writeln!(
            out,
            "negative control: every limit at this seed against every limit at the next seed"
        );
        for t in s.negative_control.iter().filter(|t| t.gaze == gaze) {
            let _ = writeln!(out, "{}", test_row(t));
        }
        let _ = writeln!(out, "{}", "-".repeat(132));
        let _ = writeln!(
            out,
            "resample control: the lazy-rendering rule applied to a fully rendered universe whose\n\
             unobserved blocks are redrawn from their own density each tick"
        );
        let _ = writeln!(
            out,
            "{:<16} {:>22} {:>22}",
            "statistic", "flags lazy k/n", "flags resample k/n"
        );
        for r in s.resample.iter().filter(|r| r.gaze == gaze) {
            let _ = writeln!(
                out,
                "{:<16} {:>22} {:>22}",
                r.statistic,
                interval(r.lazy_flagged, r.n),
                interval(r.resample_flagged, r.n)
            );
        }
        out.push('\n');
    }

    let _ = writeln!(out, "notes on standing:");
    let mut seen = Vec::new();
    for t in &s.tests {
        let note = t.standing.note();
        if !note.is_empty() && !seen.contains(&note) {
            seen.push(note);
            let _ = writeln!(out, "  {} -- {}", t.standing.label(), note);
        }
    }
    out.push('\n');

    if let Some(iso) = s.isotropy {
        let _ = writeln!(
            out,
            "isotropy at the pinned seed: the lattice reads {:.4}, a finer lattice {:.4}, against {:.1} for a\n\
             continuum. both are the Moore neighbourhood's geometry (a consequence); what had to be run is\n\
             only that natural births reach the corner, which they did in every universe measured.\n",
            iso.lattice, iso.finer, iso.continuum
        );
    }

    out.push_str(&verdict(s));
    out
}

fn verdict(s: &Survey) -> String {
    let mut out = String::new();
    let ceiling = s.control_ceiling();
    let _ = writeln!(
        out,
        "the negative control's highest power is {ceiling:.2}: any power below that is detecting seeds."
    );
    for gaze in Gaze::ALL {
        let mut found = Vec::new();
        let mut missed = Vec::new();
        for limit in LIMITS {
            match s.best(limit, gaze) {
                Some(t) if t.outcome.tpr() > ceiling.max(t.outcome.fpr()) + 0.25 => {
                    found.push(format!(
                        "{} by {} (power {:.2}, FPR {:.2}, {})",
                        limit,
                        t.statistic,
                        t.outcome.tpr(),
                        t.outcome.fpr(),
                        t.standing.label()
                    ));
                }
                _ => missed.push(limit.to_string()),
            }
        }
        let _ = writeln!(out, "{}:", gaze.label());
        if !found.is_empty() {
            let _ = writeln!(out, "  findable: {}", found.join("; "));
        }
        if !missed.is_empty() {
            let _ = writeln!(
                out,
                "  not findable by any statistic here: {}",
                missed.join(", ")
            );
        }
    }
    let _ = writeln!(
        out,
        "'findable' means power at least 0.25 above both the false-positive rate and the\n\
         negative control. a limit not found here may be findable by a statistic nobody has\n\
         written; a limit found by a consequence was never in doubt."
    );
    out.push_str(FRAMING);
    out
}

const TEST_COLUMNS: &[&str] = &[
    "kind",
    "gaze",
    "limit",
    "statistic",
    "standing",
    "mean_h0",
    "mean_h1",
    "direction",
    "threshold",
    "n_cal",
    "n_h0",
    "n_h1",
    "false_positives",
    "true_positives",
    "fpr",
    "tpr",
    "balanced_accuracy",
];

fn csv_row(kind: &str, t: &Test) -> String {
    format!(
        "{kind},{},{},{},{},{:.6},{:.6},{},{:.6},{},{},{},{},{},{:.6},{:.6},{:.6}",
        t.gaze.label(),
        t.limit,
        t.statistic,
        t.standing.label(),
        t.mean_h0,
        t.mean_h1,
        direction(t.rule.direction),
        t.rule.threshold,
        t.rule.n_cal,
        t.outcome.n_h0,
        t.outcome.n_h1,
        t.outcome.false_positives,
        t.outcome.true_positives,
        t.outcome.fpr(),
        t.outcome.tpr(),
        t.outcome.balanced_accuracy(),
    )
}

/// One row per test, including the negative control.
pub fn to_csv(s: &Survey) -> String {
    let mut out = TEST_COLUMNS.join(",");
    out.push('\n');
    for t in &s.tests {
        let _ = writeln!(out, "{}", csv_row("test", t));
    }
    for t in &s.negative_control {
        let _ = writeln!(out, "{}", csv_row("negative_control", t));
    }
    out
}

const EVIDENCE_COLUMNS: &[&str] = &[
    "seed",
    "gaze",
    "condition",
    "influence_speed",
    "smoothness",
    "axis_reach",
    "diagonal_reach",
    "anisotropy",
    "edge_birth_rate",
    "interior_birth_rate",
    "edge_excess",
    "samples",
];

/// Every raw measurement, one row per seed, gaze and condition, so the tests
/// can be re-run with a different split or rule.
pub fn evidence_to_csv(s: &Survey) -> String {
    let mut out = EVIDENCE_COLUMNS.join(",");
    out.push('\n');
    for se in &s.evidence {
        for (gaze, label, e) in &se.rows {
            let _ = writeln!(
                out,
                "{},{},{},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{}",
                se.seed,
                gaze.label(),
                label,
                e.influence_speed,
                e.smoothness,
                e.axis_reach,
                e.diagonal_reach,
                e.anisotropy(),
                e.edge_birth_rate,
                e.interior_birth_rate,
                e.edge_excess(),
                e.samples
            );
        }
    }
    out
}

fn test_json(t: &Test) -> String {
    format!(
        "{{\"gaze\": \"{}\", \"limit\": \"{}\", \"statistic\": \"{}\", \"standing\": \"{}\", \"note\": \"{}\", \
         \"mean_h0\": {}, \"mean_h1\": {}, \"direction\": \"{}\", \"threshold\": {}, \"n_cal\": {}, \
         \"n_h0\": {}, \"n_h1\": {}, \"false_positives\": {}, \"true_positives\": {}, \"fpr\": {}, \"tpr\": {}, \"balanced_accuracy\": {}}}",
        t.gaze.label(),
        t.limit,
        t.statistic,
        t.standing.label(),
        t.standing.note(),
        json_f64(t.mean_h0),
        json_f64(t.mean_h1),
        direction(t.rule.direction),
        json_f64(t.rule.threshold),
        t.rule.n_cal,
        t.outcome.n_h0,
        t.outcome.n_h1,
        t.outcome.false_positives,
        t.outcome.true_positives,
        json_f64(t.outcome.fpr()),
        json_f64(t.outcome.tpr()),
        json_f64(t.outcome.balanced_accuracy()),
    )
}

pub fn to_json(s: &Survey) -> String {
    let tests: Vec<String> = s
        .tests
        .iter()
        .map(|t| format!("    {}", test_json(t)))
        .collect();
    let control: Vec<String> = s
        .negative_control
        .iter()
        .map(|t| format!("    {}", test_json(t)))
        .collect();
    let resample: Vec<String> = s
        .resample
        .iter()
        .map(|r| {
            format!(
                "    {{\"gaze\": \"{}\", \"statistic\": \"{}\", \"lazy_flagged\": {}, \"resample_flagged\": {}, \"n\": {}}}",
                r.gaze.label(),
                r.statistic,
                r.lazy_flagged,
                r.resample_flagged,
                r.n
            )
        })
        .collect();
    let iso = s.isotropy.map_or("null".to_string(), |i| {
        format!(
            "{{\"lattice\": {}, \"finer\": {}, \"continuum\": {}, \"standing\": \"consequence\"}}",
            json_f64(i.lattice),
            json_f64(i.finer),
            json_f64(i.continuum)
        )
    });
    let seeds: Vec<String> = s.evidence.iter().map(|e| e.seed.to_string()).collect();
    format!(
        "{{\n  \"seeds\": [{}],\n  \"n_cal\": {},\n  \"n_eval\": {},\n  \"control_ceiling\": {},\n  \"tests\": [\n{}\n  ],\n  \"negative_control\": [\n{}\n  ],\n  \"resample\": [\n{}\n  ],\n  \"isotropy\": {}\n}}\n",
        seeds.join(", "),
        s.n_cal,
        s.n_eval(),
        json_f64(s.control_ceiling()),
        tests.join(",\n"),
        control.join(",\n"),
        resample.join(",\n"),
        iso
    )
}

/// `detection.csv`, `detection.json` and `evidence.csv`.
pub fn write(s: &Survey, out_dir: &Path) -> io::Result<Written> {
    std::fs::create_dir_all(out_dir)?;
    let csv: PathBuf = out_dir.join("detection.csv");
    let json: PathBuf = out_dir.join("detection.json");
    std::fs::write(&csv, to_csv(s))?;
    std::fs::write(&json, to_json(s))?;
    std::fs::write(out_dir.join("evidence.csv"), evidence_to_csv(s))?;
    Ok(Written { csv, json })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::Degradation;
    use crate::config::{Config, ReportCfg, WorldCfg};
    use crate::constraints::{Constraints, Params};
    use crate::detector::{Inhabitant, Standing, survey};
    use crate::observer::Probe;
    use crate::physics::Rules;

    fn small_survey() -> Survey {
        let c = Config {
            world: WorldCfg {
                width: 48,
                height: 48,
                ticks: 16,
                seed: 42,
                init_density: 0.3,
                seeds: 4,
                seed_stride: 1000,
            },
            rules: Rules::default(),
            constraints: Constraints::ALL_ON,
            params: Params {
                block_size: 16,
                ..Params::default()
            },
            observer: Probe {
                x: 0,
                y: 0,
                width: 24,
                height: 24,
            },
            report: ReportCfg {
                macro_grid: 8,
                out_dir: "out".into(),
            },
            nesting: Degradation::default(),
            horizon: crate::pipe::Horizon::default(),
        };
        let who = Inhabitant {
            x: 12,
            y: 12,
            width: 24,
            height: 24,
        };
        survey(&c, &who, |_| {})
    }

    #[test]
    fn the_summary_ends_with_the_framing_and_names_both_controls() {
        let s = summary(&small_survey());
        assert!(s.ends_with(FRAMING));
        assert!(s.contains("negative control"));
        assert!(s.contains("resample control"));
        assert!(s.contains("consequence"));
    }

    #[test]
    fn csvs_are_rectangular_and_the_json_balanced() {
        let sv = small_survey();
        let csv = to_csv(&sv);
        for line in csv.lines() {
            assert_eq!(line.split(',').count(), TEST_COLUMNS.len(), "{line}");
        }
        assert_eq!(csv.lines().count(), 1 + 32 + 8);
        let ev = evidence_to_csv(&sv);
        for line in ev.lines() {
            assert_eq!(line.split(',').count(), EVIDENCE_COLUMNS.len(), "{line}");
        }
        assert_eq!(ev.lines().count(), 1 + 4 * 2 * 7);
        let j = to_json(&sv);
        assert_eq!(
            j.chars().filter(|c| *c == '{').count(),
            j.chars().filter(|c| *c == '}').count()
        );
        assert!(j.contains("\"isotropy\""));
    }

    #[test]
    fn standing_notes_are_printed_once_each() {
        let s = summary(&small_survey());
        let note = Standing::Exploratory("").label();
        assert!(s.matches(&format!("  {note} -- ")).count() <= 1);
    }
}
