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
use crate::layer::Chain;
use crate::pipe::{self, Horizon, Relay};
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

// ---------------------------------------------------------------------------
// Theory 2: nesting
// ---------------------------------------------------------------------------

/// Column order for `chain.csv`.
const CHAIN_COLUMNS: &[&str] = &[
    "depth",
    "budget_work",
    "width",
    "height",
    "ticks",
    "cells",
    "predicted_work",
    "spent_work",
    "budget_used",
    "within_budget",
    "final_live_fraction",
    "churn",
    "sterile",
];

pub fn chain_to_csv(chain: &Chain) -> String {
    let mut s = CHAIN_COLUMNS.join(",");
    s.push('\n');
    for l in &chain.layers {
        let _ = writeln!(
            s,
            "{},{},{},{},{},{},{},{},{:.6},{},{:.6},{:.6},{}",
            l.layer.depth,
            l.layer.budget.work,
            l.layer.spec.width,
            l.layer.spec.height,
            l.layer.spec.ticks,
            l.layer.spec.cells(),
            l.layer.predicted_work,
            l.work.neighbor_visits,
            l.budget_used,
            l.within_budget,
            l.final_live_fraction,
            l.churn,
            l.sterile,
        );
    }
    s
}

pub fn chain_to_json(chain: &Chain) -> String {
    let mut s = String::from("{\n");
    let _ = writeln!(
        s,
        "  \"root_budget\": {}, \"fraction\": {:.6}, \"viable_work\": {}, \"viable_edge\": {},",
        chain.root_budget.work,
        chain.degradation.fraction,
        chain.degradation.viable_work,
        chain.degradation.viable_edge
    );
    let _ = writeln!(
        s,
        "  \"predicted_max_depth\": {}, \"built_depth\": {}, \"productive_depth\": {},",
        chain.predicted_max_depth,
        chain.layers.len(),
        chain.productive_depth()
    );
    let _ = writeln!(
        s,
        "  \"total_work\": {}, \"total_cost_bound\": {:.1},",
        chain.total_work, chain.total_cost_bound
    );
    s.push_str("  \"layers\": [\n");
    for (i, l) in chain.layers.iter().enumerate() {
        let _ = write!(
            s,
            "    {{\"depth\": {}, \"budget_work\": {}, \"width\": {}, \"height\": {}, \
             \"cells\": {}, \"predicted_work\": {}, \"spent_work\": {}, \"budget_used\": {:.6}, \
             \"within_budget\": {}, \"final_live_fraction\": {:.6}, \"churn\": {:.6}, \
             \"sterile\": {}}}",
            l.layer.depth,
            l.layer.budget.work,
            l.layer.spec.width,
            l.layer.spec.height,
            l.layer.spec.cells(),
            l.layer.predicted_work,
            l.work.neighbor_visits,
            l.budget_used,
            l.within_budget,
            l.final_live_fraction,
            l.churn,
            l.sterile,
        );
        if i + 1 < chain.layers.len() {
            s.push(',');
        }
        s.push('\n');
    }
    s.push_str("  ]\n}\n");
    s
}

/// Files written by one chain run.
pub fn write_chain(chain: &Chain, out_dir: &Path) -> io::Result<Written> {
    std::fs::create_dir_all(out_dir)?;
    let csv = out_dir.join("chain.csv");
    let json = out_dir.join("chain.json");
    std::fs::write(&csv, chain_to_csv(chain))?;
    std::fs::write(&json, chain_to_json(chain))?;
    Ok(Written { csv, json })
}

pub fn chain_summary(chain: &Chain) -> String {
    let mut s = String::new();

    let _ = writeln!(
        s,
        "root budget {} work units, each child gets {:.0}% of its host",
        chain.root_budget.work,
        chain.degradation.fraction * 100.0
    );
    let _ = writeln!(
        s,
        "layer 0 is this process; the universes below are layers 1 and down\n"
    );

    let _ = writeln!(
        s,
        "{:>5}  {:>11}  {:>11}  {:>13}  {:>8}  {:>9}  {:>8}",
        "depth", "world", "budget", "spent", "used", "churn", "state"
    );
    let _ = writeln!(s, "{}", "-".repeat(78));
    for l in &chain.layers {
        let _ = writeln!(
            s,
            "{:>5}  {:>11}  {:>11}  {:>13}  {:>7.1}%  {:>9.5}  {:>8}",
            l.layer.depth,
            format!("{}x{}", l.layer.spec.width, l.layer.spec.height),
            l.layer.budget.work,
            l.work.neighbor_visits,
            l.budget_used * 100.0,
            l.churn,
            if l.sterile { "sterile" } else { "live" },
        );
    }

    s.push('\n');
    s.push_str(&chain_verdict(chain));
    s
}

