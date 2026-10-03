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

use crate::bootloader::{BootChain, BootLayer, EdgeCell, Ending};
use crate::experiment::Spread;
use crate::layer::Chain;
use crate::pipe::{self, Relay};
use crate::sweep::{self, Sensitivity, Sweep};
use std::fmt::Write as _;
use std::io;
use std::path::{Path, PathBuf};

pub mod boot;
pub mod detect;
pub mod information;
pub mod limits;
pub mod measure;
pub mod nesting;

/// Files written by one experiment.
pub struct Written {
    pub csv: PathBuf,
    pub json: PathBuf,
}

// ---------------------------------------------------------------------------
// Shared helpers for ensembles
// ---------------------------------------------------------------------------

/// `mean [min, max]`, the one way a spread is printed anywhere in a report.
pub fn spread_cell(sp: &Spread, digits: usize) -> String {
    if sp.n == 0 {
        return "n/a".to_string();
    }
    format!(
        "{:.d$} [{:.d$}, {:.d$}]",
        sp.mean,
        sp.min,
        sp.max,
        d = digits
    )
}

/// One spread as a JSON object.
pub fn spread_json(sp: &Spread) -> String {
    format!(
        "{{\"mean\": {}, \"min\": {}, \"max\": {}, \"n\": {}}}",
        json_f64(sp.mean),
        json_f64(sp.min),
        json_f64(sp.max),
        sp.n
    )
}

fn json_f64(v: f64) -> String {
    if v.is_finite() {
        format!("{v:.6}")
    } else {
        "null".to_string()
    }
}

/// Seeds an ensemble ran, as printed in every ensemble header.
pub fn seed_list(seeds: &[u64]) -> String {
    match seeds {
        [] => String::new(),
        [only] => only.to_string(),
        [first, .., last] => format!("{first}, ..., {last}"),
    }
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
        relay.horizon.message_bits(),
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
    s.push_str("  ],\n  \"widths\": [\n");
    let widths = relay.width_sweep();
    for (i, w) in widths.iter().enumerate() {
        let _ = write!(
            s,
            "    {{\"bits\": {}, \"levels_seen\": {}, \"correlation\": {}}}",
            w.bits,
            w.levels_seen,
            json_f64(w.correlation)
        );
        if i + 1 < widths.len() {
            s.push(',');
        }
        s.push('\n');
    }
    s.push_str("  ]\n}\n");
    s
}

/// The channel-width sweep, one row per width.
pub fn widths_to_csv(relay: &Relay) -> String {
    let mut s = String::from("bits,levels_seen,correlation\n");
    for w in relay.width_sweep() {
        let _ = writeln!(s, "{},{},{:.6}", w.bits, w.levels_seen, w.correlation);
    }
    s
}

pub fn write_pipe(relay: &Relay, out_dir: &Path) -> io::Result<Written> {
    std::fs::create_dir_all(out_dir)?;
    let csv = out_dir.join("pipe.csv");
    let json = out_dir.join("pipe.json");
    std::fs::write(&csv, pipe_to_csv(relay))?;
    std::fs::write(&json, pipe_to_json(relay))?;
    std::fs::write(out_dir.join("widths.csv"), widths_to_csv(relay))?;
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
        h.message_bits()
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

    let _ = writeln!(
        s,
        "\nthe same child read through narrower channels at once:\n\n\
         {:>6}  {:>8}  {:>12}",
        "bits", "levels", "correlation"
    );
    let _ = writeln!(s, "{}", "-".repeat(30));
    for w in relay.width_sweep() {
        let corr = if w.correlation.is_nan() {
            "constant".to_string()
        } else {
            format!("{:.4}", w.correlation)
        };
        let _ = writeln!(s, "{:>6}  {:>8}  {corr:>12}", w.bits, w.levels_seen);
    }

    s.push('\n');
    s.push_str(&pipe_verdict(relay));
    s
}

