//! Turning a run into a claim someone else can check.
//!
//! Every number printed here is either reproducible on any machine (work
//! counters, divergences, cell counts) or explicitly marked as not being so
//! (wall time). Memory is reported twice, because there are two honest answers
//! and only one of them is measured: `peak_live_bytes` is what a
//! resource-honest implementation would hold, `allocated_bytes` is what this
//! one really allocates.
//!
//! JSON and CSV are written by hand rather than through a serializer. The
//! output format is part of the claim, so it is worth being able to read the
//! code that produces it.

use crate::experiment::{Comparison, Experiment};
use std::fmt::Write as _;
use std::io;
use std::path::{Path, PathBuf};

/// Column order for `runs.csv`. Kept in one place so the header and the rows
/// cannot drift apart.
const COLUMNS: &[&str] = &[
    "label",
    "discrete_space",
    "discrete_time",
    "speed_cap",
    "lazy_rendering",
    "fine_cells",
    "influence_speed",
    "neighbor_visits",
    "cell_updates",
    "block_updates",
    "cells_rendered",
    "wall_ms",
    "peak_live_bytes",
    "allocated_bytes",
    "work_ratio",
    "time_ratio",
    "memory_ratio",
    "mean_divergence",
    "final_divergence",
    "live_delta",
    "final_live_fraction",
];

/// Files written by one experiment.
pub struct Written {
    pub csv: PathBuf,
    pub json: PathBuf,
}

pub fn write(exp: &Experiment, out_dir: &Path) -> io::Result<Written> {
    std::fs::create_dir_all(out_dir)?;
    let csv = out_dir.join("runs.csv");
    let json = out_dir.join("report.json");
    std::fs::write(&csv, to_csv(exp))?;
    std::fs::write(&json, to_json(exp))?;
    Ok(Written { csv, json })
}

/// The reference run appears as its own row with ratios of exactly 1 and
/// divergence of exactly 0, so the CSV is self-contained.
pub fn to_csv(exp: &Experiment) -> String {
    let mut s = COLUMNS.join(",");
    s.push('\n');
    let reference = Comparison {
        run: exp.reference.clone(),
        work_ratio: 1.0,
        time_ratio: 1.0,
        memory_ratio: 1.0,
        mean_divergence: 0.0,
        final_divergence: 0.0,
        live_delta: 0.0,
    };
    for c in std::iter::once(&reference).chain(exp.comparisons.iter()) {
        let r = &c.run;
        let _ = writeln!(
            s,
            "{},{},{},{},{},{},{:.6},{},{},{},{},{:.3},{},{},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6}",
            r.label,
            r.constraints.discrete_space,
            r.constraints.discrete_time,
            r.constraints.speed_cap,
            r.constraints.lazy_rendering,
            r.fine_cells,
            r.influence_speed,
            r.work.neighbor_visits,
            r.work.cell_updates,
            r.work.block_updates,
            r.work.cells_rendered,
            r.wall_ms,
            r.peak_live_bytes,
            r.allocated_bytes,
            c.work_ratio,
            c.time_ratio,
            c.memory_ratio,
            c.mean_divergence,
            c.final_divergence,
            c.live_delta,
            r.final_live_fraction,
        );
    }
    s
}

pub fn to_json(exp: &Experiment) -> String {
    let mut s = String::from("{\n");
    let _ = writeln!(
        s,
        "  \"chaos_floor\": {:.6}, \"chaos_floor_live\": {:.6},",
        exp.chaos_floor, exp.chaos_floor_live
    );
    let _ = writeln!(s, "  \"reference\": {},", run_json(&exp.reference, None));
    s.push_str("  \"runs\": [\n");
    for (i, c) in exp.comparisons.iter().enumerate() {
        let _ = write!(s, "    {}", run_json(&c.run, Some(c)));
        if i + 1 < exp.comparisons.len() {
            s.push(',');
        }
        s.push('\n');
    }
    s.push_str("  ]\n}\n");
    s
}