fn chain_verdict(chain: &Chain) -> String {
    let mut s = String::new();

    if chain.layers.is_empty() {
        s.push_str("the root budget could not run a universe at all; there is no chain.\n");
        return s;
    }

    let built = chain.layers.len();
    let _ = writeln!(
        s,
        "the chain terminated at depth {built}; the closed form allowed at most {}",
        chain.predicted_max_depth
    );

    let over = chain.layers.iter().filter(|l| !l.within_budget).count();
    if over == 0 {
        let _ = writeln!(s, "every layer stayed inside the budget its host gave it");
    } else {
        let _ = writeln!(
            s,
            "WARNING: {over} layer(s) outspent their host -- the chain is incoherent"
        );
    }

    let _ = writeln!(
        s,
        "total cost {} against a geometric bound of {:.0}: an arbitrarily deep chain \n\
         still costs the host less than {:.2}x the root layer alone",
        chain.total_work,
        chain.total_cost_bound,
        1.0 / (1.0 - chain.degradation.fraction),
    );

    let productive = chain.productive_depth();
    let sterile = built - chain.layers.iter().filter(|l| !l.sterile).count();
    if sterile == 0 {
        let _ = writeln!(
            s,
            "every layer was still doing something at the end of its run"
        );
    } else {
        let _ = writeln!(
            s,
            "{sterile} of {built} layers ran but produced nothing: degradation has a horizon \n\
             at depth {productive}, past which a universe is affordable but sterile"
        );
    }

    s.push_str(
        "\nthis says nesting and degradation are coherent as a model. layers here cannot \n\
         reach each other -- the pipe between them is v0.3, so their mutual blindness is \n\
         an omission rather than a claim.\n",
    );
    s
}

// ---------------------------------------------------------------------------
// Theory 3: the pipe
// ---------------------------------------------------------------------------

const PIPE_COLUMNS: &[&str] = &["threshold", "visible_fraction", "samples", "correlation"];

pub fn pipe_to_csv(relay: &Relay) -> String {
    let mut s = PIPE_COLUMNS.join(",");
    s.push('\n');
    for row in relay.threshold_sweep(pipe::THRESHOLDS) {
        let _ = writeln!(
            s,
            "{:.4},{:.6},{},{:.6}",
            row.threshold, row.visible_fraction, row.samples, row.correlation
        );
    }
    s
}

pub fn pipe_to_json(relay: &Relay) -> String {
    let mut s = String::from("{\n");
    let _ = writeln!(
        s,
        "  \"horizon\": {{\"x\": {}, \"y\": {}, \"width\": {}, \"height\": {}}},",
        relay.horizon.x, relay.horizon.y, relay.horizon.width, relay.horizon.height
    );
    let _ = writeln!(
        s,
        "  \"content_bits\": {}, \"message_bits\": {}, \"compression_ratio\": {:.8},",
        relay.horizon.content_bits(),
        Horizon::MESSAGE_BITS,
        relay.horizon.compression_ratio()
    );
    let _ = writeln!(
        s,
        "  \"messages\": {}, \"content_avalanche\": {:.6}, \"magnitude_correlation\": {:.6},",
        relay.received.all().len(),
        relay.content_avalanche,
        relay.magnitude_correlation()
    );
    s.push_str("  \"thresholds\": [\n");
    let rows = relay.threshold_sweep(pipe::THRESHOLDS);
    for (i, r) in rows.iter().enumerate() {
        let _ = write!(
            s,
            "    {{\"threshold\": {:.4}, \"visible_fraction\": {:.6}, \"samples\": {}, \"correlation\": {:.6}}}",
            r.threshold, r.visible_fraction, r.samples, r.correlation
        );
        if i + 1 < rows.len() {
            s.push(',');
        }
        s.push('\n');
    }
    s.push_str("  ]\n}\n");
    s
}

pub fn write_pipe(relay: &Relay, out_dir: &Path) -> io::Result<Written> {
    std::fs::create_dir_all(out_dir)?;
    let csv = out_dir.join("pipe.csv");
    let json = out_dir.join("pipe.json");
    std::fs::write(&csv, pipe_to_csv(relay))?;
    std::fs::write(&json, pipe_to_json(relay))?;
    Ok(Written { csv, json })
}