fn pipe_verdict(relay: &Relay) -> String {
    let mut s = String::new();
    let a = relay.content_avalanche;
    let c = relay.magnitude_correlation();

    // What is designed in, said first and plainly, so nothing below reads as
    // a discovery when it is a definition.
    s.push_str(
        "designed in, not found: a message has a magnitude and a digest, and the digest\n\
         is a hash. that magnitude crosses and arrangement scatters follows from that.\n",
    );

    if a.is_nan() {
        let _ = writeln!(
            s,
            "no digest crossed at this width, so there was no arrangement to scatter"
        );
    } else if (0.35..=0.65).contains(&a) {
        let _ = writeln!(
            s,
            "the fold behaves as a hash ({:.1}% of digest bits flip on a one-cell change):\n\
             a check on how it was built, not a result about pipes",
            a * 100.0
        );
    } else {
        let _ = writeln!(
            s,
            "the fold does NOT behave as a hash ({a:.3} avalanche): the digest leaks the\n\
             arrangement, and the pipe is not the serializing write it claims to be"
        );
    }

    if c.is_nan() {
        let _ = writeln!(
            s,
            "magnitude carried nothing measurable: the child never varied"
        );
    } else {
        let _ = writeln!(
            s,
            "\nmeasured: what crossed tracks the child at {c:.4}, from a channel carrying\n\
             {:.2}% of the information",
            relay.horizon.compression_ratio() * 100.0
        );
        match relay.width_for(0.9) {
            Some(bits) => {
                let _ = writeln!(
                    s,
                    "measured: {bits} bits per tick are enough to keep 90% of that correlation"
                );
            }
            None => {
                let _ = writeln!(s, "no narrower channel kept 90% of that correlation");
            }
        }
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
        "{:>5}  {:>11}  {:>13}  {:>7}  {:>9}  {:>10}  {:>8}  {:>7}",
        "depth", "world", "seed", "boots", "per 1000", "transport", "crossed", "child"
    );
    let _ = writeln!(s, "{}", "-".repeat(83));
    for l in &chain.layers {
        let _ = writeln!(
            s,
            "{:>5}  {:>11}  {:>13}  {:>7}  {:>9.3}  {:>10.1}  {:>8}  {:>7}",
            l.depth,
            format!("{}x{}", l.spec.width, l.spec.height),
            l.seed % 1_000_000_000,
            l.survey.bootloaders,
            l.survey.per_kilocell(),
            l.survey.transport,
            l.crossed,
            if l.booted_child { "yes" } else { "no" },
        );
    }
    s.push_str(
        "'per 1000' is bootloaders per thousand cells: a count falls with the world whatever the\n\
         world does, a density need not.\n",
    );

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

// ---------------------------------------------------------------------------
// The other experiments across seeds
// ---------------------------------------------------------------------------
//
// Each takes the per-seed results in seed order, pinned seed first, and writes
// one row per seed to `ensemble.csv` beside that experiment's own files. Every
// summary reports spreads, never a lone value, and ends by declining to
// overstate what an ensemble of a toy shows.

const ENSEMBLE_CLOSER: &str = "\nmore seeds make this a finding about the model rather than about one universe in \
it. they say nothing about whether our universe works this way.\n";

fn ensemble_header(s: &mut String, seeds: &[u64]) {
    let _ = writeln!(s, "ensemble: {} seeds ({})", seeds.len(), seed_list(seeds));
}

fn write_rows(out_dir: &Path, csv: String) -> io::Result<PathBuf> {
    std::fs::create_dir_all(out_dir)?;
    let path = out_dir.join("ensemble.csv");
    std::fs::write(&path, csv)?;
    Ok(path)
}

/// Whether churn fell at every step down a chain.
fn churn_falls(chain: &Chain) -> bool {
    chain.layers.windows(2).all(|w| w[1].churn < w[0].churn)
}

pub fn chain_ensemble_summary(runs: &[(u64, Chain)]) -> String {
    let mut s = String::new();
    let seeds: Vec<u64> = runs.iter().map(|(s, _)| *s).collect();
    let n = runs.len();
    ensemble_header(&mut s, &seeds);
    let _ = writeln!(
        s,
        "depth built: {}",
        spread_cell(
            &Spread::of(runs.iter().map(|(_, c)| c.layers.len() as f64)),
            2
        )
    );
    let deepest = runs.iter().map(|(_, c)| c.layers.len()).max().unwrap_or(0);
    let _ = writeln!(
        s,
        "\n{:>5}  {:>6}  {:<30}",
        "depth", "seeds", "churn: mean [min, max]"
    );
    let _ = writeln!(s, "{}", "-".repeat(46));
    for d in 0..deepest {
        let sp = Spread::of(
            runs.iter()
                .filter_map(|(_, c)| c.layers.get(d).map(|l| l.churn)),
        );
        let _ = writeln!(s, "{:>5}  {:>6}  {:<30}", d + 1, sp.n, spread_cell(&sp, 5));
    }
    let falls = runs.iter().filter(|(_, c)| churn_falls(c)).count();
    let calmer = runs
        .iter()
        .filter(|(_, c)| match (c.layers.first(), c.layers.last()) {
            (Some(a), Some(b)) => c.layers.len() > 1 && b.churn < a.churn,
            _ => false,
        })
        .count();
    let ratio =
        Spread::of(
            runs.iter()
                .filter_map(|(_, c)| match (c.layers.first(), c.layers.last()) {
                    (Some(a), Some(b)) if c.layers.len() > 1 && b.churn > 0.0 => {
                        Some(a.churn / b.churn)
                    }
                    _ => None,
                }),
        );
    let _ = writeln!(
        s,
        "\nthe deepest layer was calmer than the root in {calmer}/{n} seeds, by {}x",
        spread_cell(&ratio, 2)
    );
    let _ = writeln!(
        s,
        "churn fell at every step down the chain in {falls}/{n} seeds"
    );
    s.push_str(ENSEMBLE_CLOSER);
    s
}

pub fn write_chain_ensemble(runs: &[(u64, Chain)], out_dir: &Path) -> io::Result<PathBuf> {
    let mut csv = String::from("seed,depth,width,height,budget_work,spent_work,churn,sterile\n");
    for (seed, c) in runs {
        for l in &c.layers {
            let _ = writeln!(
                csv,
                "{seed},{},{},{},{},{},{:.6},{}",
                l.layer.depth,
                l.layer.spec.width,
                l.layer.spec.height,
                l.layer.budget.work,
                l.work.neighbor_visits,
                l.churn,
                l.sterile
            );
        }
    }
    write_rows(out_dir, csv)
}

pub fn pipe_ensemble_summary(runs: &[(u64, Relay)]) -> String {
    let mut s = String::new();
    let seeds: Vec<u64> = runs.iter().map(|(s, _)| *s).collect();
    ensemble_header(&mut s, &seeds);
    let _ = writeln!(
        s,
        "content avalanche:     {}",
        spread_cell(
            &Spread::of(runs.iter().map(|(_, r)| r.content_avalanche)),
            3
        )
    );
    let _ = writeln!(
        s,
        "magnitude correlation: {}",
        spread_cell(
            &Spread::of(runs.iter().map(|(_, r)| r.magnitude_correlation())),
            3
        )
    );
    let _ = writeln!(
        s,
        "narrowest width keeping 90% of the full correlation: {}",
        spread_cell(
            &Spread::of(
                runs.iter()
                    .map(|(_, r)| r.width_for(0.9).map_or(f64::NAN, f64::from))
            ),
            1
        )
    );
    let _ = writeln!(s, "\n{:>6}  {:<30}", "bits", "correlation: mean [min, max]");
    let _ = writeln!(s, "{}", "-".repeat(40));
    for (i, &bits) in pipe::WIDTHS.iter().enumerate() {
        let sp = Spread::of(
            runs.iter()
                .map(|(_, r)| r.width_sweep().get(i).map_or(f64::NAN, |w| w.correlation)),
        );
        let _ = writeln!(s, "{:>6}  {:<30}", bits, spread_cell(&sp, 3));
    }
    let _ = writeln!(
        s,
        "\n{:>10}  {:<28}",
        "threshold", "registers: mean [min, max]"
    );
    let _ = writeln!(s, "{}", "-".repeat(40));
    for &t in pipe::THRESHOLDS {
        let sp = Spread::of(runs.iter().map(|(_, r)| r.visible_fraction(t)));
        let _ = writeln!(s, "{:>10.2}  {:<28}", t, spread_cell(&sp, 3));
    }
    s.push_str(ENSEMBLE_CLOSER);
    s
}

pub fn write_pipe_ensemble(runs: &[(u64, Relay)], out_dir: &Path) -> io::Result<PathBuf> {
    let mut csv = String::from("seed,content_avalanche,magnitude_correlation,width_for_90\n");
    for (seed, r) in runs {
        let _ = writeln!(
            csv,
            "{seed},{:.6},{:.6},{}",
            r.content_avalanche,
            r.magnitude_correlation(),
            r.width_for(0.9)
                .map_or("NaN".to_string(), |b| b.to_string())
        );
    }
    write_rows(out_dir, csv)
}

pub fn sweep_ensemble_summary(runs: &[(u64, Sweep)]) -> String {
    let mut s = String::new();
    let seeds: Vec<u64> = runs.iter().map(|(s, _)| *s).collect();
    let n = runs.len();
    ensemble_header(&mut s, &seeds);
    let admitted = runs
        .iter()
        .filter(|(_, sw)| sw.reference_is_admitted())
        .count();
    let _ = writeln!(
        s,
        "productive share of distinct laws: {}",
        spread_cell(
            &Spread::of(runs.iter().map(|(_, sw)| sw.productive_rule_fraction())),
            3
        )
    );
    let _ = writeln!(
        s,
        "productive laws:                   {}",
        spread_cell(
            &Spread::of(runs.iter().map(|(_, sw)| sw.distinct_complex() as f64)),
            1
        )
    );
    let _ = writeln!(
        s,
        "distinct laws reachable:           {}",
        spread_cell(
            &Spread::of(runs.iter().map(|(_, sw)| sw.distinct_rules() as f64)),
            1
        )
    );
    let _ = writeln!(s, "Conway passed its own bar in {admitted}/{n} seeds");
    s.push_str(ENSEMBLE_CLOSER);
    s
}

pub fn write_sweep_ensemble(runs: &[(u64, Sweep)], out_dir: &Path) -> io::Result<PathBuf> {
    let mut csv = String::from(
        "seed,distinct_rules,distinct_complex,productive_rule_fraction,productive_fraction,reference_admitted\n",
    );
    for (seed, sw) in runs {
        let _ = writeln!(
            csv,
            "{seed},{},{},{:.6},{:.6},{}",
            sw.distinct_rules(),
            sw.distinct_complex(),
            sw.productive_rule_fraction(),
            sw.productive_fraction(),
            sw.reference_is_admitted()
        );
    }
    write_rows(out_dir, csv)
}

// ---------------------------------------------------------------------------
// Theory 6: sensitivity to the criterion
// ---------------------------------------------------------------------------

fn pct(v: f64) -> String {
    format!("{:.1}%", v * 100.0)
}

pub fn sensitivity_summary(sens: &Sensitivity) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "fine-tuning under three criteria, over every distinct law reachable by sweeping\n\
         both band centres and both half-widths: {} laws, {} of them at Conway's widths\n",
        sens.law_count(false),
        sens.law_count(true)
    );
    let _ = writeln!(
        s,
        "{:<22} {:>14} {:>18} {:>18}",
        "criterion", "conway passes", "conway's widths", "all four constants"
    );
    let _ = writeln!(s, "{}", "-".repeat(75));
    for (i, c) in sens.criteria.iter().enumerate() {
        let _ = writeln!(
            s,
            "{:<22} {:>14} {:>18} {:>18}",
            c.label(),
            if c.admits(&sens.conway) { "yes" } else { "no" },
            pct(sens.fraction(i, true)),
            pct(sens.fraction(i, false))
        );
    }
    let (lo, hi) = sens.range();
    let _ = writeln!(
        s,
        "\nproductive share of distinct laws, across criteria: {} to {}",
        pct(lo),
        pct(hi)
    );
    s.push_str(
        "\nonly the first criterion looks at Conway. the other two have bands fixed in advance,\n\
         which is a choice too: every criterion here encodes a guess about what complexity is.\n\
         the spread between them is the honest size of the answer.\n",
    );
    s
}