fn run_json(r: &crate::experiment::RunResult, c: Option<&Comparison>) -> String {
    let mut s = String::new();
    let _ = write!(
        s,
        "{{\"label\": \"{}\", \"constraints\": {{\"discrete_space\": {}, \"discrete_time\": {}, \
         \"speed_cap\": {}, \"lazy_rendering\": {}}}, \"fine_cells\": {}, \
         \"influence_speed\": {:.6}, \"neighbor_visits\": {}, \"cell_updates\": {}, \
         \"block_updates\": {}, \"cells_rendered\": {}, \"wall_ms\": {:.3}, \
         \"peak_live_bytes\": {}, \"allocated_bytes\": {}, \"final_live_fraction\": {:.6}",
        r.label,
        r.constraints.discrete_space,
        r.constraints.discrete_time,
        r.constraints.speed_cap,
        r.constraints.lazy_rendering,
        r.fine_cells,
        r.influence_speed,
        r.work.neighbor_visits,
        r.work.cell_updates,
        r.work.block_updates,
        r.work.cells_rendered,
        r.wall_ms,
        r.peak_live_bytes,
        r.allocated_bytes,
        r.final_live_fraction,
    );
    if let Some(c) = c {
        let _ = write!(
            s,
            ", \"work_ratio\": {:.6}, \"time_ratio\": {:.6}, \"memory_ratio\": {:.6}, \
             \"mean_divergence\": {:.6}, \"final_divergence\": {:.6}, \"live_delta\": {:.6}",
            c.work_ratio,
            c.time_ratio,
            c.memory_ratio,
            c.mean_divergence,
            c.final_divergence,
            c.live_delta
        );
    }
    s.push('}');
    s
}

/// The printed summary. Reads as an answer to the question the experiment
/// asks, not as a dump of counters.
pub fn summary(exp: &Experiment) -> String {
    let r = &exp.reference;
    let mut s = String::new();

    let _ = writeln!(
        s,
        "reference universe (no limits): {} cells, {} neighbour visits, {:.0} ms",
        r.fine_cells, r.work.neighbor_visits, r.wall_ms
    );
    let _ = writeln!(
        s,
        "control (same universe, different seed): macro floor {:.5}, occupancy floor {:.5}",
        exp.chaos_floor, exp.chaos_floor_live
    );
    let _ = writeln!(
        s,
        "this world is chaotic, so divergence at or below the floor is what chaos alone\n\
         produces. only divergence above the floor is a limit showing through.\n"
    );

    let _ = writeln!(
        s,
        "{:<10} {:>8} {:>8} {:>8} {:>10} {:>10} {:>10}",
        "limit", "work", "time", "memory", "macro div", "vs floor", "occup div"
    );
    let _ = writeln!(s, "{}", "-".repeat(70));
    for c in &exp.comparisons {
        let _ = writeln!(
            s,
            "{:<10} {:>7.3}x {:>7.3}x {:>7.3}x {:>10.5} {:>9.2}x {:>10.5}",
            c.run.label,
            c.work_ratio,
            c.time_ratio,
            c.memory_ratio,
            c.mean_divergence,
            floor_ratio(c.mean_divergence, exp.chaos_floor),
            c.live_delta,
        );
    }

    s.push('\n');
    s.push_str(&verdict(exp));
    s
}

fn floor_ratio(d: f64, floor: f64) -> f64 {
    if floor == 0.0 { f64::NAN } else { d / floor }
}

/// A limit is only interesting if it is both cheap and hard to notice.
///
/// "Hard to notice" is judged against the chaos floor rather than against
/// zero, because zero is unreachable in a chaotic world: re-seeding the
/// reference already moves the macro field by the floor amount.
const NOTICEABLE: f64 = 1.25;
const CHEAP: f64 = 0.9;

