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

use crate::bootloader::BootChain;
use crate::detector::Finding;
use crate::experiment::{Comparison, Experiment};
use crate::layer::Chain;
use crate::pipe::{self, Horizon, Relay};
use crate::sweep::{self, Sweep};
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

// ---------------------------------------------------------------------------
// Detection: what a limit looks like from inside
// ---------------------------------------------------------------------------

const DETECT_COLUMNS: &[&str] = &[
    "gaze",
    "limit",
    "signal",
    "with_limit",
    "without_limit",
    "detectable",
    "influence_speed_with",
    "influence_speed_without",
    "smoothness_with",
    "smoothness_without",
];

pub fn detect_to_csv(findings: &[Finding]) -> String {
    let mut s = DETECT_COLUMNS.join(",");
    s.push('\n');
    for f in findings {
        let _ = writeln!(
            s,
            "{},{},{},{:.6},{:.6},{},{:.6},{:.6},{:.6},{:.6}",
            f.gaze.label(),
            f.limit,
            f.signal,
            f.with_value,
            f.without_value,
            f.detectable,
            f.with.influence_speed,
            f.without.influence_speed,
            f.with.smoothness,
            f.without.smoothness,
        );
    }
    s
}

pub fn detect_to_json(findings: &[Finding]) -> String {
    let mut s = String::from("{\n  \"limits\": [\n");
    for (i, f) in findings.iter().enumerate() {
        let _ = write!(
            s,
            "    {{\"limit\": \"{}\", \"signal\": \"{}\", \"with\": {:.6}, \
             \"without\": {:.6}, \"detectable\": {}, \"gaze\": \"{}\", \"note\": \"{}\"}}",
            f.limit,
            f.signal,
            f.with_value,
            f.without_value,
            f.detectable,
            f.gaze.label(),
            f.note
        );
        if i + 1 < findings.len() {
            s.push(',');
        }
        s.push('\n');
    }
    s.push_str("  ]\n}\n");
    s
}

pub fn write_detect(findings: &[Finding], out_dir: &Path) -> io::Result<Written> {
    std::fs::create_dir_all(out_dir)?;
    let csv = out_dir.join("detection.csv");
    let json = out_dir.join("detection.json");
    std::fs::write(&csv, detect_to_csv(findings))?;
    std::fs::write(&json, detect_to_json(findings))?;
    Ok(Written { csv, json })
}

/// Both gazes side by side. The contrast is the result.
pub fn detect_report(rendering: &[Finding], passive: &[Finding]) -> String {
    let mut s = String::new();
    s.push_str("== an inhabitant whose looking renders what it looks at ==\n\n");
    s.push_str(&detect_summary(rendering));
    s.push_str("\n== the same inhabitant, hypothetically able to read without rendering ==\n\n");
    s.push_str(&detect_summary(passive));
    s.push_str(&gaze_contrast(rendering, passive));
    s
}

/// What changed between the two gazes, and what that means.
fn gaze_contrast(rendering: &[Finding], passive: &[Finding]) -> String {
    let mut s = String::from("\n== what the difference shows ==\n\n");
    let mut any = false;
    for (r, p) in rendering.iter().zip(passive.iter()) {
        if r.detectable != p.detectable {
            any = true;
            let _ = writeln!(
                s,
                "{} is {} to an inhabitant that renders by looking, and {} to one that does not.",
                r.limit,
                if r.detectable { "visible" } else { "invisible" },
                if p.detectable { "visible" } else { "invisible" }
            );
        }
    }
    if any {
        s.push_str(
            "\nso what conceals it is the act of looking, not where the inhabitant happens to\n\
             live. the framework defines a probe as the event that forces full-resolution\n\
             computation, which makes reading without rendering not a hard measurement but a\n\
             contradiction. a limit hidden this way is hidden in principle.\n",
        );
    } else {
        s.push_str("both gazes reach the same verdict on every limit.\n");
    }
    s
}

pub fn detect_summary(findings: &[Finding]) -> String {
    let mut s = String::new();

    s.push_str(
        "an inhabitant measuring its own region, with no access to the config, the\n\
         constraint flags, or any second universe to compare against\n\n",
    );

    let _ = writeln!(
        s,
        "{:<16}  {:>16}  {:>10}  {:>10}  {:>12}",
        "limit", "signal", "with", "without", "verdict"
    );
    let _ = writeln!(s, "{}", "-".repeat(72));
    for f in findings {
        let _ = writeln!(
            s,
            "{:<16}  {:>16}  {:>10.4}  {:>10.4}  {:>12}",
            f.limit,
            f.signal,
            f.with_value,
            f.without_value,
            if f.detectable { "found" } else { "invisible" }
        );
    }

    s.push('\n');
    for f in findings {
        let _ = writeln!(s, "{:<16} {}", f.limit, f.note);
    }

    s.push('\n');
    s.push_str(&detect_verdict(findings));
    s
}