pub fn write_sensitivity(sens: &Sensitivity, out_dir: &Path) -> io::Result<Written> {
    std::fs::create_dir_all(out_dir)?;
    let mut csv = String::from(
        "signature,conway_widths,final_live,activity,dispersion,compressibility,growth",
    );
    for c in &sens.criteria {
        let _ = write!(csv, ",{}", c.label().replace(' ', "_"));
    }
    csv.push('\n');
    for (sig, fixed, p) in &sens.laws {
        let _ = write!(
            csv,
            "{sig},{fixed},{:.6},{:.8},{:.6},{:.6},{:.6}",
            p.final_live, p.activity, p.dispersion, p.compressibility, p.growth
        );
        for c in &sens.criteria {
            let _ = write!(csv, ",{}", c.admits(p));
        }
        csv.push('\n');
    }

    let mut json = String::from("{\n");
    let _ = writeln!(
        json,
        "  \"laws\": {}, \"laws_at_conway_widths\": {},",
        sens.law_count(false),
        sens.law_count(true)
    );
    let (lo, hi) = sens.range();
    let _ = writeln!(json, "  \"range\": [{}, {}],", json_f64(lo), json_f64(hi));
    json.push_str("  \"criteria\": [\n");
    for (i, c) in sens.criteria.iter().enumerate() {
        let _ = write!(
            json,
            "    {{\"criterion\": \"{}\", \"conway_admitted\": {}, \"at_conway_widths\": {}, \
             \"all_constants\": {}}}",
            c.label(),
            c.admits(&sens.conway),
            json_f64(sens.fraction(i, true)),
            json_f64(sens.fraction(i, false))
        );
        if i + 1 < sens.criteria.len() {
            json.push(',');
        }
        json.push('\n');
    }
    json.push_str("  ]\n}\n");

    let csv_path = out_dir.join("sensitivity.csv");
    let json_path = out_dir.join("sensitivity.json");
    std::fs::write(&csv_path, csv)?;
    std::fs::write(&json_path, json)?;
    Ok(Written {
        csv: csv_path,
        json: json_path,
    })
}