/// State what the numbers support, and refuse to overstate it.
fn verdict(exp: &Experiment) -> String {
    let mut s = String::new();
    let Some(all_on) = exp.comparisons.iter().find(|c| c.run.label == "all_on") else {
        return s;
    };

    let _ = writeln!(
        s,
        "all limits together: {:.1}% less work, {:.1}% less memory, macro divergence {:.5} \
         ({:.2}x the chaos floor)",
        (1.0 - all_on.work_ratio) * 100.0,
        (1.0 - all_on.memory_ratio) * 100.0,
        all_on.mean_divergence,
        floor_ratio(all_on.mean_divergence, exp.chaos_floor),
    );

    let mut free: Vec<&str> = Vec::new();
    let mut costly: Vec<&str> = Vec::new();
    for c in &exp.comparisons {
        if c.run.label == "all_on" {
            continue;
        }
        let noticeable = floor_ratio(c.mean_divergence, exp.chaos_floor) > NOTICEABLE;
        if c.work_ratio < CHEAP && !noticeable {
            free.push(&c.run.label);
        } else if noticeable {
            costly.push(&c.run.label);
        }
    }

    if !free.is_empty() {
        let _ = writeln!(
            s,
            "cheap, and no more visible than a change of seed: {}",
            free.join(", ")
        );
    }
    if !costly.is_empty() {
        let _ = writeln!(
            s,
            "cheap, but visible above the chaos floor, so not a free lunch: {}",
            costly.join(", ")
        );
    }
    if free.is_empty() && costly.is_empty() {
        s.push_str("no limit was both cheap and invisible at this threshold\n");
    }

    s.push_str(
        "\nthis says the limits are coherent as optimizations inside this model. \
         it says nothing about whether our universe works this way.\n",
    );
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, ReportCfg, WorldCfg};
    use crate::constraints::{Constraints, Params};
    use crate::experiment::run_all;
    use crate::observer::Probe;
    use crate::physics::Rules;

    fn exp() -> Experiment {
        let cfg = Config {
            world: WorldCfg {
                width: 32,
                height: 32,
                ticks: 6,
                seed: 5,
                init_density: 0.3,
            },
            rules: Rules::default(),
            constraints: Constraints::ALL_ON,
            params: Params::default(),
            observer: Probe {
                x: 0,
                y: 0,
                width: 8,
                height: 8,
            },
            report: ReportCfg {
                macro_grid: 8,
                out_dir: "out".into(),
            },
        };
        run_all(&cfg, |_| {})
    }

    #[test]
    fn csv_header_matches_row_width() {
        let csv = to_csv(&exp());
        let mut lines = csv.lines();
        let header = lines.next().unwrap();
        assert_eq!(header.split(',').count(), COLUMNS.len());
        for line in lines {
            assert_eq!(line.split(',').count(), COLUMNS.len(), "row: {line}");
        }
    }

    #[test]
    fn csv_reports_the_chaos_floor_in_json() {
        let json = to_json(&exp());
        assert!(json.contains("chaos_floor"));
        assert!(json.contains("live_delta"));
    }

    #[test]
    fn the_summary_calibrates_against_the_control() {
        let s = summary(&exp());
        assert!(s.contains("chaos"), "summary must state the floor: {s}");
        assert!(s.contains("vs floor"));
    }

    #[test]
    fn csv_includes_the_reference_and_every_variant() {
        let csv = to_csv(&exp());
        assert_eq!(csv.lines().count(), 7, "header + reference + five variants");
        assert!(csv.contains("all_off"));
        assert!(csv.contains("all_on"));
    }

    #[test]
    fn json_is_balanced() {
        let json = to_json(&exp());
        let opens = json.chars().filter(|c| *c == '{').count();
        let closes = json.chars().filter(|c| *c == '}').count();
        assert_eq!(opens, closes);
        assert_eq!(
            json.chars().filter(|c| *c == '[').count(),
            json.chars().filter(|c| *c == ']').count()
        );
    }

    #[test]
    fn the_summary_refuses_to_overclaim() {
        let s = summary(&exp());
        assert!(s.contains("says nothing about whether our universe works this way"));
    }

    #[test]
    fn the_summary_names_every_run() {
        let s = summary(&exp());
        for c in exp().comparisons {
            assert!(s.contains(&c.run.label), "missing {}", c.run.label);
        }
    }
}