fn detect_verdict(findings: &[Finding]) -> String {
    let mut s = String::new();
    let found: Vec<&str> = findings
        .iter()
        .filter(|f| f.detectable)
        .map(|f| f.limit)
        .collect();
    let hidden: Vec<&str> = findings
        .iter()
        .filter(|f| !f.detectable)
        .map(|f| f.limit)
        .collect();

    if !found.is_empty() {
        let _ = writeln!(s, "findable from inside: {}", found.join(", "));
    }
    if !hidden.is_empty() {
        let _ = writeln!(s, "leaves no fingerprint: {}", hidden.join(", "));
    }

    // The confound is the interesting part and deserves saying out loud.
    let speed_movers: Vec<&str> = findings
        .iter()
        .filter(|f| f.signal == "influence_speed" && f.detectable)
        .map(|f| f.limit)
        .collect();
    if speed_movers.len() > 1 {
        let _ = writeln!(
            s,
            "\n{} all move the same statistic and nothing else, so an inhabitant can\n\
             measure the speed of influence but cannot say which limit set it. detecting\n\
             that you are constrained is not the same as learning how.",
            speed_movers.join(" and ")
        );
    }

    s.push_str(
        "\nnone of this tells an inhabitant whether it is simulated. it tells it which of\n\
         its own laws have the shape of an optimization -- which is the most the model\n\
         allows anyone on the inside to know.\n",
    );
    s
}

// ---------------------------------------------------------------------------
// Theory 6: fine-tuning
// ---------------------------------------------------------------------------

const SWEEP_COLUMNS: &[&str] = &[
    "birth_centre",
    "survive_centre",
    "final_live",
    "activity",
    "dispersion",
    "complex",
];

pub fn sweep_to_csv(sw: &Sweep) -> String {
    let mut s = SWEEP_COLUMNS.join(",");
    s.push('\n');
    for o in &sw.grid {
        let _ = writeln!(
            s,
            "{:.6},{:.6},{:.6},{:.8},{:.8},{}",
            o.birth_centre, o.survive_centre, o.final_live, o.activity, o.dispersion, o.complex
        );
    }
    s
}

pub fn sweep_to_json(sw: &Sweep) -> String {
    let mut s = String::from("{\n");
    let _ = writeln!(
        s,
        "  \"steps\": {}, \"min\": {:.6}, \"max\": {:.6},",
        sw.steps, sw.min, sw.max
    );
    let _ = writeln!(
        s,
        "  \"bar\": {{\"min_activity\": {:.8}, \"max_activity\": {:.8}, \
         \"min_dispersion\": {:.6}, \"max_dispersion\": {:.6}, \
         \"min_live\": {:.6}, \"max_live\": {:.6}}},",
        sw.bar.min_activity,
        sw.bar.max_activity,
        sw.bar.min_dispersion,
        sw.bar.max_dispersion,
        sw.bar.min_live,
        sw.bar.max_live
    );
    let _ = writeln!(
        s,
        "  \"productive_fraction\": {:.6}, \"productive_rule_fraction\": {:.6}, \
         \"distinct_rules\": {}, \"distinct_complex\": {}, \"reference_admitted\": {},",
        sw.productive_fraction(),
        sw.productive_rule_fraction(),
        sw.distinct_rules(),
        sw.distinct_complex(),
        sw.reference_is_admitted()
    );
    s.push_str("  \"grid\": [\n");
    for (i, o) in sw.grid.iter().enumerate() {
        let _ = write!(
            s,
            "    {{\"birth_centre\": {:.6}, \"survive_centre\": {:.6}, \"final_live\": {:.6}, \
             \"activity\": {:.8}, \"structure\": {:.8}, \"complex\": {}}}",
            o.birth_centre, o.survive_centre, o.final_live, o.activity, o.dispersion, o.complex
        );
        if i + 1 < sw.grid.len() {
            s.push(',');
        }
        s.push('\n');
    }
    s.push_str("  ]\n}\n");
    s
}

pub fn write_sweep(sw: &Sweep, out_dir: &Path) -> io::Result<Written> {
    std::fs::create_dir_all(out_dir)?;
    let csv = out_dir.join("sweep.csv");
    let json = out_dir.join("sweep.json");
    std::fs::write(&csv, sweep_to_csv(sw))?;
    std::fs::write(&json, sweep_to_json(sw))?;
    Ok(Written { csv, json })
}