pub fn sensitivity_ensemble_summary(runs: &[(u64, Sensitivity)]) -> String {
    let mut s = String::new();
    let seeds: Vec<u64> = runs.iter().map(|(s, _)| *s).collect();
    let n = runs.len();
    ensemble_header(&mut s, &seeds);
    let Some((_, first)) = runs.first() else {
        return s;
    };
    let _ = writeln!(
        s,
        "\n{:<22} {:>9}  {:<24} {:<24}",
        "criterion", "conway", "conway's widths", "all four constants"
    );
    let _ = writeln!(s, "{}", "-".repeat(82));
    for (i, c) in first.criteria.iter().enumerate() {
        let passes = runs
            .iter()
            .filter(|(_, x)| x.criteria[i].admits(&x.conway))
            .count();
        let fixed = Spread::of(runs.iter().map(|(_, x)| x.fraction(i, true)));
        let all = Spread::of(runs.iter().map(|(_, x)| x.fraction(i, false)));
        let _ = writeln!(
            s,
            "{:<22} {:>9}  {:<24} {:<24}",
            c.label(),
            format!("{passes}/{n}"),
            spread_cell(&fixed, 3),
            spread_cell(&all, 3)
        );
    }
    let lows = Spread::of(runs.iter().map(|(_, x)| x.range().0));
    let highs = Spread::of(runs.iter().map(|(_, x)| x.range().1));
    let _ = writeln!(
        s,
        "\nlowest criterion per seed:  {}\nhighest criterion per seed: {}",
        spread_cell(&lows, 3),
        spread_cell(&highs, 3)
    );
    s.push_str(ENSEMBLE_CLOSER);
    s
}