pub fn pipe_summary(relay: &Relay) -> String {
    let mut s = String::new();
    let h = &relay.horizon;

    let _ = writeln!(
        s,
        "horizon {}x{} at ({}, {}): {} bits of content per tick, {} bits transmitted",
        h.width,
        h.height,
        h.x,
        h.y,
        h.content_bits(),
        Horizon::MESSAGE_BITS
    );
    let _ = writeln!(
        s,
        "the channel carries {:.2}% of what a faithful description would need\n",
        h.compression_ratio() * 100.0
    );

    let _ = writeln!(
        s,
        "content structure: {:.1}% of digest bits flip when one cell changes",
        relay.content_avalanche * 100.0
    );
    let _ = writeln!(
        s,
        "timing and magnitude: correlation {:.4} between what crossed and what the child was doing\n",
        relay.magnitude_correlation()
    );

    let _ = writeln!(
        s,
        "{:>10}  {:>10}  {:>8}  {:>12}",
        "threshold", "registers", "events", "correlation"
    );
    let _ = writeln!(s, "{}", "-".repeat(48));
    for r in relay.threshold_sweep(pipe::THRESHOLDS) {
        let corr = if r.correlation.is_nan() {
            format!("too few (<{})", pipe::MIN_CORRELATION_SAMPLES)
        } else {
            format!("{:.4}", r.correlation)
        };
        let _ = writeln!(
            s,
            "{:>10.2}  {:>9.1}%  {:>8}  {corr:>12}",
            r.threshold,
            r.visible_fraction * 100.0,
            r.samples
        );
    }

    s.push('\n');
    s.push_str(&pipe_verdict(relay));
    s
}

fn pipe_verdict(relay: &Relay) -> String {
    let mut s = String::new();
    let a = relay.content_avalanche;
    let c = relay.magnitude_correlation();

    if (0.35..=0.65).contains(&a) {
        let _ = writeln!(
            s,
            "content did not survive: a one-cell change scatters the digest, so comparing\n\
             digests recovers nothing about the arrangement"
        );
    } else {
        let _ = writeln!(
            s,
            "content partly survived ({a:.3} avalanche) -- the fold is not destroying structure\n\
             the way a serializing write should"
        );
    }

    if c.is_nan() {
        let _ = writeln!(
            s,
            "magnitude carried nothing measurable: the child never varied"
        );
    } else if c.abs() >= 0.5 {
        let _ = writeln!(
            s,
            "timing and magnitude did survive: what crossed tracks the child at {c:.4}, from a\n\
             channel carrying {:.2}% of the information",
            relay.horizon.compression_ratio() * 100.0
        );
    } else {
        let _ = writeln!(
            s,
            "timing and magnitude survived only weakly ({c:.4}); the keyhole is a poor guide to\n\
             the room"
        );
    }

    // Where the parent stops seeing anything at all.
    let sweep = relay.threshold_sweep(pipe::THRESHOLDS);
    if let Some(blind) = sweep.iter().find(|r| r.visible_fraction == 0.0) {
        let _ = writeln!(
            s,
            "above a logging threshold of {:.2} the child stops existing as far as the parent\n\
             is concerned -- not quietly, not in aggregate, not at all",
            blind.threshold
        );
    }

    s.push_str(
        "\nthe child was not told it was being read, and holds no type that could tell it.\n\
         mutual blindness here is enforced by the compiler rather than by convention.\n",
    );
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::Degradation;
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
            nesting: Degradation::default(),
            horizon: crate::pipe::Horizon::default(),
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

    /// Every CSV here writes its header from a `const` and its rows from a
    /// separate format string, so the two can drift apart silently. They did
    /// once: `pipe.csv` shipped four fields under a three-field header because
    /// a formatter reflowed the constant out of a patch's way. One guard per
    /// CSV, no exceptions.
    fn assert_rectangular(csv: &str, expected: usize) {
        let mut lines = csv.lines();
        let header = lines.next().expect("a header");
        assert_eq!(header.split(',').count(), expected, "header: {header}");
        for line in lines {
            assert_eq!(line.split(',').count(), expected, "row: {line}");
        }
    }

    #[test]
    fn pipe_csv_is_rectangular() {
        let relay = Relay {
            horizon: Horizon::default(),
            received: crate::pipe::WriteEnd::new().seal(),
            child_truth: vec![0.1, 0.2, 0.3],
            content_avalanche: 0.5,
        };
        assert_rectangular(&pipe_to_csv(&relay), PIPE_COLUMNS.len());
    }

    #[test]
    fn chain_csv_is_rectangular() {
        let chain = Chain {
            root_budget: crate::budget::Budget::new(1000),
            degradation: crate::budget::Degradation::default(),
            predicted_max_depth: 0,
            layers: Vec::new(),
            total_work: 0,
            total_cost_bound: 0.0,
        };
        assert_rectangular(&chain_to_csv(&chain), CHAIN_COLUMNS.len());
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