/// Which glyph a setting earns on the map.
fn glyph(o: &sweep::Outcome, bar: &sweep::Bar) -> char {
    if o.complex {
        return '#';
    }
    if o.final_live <= 0.001 {
        return ' '; // empty
    }
    if o.final_live >= 0.95 {
        return '@'; // saturated
    }
    if o.activity < bar.min_activity * 0.1 {
        return '.'; // alive but frozen
    }
    if o.activity > bar.max_activity {
        return '~'; // churning too hard to be anything
    }
    ':' // in between, but not resembling the reference closely enough
}

pub fn sweep_summary(sw: &Sweep) -> String {
    let mut s = String::new();

    let _ = writeln!(
        s,
        "swept both band centres over [{:.2}, {:.2}] at {} steps: {} universes",
        sw.min,
        sw.max,
        sw.steps,
        sw.grid.len()
    );
    let _ = writeln!(
        s,
        "bar calibrated from Conway, every criterion a band: activity in [{:.5}, {:.5}],\n\
         dispersion in [{:.3}, {:.3}], occupancy in [{:.3}, {:.2}]\n",
        sw.bar.min_activity,
        sw.bar.max_activity,
        sw.bar.min_dispersion,
        sw.bar.max_dispersion,
        sw.bar.min_live,
        sw.bar.max_live
    );

    // The map. Survive centre runs down, birth centre runs across.
    s.push_str("  survive\n");
    for row in 0..sw.steps {
        let sc = sw.at(0, row).survive_centre;
        let _ = write!(s, "  {sc:>6.3} |");
        for col in 0..sw.steps {
            s.push(glyph(sw.at(col, row), &sw.bar));
        }
        s.push('\n');
    }
    let _ = write!(s, "         +");
    for _ in 0..sw.steps {
        s.push('-');
    }
    let _ = writeln!(s, "\n          {:<width$}", "birth", width = sw.steps);
    let _ = writeln!(
        s,
        "          {:.2}{:>width$.2}",
        sw.min,
        sw.max,
        width = sw.steps.saturating_sub(4).max(1)
    );

    s.push_str("\n  # complex   : near   ~ chaotic   . frozen   @ saturated   (blank) empty\n\n");

    s.push_str(&sweep_verdict(sw));
    s
}

fn sweep_verdict(sw: &Sweep) -> String {
    let mut s = String::new();
    let f = sw.productive_rule_fraction();

    if !sw.reference_is_admitted() {
        s.push_str(
            "WARNING: the reference setting failed the bar it set. the calibration is broken,\n\
             and nothing below should be believed.\n\n",
        );
    }

    let _ = writeln!(
        s,
        "{:.1}% of the swept area produced a complex universe ({} of {} settings)",
        f * 100.0,
        sw.grid.iter().filter(|o| o.complex).count(),
        sw.grid.len()
    );

    // The area fraction flatters the sweep's resolution. Say the honest number.
    let rf = sw.productive_rule_fraction();
    let _ = writeln!(
        s,
        "but those {} settings denote only {} distinct laws, of which {} were productive:\n\
         {:.1}% of the laws this sweep can actually reach",
        sw.grid.len(),
        sw.distinct_rules(),
        sw.distinct_complex(),
        rf * 100.0
    );
    let _ = writeln!(
        s,
        "a neighbourhood of eight cells only ever has densities k/8, so nudging a band\n\
         centre usually changes nothing. area is the resolution of the sweep; laws are\n\
         the resolution of the universe.\n"
    );

    if f < 0.15 {
        s.push_str(
            "the productive band is narrow. most settings of these constants give a universe\n\
             that empties, saturates, or freezes, and the ones that do not sit close together.\n",
        );
    } else if f < 0.5 {
        s.push_str(
            "the productive band is a minority of the space but not a sliver. fine-tuning\n\
             holds here in a weaker form than the argument usually assumes.\n",
        );
    } else {
        s.push_str(
            "most of the swept space is productive. within this model, on these constants,\n\
             fine-tuning does not hold -- complexity is the common case, not the rare one.\n",
        );
    }

    s.push_str(
        "\nwhat this does not show: the bar is calibrated from Conway, so a productive band\n\
         means settings that behave like the one setting already believed interesting. it is\n\
         a measure of resemblance, not of worth. two constants were swept out of the many a\n\
         universe has, and the widths of the bands were held fixed.\n",
    );
    s
}

// ---------------------------------------------------------------------------
// Theory 5: bootloader life
// ---------------------------------------------------------------------------