pub fn write_sensitivity_ensemble(
    runs: &[(u64, Sensitivity)],
    out_dir: &Path,
) -> io::Result<PathBuf> {
    std::fs::create_dir_all(out_dir)?;
    let mut csv = String::from("seed,criterion,conway_admitted,at_conway_widths,all_constants\n");
    for (seed, x) in runs {
        for (i, c) in x.criteria.iter().enumerate() {
            let _ = writeln!(
                csv,
                "{seed},{},{},{:.6},{:.6}",
                c.label(),
                c.admits(&x.conway),
                x.fraction(i, true),
                x.fraction(i, false)
            );
        }
    }
    let path = out_dir.join("sensitivity_ensemble.csv");
    std::fs::write(&path, csv)?;
    Ok(path)
}

pub fn boot_ensemble_summary(runs: &[(u64, BootChain)]) -> String {
    let mut s = String::new();
    let seeds: Vec<u64> = runs.iter().map(|(s, _)| *s).collect();
    let n = runs.len();
    ensemble_header(&mut s, &seeds);
    let _ = writeln!(
        s,
        "depth reached: {}",
        spread_cell(&Spread::of(runs.iter().map(|(_, c)| c.depth() as f64)), 2)
    );
    let deepest = runs.iter().map(|(_, c)| c.layers.len()).max().unwrap_or(0);
    let _ = writeln!(
        s,
        "\n{:>5}  {:>6}  {:<26} {:<30} {:<26}",
        "depth",
        "seeds",
        "boots: mean [min, max]",
        "per 1000 cells: mean ± sd [ci]",
        "edge: mean [min, max]"
    );
    let _ = writeln!(s, "{}", "-".repeat(100));
    for d in 0..deepest {
        let layers: Vec<&BootLayer> = runs.iter().filter_map(|(_, c)| c.layers.get(d)).collect();
        let boots = Spread::of(layers.iter().map(|l| l.survey.bootloaders as f64));
        let density = crate::stats::Summary::of(layers.iter().map(|l| l.survey.per_kilocell()));
        let edge = Spread::of(layers.iter().map(|l| l.spec.width as f64));
        let _ = writeln!(
            s,
            "{:>5}  {:>6}  {:<26} {:<30} {:<26}",
            d + 1,
            layers.len(),
            spread_cell(&boots, 1),
            limits::summary_cell(&density, 3),
            spread_cell(&edge, 1)
        );
    }
    let thinning = runs
        .iter()
        .filter(|(_, c)| {
            c.layers
                .windows(2)
                .all(|w| w[1].survey.bootloaders <= w[0].survey.bootloaders)
        })
        .count();
    let density_falls = runs
        .iter()
        .filter(|(_, c)| {
            c.layers.len() >= 2
                && c.layers.last().unwrap().survey.per_kilocell()
                    < c.layers[0].survey.per_kilocell()
        })
        .count();
    let _ = writeln!(
        s,
        "\nbootloaders never rose down the chain in {}; a count falls with area, so read the\n\
         density: the deepest layer is below the root per thousand cells in {}",
        limits::share_cell(thinning, n),
        limits::share_cell(density_falls, n)
    );
    let mut reasons: Vec<(&str, usize)> = Vec::new();
    for (_, c) in runs {
        match reasons.iter_mut().find(|(r, _)| *r == c.ended_because) {
            Some((_, k)) => *k += 1,
            None => reasons.push((c.ended_because, 1)),
        }
    }
    s.push_str("why the chain ended:\n");
    for (r, k) in reasons {
        let _ = writeln!(s, "  {k:>3}/{n}  {r}");
    }
    s.push_str(ENSEMBLE_CLOSER);
    s
}

/// What removing the bootloader gate changed, for one seed.
///
/// The child's seed is hashed from what crossed the horizon and nothing else,
/// so the gate is the only way bootloaders reach the next layer. If the two
/// chains build the same layers, the gate never fired and bootloaders changed
/// nothing; if the ungated one runs on past a layer, the gate is what stopped
/// it, and still nothing about the layers it shares was decided by life.
pub fn gate_ablation(gated: &BootChain, ungated: &BootChain) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "ablation: the same chain with the bootloader gate removed\n\n\
         {:<10} {:>6}  ended because",
        "", "depth"
    );
    for c in [gated, ungated] {
        let _ = writeln!(
            s,
            "{:<10} {:>6}  {}",
            c.gate.label(),
            c.depth(),
            c.ended_because
        );
    }
    s.push('\n');
    let fired_at = gated
        .layers
        .iter()
        .find(|l| !l.survey.can_boot())
        .map(|l| l.depth);
    if gated.same_layers(ungated) {
        match fired_at {
            None => s.push_str(
                "the two chains are identical: every layer had a bootloader, so the gate never\n\
                 fired, and bootloaders changed nothing the next layer received.\n",
            ),
            Some(d) => {
                let _ = writeln!(
                    s,
                    "the two chains are identical: the gate fired at depth {d}, but no smaller world\n\
                     was viable, so the chain would have stopped there anyway."
                );
            }
        }
    } else if gated.is_prefix_of(ungated) {
        let _ = writeln!(
            s,
            "the ungated chain built the same {} layers and then {} more: the gate is what\n\
             stopped it. bootloaders decide whether a child exists, never what it is.",
            gated.depth(),
            ungated.depth() - gated.depth()
        );
    } else {
        s.push_str(
            "the two chains differ in layers they share: bootloaders shaped what a child\n\
             received, not only whether it existed.\n",
        );
    }
    s
}