const BOOT_COLUMNS: &[&str] = &[
    "depth",
    "width",
    "height",
    "budget_work",
    "seed",
    "tracks",
    "bootloaders",
    "transport",
    "longest_lifetime",
    "crossed",
    "booted_child",
];

pub fn boot_to_csv(chain: &BootChain) -> String {
    let mut s = BOOT_COLUMNS.join(",");
    s.push('\n');
    for l in &chain.layers {
        let _ = writeln!(
            s,
            "{},{},{},{},{},{},{},{:.4},{},{},{}",
            l.depth,
            l.spec.width,
            l.spec.height,
            l.budget.work,
            l.seed,
            l.survey.tracks,
            l.survey.bootloaders,
            l.survey.transport,
            l.survey.longest_lifetime,
            l.crossed,
            l.booted_child,
        );
    }
    s
}

pub fn boot_to_json(chain: &BootChain) -> String {
    let mut s = String::from("{\n");
    let _ = writeln!(
        s,
        "  \"depth\": {}, \"ended_because\": \"{}\",",
        chain.depth(),
        chain.ended_because
    );
    s.push_str("  \"layers\": [\n");
    for (i, l) in chain.layers.iter().enumerate() {
        let _ = write!(
            s,
            "    {{\"depth\": {}, \"width\": {}, \"height\": {}, \"budget_work\": {}, \
             \"seed\": {}, \"tracks\": {}, \"bootloaders\": {}, \"transport\": {:.4}, \
             \"longest_lifetime\": {}, \"crossed\": {}, \"booted_child\": {}}}",
            l.depth,
            l.spec.width,
            l.spec.height,
            l.budget.work,
            l.seed,
            l.survey.tracks,
            l.survey.bootloaders,
            l.survey.transport,
            l.survey.longest_lifetime,
            l.crossed,
            l.booted_child,
        );
        if i + 1 < chain.layers.len() {
            s.push(',');
        }
        s.push('\n');
    }
    s.push_str("  ]\n}\n");
    s
}

pub fn write_boot(chain: &BootChain, out_dir: &Path) -> io::Result<Written> {
    std::fs::create_dir_all(out_dir)?;
    let csv = out_dir.join("boot.csv");
    let json = out_dir.join("boot.json");
    std::fs::write(&csv, boot_to_csv(chain))?;
    std::fs::write(&json, boot_to_json(chain))?;
    Ok(Written { csv, json })
}

pub fn boot_summary(chain: &BootChain) -> String {
    let mut s = String::new();

    s.push_str(
        "each layer is seeded by what crossed its parent's horizon: emergent structures\n\
         drive the activity, the activity is what crosses, and what crosses is all the\n\
         child ever receives\n\n",
    );

    let _ = writeln!(
        s,
        "{:>5}  {:>11}  {:>13}  {:>7}  {:>10}  {:>8}  {:>7}",
        "depth", "world", "seed", "boots", "transport", "crossed", "child"
    );
    let _ = writeln!(s, "{}", "-".repeat(72));
    for l in &chain.layers {
        let _ = writeln!(
            s,
            "{:>5}  {:>11}  {:>13}  {:>7}  {:>10.1}  {:>8}  {:>7}",
            l.depth,
            format!("{}x{}", l.spec.width, l.spec.height),
            l.seed % 1_000_000_000,
            l.survey.bootloaders,
            l.survey.transport,
            l.crossed,
            if l.booted_child { "yes" } else { "no" },
        );
    }

    s.push('\n');
    s.push_str(&boot_verdict(chain));
    s
}

fn boot_verdict(chain: &BootChain) -> String {
    let mut s = String::new();

    if chain.layers.is_empty() {
        s.push_str("no layer could be built at all.\n");
        return s;
    }

    let _ = writeln!(
        s,
        "the chain reached depth {} and stopped: {}",
        chain.depth(),
        chain.ended_because
    );

    let sterile = chain.layers.iter().filter(|l| !l.survey.can_boot()).count();
    if sterile > 0 {
        let _ = writeln!(
            s,
            "{sterile} of {} layers produced no bootloader at all",
            chain.depth()
        );
    }

    let total: usize = chain.layers.iter().map(|l| l.survey.bootloaders).sum();
    let _ = writeln!(
        s,
        "{total} bootloading structures across the chain, carrying {:.0} cells of transport",
        chain.layers.iter().map(|l| l.survey.transport).sum::<f64>()
    );

    s.push_str(
        "\na bootloader here is a pattern that persists, stays localized, and travels --\n\
         structure moved to somewhere it was not. that is the precondition for booting\n\
         anything, not the achievement itself. nothing in this model builds a computer;\n\
         it shows that the transport such a thing would require is available, and that a\n\
         layer without it has no way to seed the next one.\n",
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