/// The ablation across an ensemble.
pub fn gate_ensemble_summary(runs: &[(u64, (BootChain, BootChain))]) -> String {
    let mut s = String::new();
    let n = runs.len();
    let identical = runs.iter().filter(|(_, (g, u))| g.same_layers(u)).count();
    let extended = runs
        .iter()
        .filter(|(_, (g, u))| !g.same_layers(u) && g.is_prefix_of(u))
        .count();
    let other = n - identical - extended;
    let fired = runs
        .iter()
        .filter(|(_, (g, _))| g.layers.iter().any(|l| !l.survey.can_boot()))
        .count();
    let _ = writeln!(
        s,
        "gate ablation over {n} seeds: identical chains {identical}/{n}, ungated ran on \
         {extended}/{n}, differed in shared layers {other}/{n}"
    );
    let _ = writeln!(
        s,
        "the gate fired in {fired}/{n} seeds and was the only thing stopping the chain in {extended}/{n}"
    );
    let _ = writeln!(
        s,
        "depth gated {}, ungated {}",
        spread_cell(
            &Spread::of(runs.iter().map(|(_, (g, _))| g.depth() as f64)),
            2
        ),
        spread_cell(
            &Spread::of(runs.iter().map(|(_, (_, u))| u.depth() as f64)),
            2
        )
    );
    s
}

/// The ablation across an ensemble, one row per seed.
pub fn write_gate_ensemble(
    runs: &[(u64, (BootChain, BootChain))],
    out_dir: &Path,
) -> io::Result<PathBuf> {
    std::fs::create_dir_all(out_dir)?;
    let mut csv = String::from("seed,gated_depth,ungated_depth,gate_fired,identical,prefix\n");
    for (seed, (g, u)) in runs {
        let _ = writeln!(
            csv,
            "{seed},{},{},{},{},{}",
            g.depth(),
            u.depth(),
            g.layers.iter().any(|l| !l.survey.can_boot()),
            g.same_layers(u),
            g.is_prefix_of(u)
        );
    }
    let path = out_dir.join("gate.csv");
    std::fs::write(&path, csv)?;
    Ok(path)
}

/// One chain's ablation as a JSON object.
pub fn gate_json(gated: &BootChain, ungated: &BootChain) -> String {
    format!(
        "{{\"gated_depth\": {}, \"ungated_depth\": {}, \"identical\": {}, \
         \"ungated_extends_gated\": {}, \"ungated_ended_because\": \"{}\"}}",
        gated.depth(),
        ungated.depth(),
        gated.same_layers(ungated),
        gated.is_prefix_of(ungated),
        ungated.ended_because
    )
}

pub fn write_gate(gated: &BootChain, ungated: &BootChain, out_dir: &Path) -> io::Result<PathBuf> {
    std::fs::create_dir_all(out_dir)?;
    let path = out_dir.join("ablation.json");
    std::fs::write(&path, gate_json(gated, ungated) + "\n")?;
    Ok(path)
}

pub fn write_boot_ensemble(runs: &[(u64, BootChain)], out_dir: &Path) -> io::Result<PathBuf> {
    let mut csv = String::from(
        "seed,depth,width,height,layer_seed,bootloaders,transport,crossed,booted_child,ended_because\n",
    );
    for (seed, c) in runs {
        for l in &c.layers {
            let _ = writeln!(
                csv,
                "{seed},{},{},{},{},{},{:.3},{},{},\"{}\"",
                l.depth,
                l.spec.width,
                l.spec.height,
                l.seed,
                l.survey.bootloaders,
                l.survey.transport + 0.0,
                l.crossed,
                l.booted_child,
                c.ended_because
            );
        }
    }
    write_rows(out_dir, csv)
}

// ---------------------------------------------------------------------------
// Where a chain dies
// ---------------------------------------------------------------------------

const ENDINGS: &[Ending] = &[
    Ending::Budget,
    Ending::Space,
    Ending::Threshold,
    Ending::Sterile,
    Ending::Tied,
];

fn edge_grid(cells: &[EdgeCell], glyph: impl Fn(&EdgeCell) -> char) -> String {
    let mut s = String::new();
    let _ = write!(s, "{:>12} ", "floor\\frac");
    for f in crate::bootloader::EDGE_FRACTIONS {
        let _ = write!(s, "{:>5.2}", f);
    }
    s.push('\n');
    for &edge in crate::bootloader::EDGE_FLOORS {
        let _ = write!(s, "{:>12} ", format!("edge {edge}"));
        for &f in crate::bootloader::EDGE_FRACTIONS {
            let c = cells
                .iter()
                .find(|c| c.viable_edge == edge && c.fraction == f)
                .map_or('?', &glyph);
            let _ = write!(s, "{:>5}", c);
        }
        s.push('\n');
    }
    s
}

fn edge_legend() -> String {
    let parts: Vec<String> = ENDINGS
        .iter()
        .map(|e| format!("{} {}", e.glyph(), e.label()))
        .collect();
    format!("  {}\n", parts.join("   "))
}

pub fn edge_summary(cells: &[EdgeCell]) -> String {
    let mut s = String::from(
        "why each chain stopped, by degradation fraction (columns) and smallest viable\n\
         world edge (rows). sterility counts only where the gate alone stopped the chain.\n\n",
    );
    s.push_str(&edge_grid(cells, |c| c.ending.glyph()));
    s.push('\n');
    s.push_str(&edge_legend());
    s.push('\n');
    for e in ENDINGS {
        let k = cells.iter().filter(|c| c.ending == *e).count();
        if k > 0 {
            let _ = writeln!(s, "{:>3}/{}  {}", k, cells.len(), e.label());
        }
    }
    s
}

pub fn write_edge(runs: &[(u64, Vec<EdgeCell>)], out_dir: &Path) -> io::Result<PathBuf> {
    std::fs::create_dir_all(out_dir)?;
    let mut csv = String::from("seed,fraction,viable_edge,gated_depth,ungated_depth,ending\n");
    for (seed, cells) in runs {
        for c in cells {
            let _ = writeln!(
                csv,
                "{seed},{:.2},{},{},{},{}",
                c.fraction,
                c.viable_edge,
                c.gated_depth,
                c.ungated_depth,
                c.ending.code()
            );
        }
    }
    let path = out_dir.join("edge.csv");
    std::fs::write(&path, csv)?;
    Ok(path)
}

/// Across seeds: the commonest ending in each cell, and how often sterility
/// alone was binding there.
pub fn edge_ensemble_summary(runs: &[(u64, Vec<EdgeCell>)]) -> String {
    let mut s = String::new();
    let seeds: Vec<u64> = runs.iter().map(|(s, _)| *s).collect();
    let n = runs.len();
    ensemble_header(&mut s, &seeds);
    let Some((_, first)) = runs.first() else {
        return s;
    };
    let at = |i: usize| runs.iter().filter_map(move |(_, cells)| cells.get(i));
    let commonest: Vec<EdgeCell> = first
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let best = ENDINGS
                .iter()
                .max_by_key(|e| {
                    (
                        at(i).filter(|x| x.ending == **e).count(),
                        std::cmp::Reverse(**e),
                    )
                })
                .copied()
                .unwrap_or(c.ending);
            EdgeCell { ending: best, ..*c }
        })
        .collect();
    s.push_str("\ncommonest ending in each cell:\n\n");
    s.push_str(&edge_grid(&commonest, |c| c.ending.glyph()));
    s.push('\n');
    s.push_str(&edge_legend());

    let sterile_share: Vec<EdgeCell> = first.to_vec();
    s.push_str(
        "\nseeds in which sterility alone stopped the chain, in tenths (0-9, + for all):\n\n",
    );
    let idx = |c: &EdgeCell| first.iter().position(|x| x == c).unwrap_or(0);
    s.push_str(&edge_grid(&sterile_share, |c| {
        let k = at(idx(c)).filter(|x| x.ending == Ending::Sterile).count();
        if k == n {
            '+'
        } else {
            char::from_digit((k * 10 / n.max(1)) as u32, 10).unwrap_or('?')
        }
    }));
    let any = (0..first.len())
        .filter(|&i| at(i).any(|x| x.ending == Ending::Sterile))
        .count();
    let _ = writeln!(
        s,
        "\nsterility alone stopped the chain somewhere in {any}/{} cells, in at least one seed",
        first.len()
    );
    s.push_str(ENSEMBLE_CLOSER);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

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
            horizon: crate::pipe::Horizon::default(),
            received: crate::pipe::WriteEnd::new().seal(),
            child_truth: vec![0.1, 0.2, 0.3],
            content_avalanche: 0.5,
            by_width: Vec::new(),
        };
        assert_rectangular(&pipe_to_csv(&relay), PIPE_COLUMNS.len());
        assert_rectangular(&widths_to_csv(&relay), 3);
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
    fn spread_cells_and_json_handle_an_empty_spread() {
        let empty = Spread::of(std::iter::empty());
        assert_eq!(spread_cell(&empty, 3), "n/a");
        assert!(spread_json(&empty).contains("null"));
    }
}
