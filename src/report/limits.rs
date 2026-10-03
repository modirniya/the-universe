//! Theory 1's report: the factorial, its nulls, and the cost–fidelity table.

use super::{Written, json_f64, seed_list};
use crate::limits::{
    Cell, FACTORS, Factorial, FactorialEnsemble, NULLS, OBSERVABLES, ablation_labels, cell_labels,
};
use crate::stats::{self, Summary};
use std::fmt::Write as _;
use std::io;
use std::path::{Path, PathBuf};

/// The sentence every Theory 1 verdict ends with. A test enforces it.
pub const FRAMING: &str = "this says the limits are coherent as optimizations inside this model. \
it says nothing about whether our universe works this way.\n";

const CONSEQUENCE: &str = "the work and memory ratios follow from the rules by arithmetic \
(fewer cells, substeps, neighbours or resolved blocks); only the fidelity columns had to be run.\n";

// ---------------------------------------------------------------------------
// One seed
// ---------------------------------------------------------------------------

fn cell_row(c: &Cell) -> String {
    format!(
        "{:<22} {:>7.3}x {:>7.3}x {:>9.5} {:>9.5} {:>9.5} {:>8.4} {:>8.4}",
        c.label,
        c.work_ratio,
        c.memory_ratio,
        c.div_half,
        c.div_final,
        c.div_whole,
        c.profile.occupancy,
        c.profile.churn,
    )
}

/// The pinned seed, in full.
pub fn summary(f: &Factorial) -> String {
    let mut s = String::new();
    let r = f.reference();
    let _ = writeln!(
        s,
        "seed {}: reference universe (no limits) spent {} neighbour visits, {:.0} ms, occupancy {:.4}",
        f.seed, r.work.neighbor_visits, r.wall_ms, r.profile.occupancy
    );
    let _ = writeln!(
        s,
        "divergence is the mean |difference| of two macro fields; 'half' averages the second half\n\
         of the run, 'final' is the last tick, 'whole' is the v0.9 whole-run mean kept for comparison.\n\
         read every limit against the nulls at the bottom, not against zero.\n"
    );
    let _ = writeln!(
        s,
        "{:<22} {:>8} {:>8} {:>9} {:>9} {:>9} {:>8} {:>8}",
        "setting", "work", "memory", "div half", "div final", "div whole", "occup", "churn"
    );
    let _ = writeln!(s, "{}", "-".repeat(90));
    for c in &f.cells {
        let _ = writeln!(s, "{}", cell_row(c));
    }
    let _ = writeln!(s, "{}", "-".repeat(90));
    let _ = writeln!(
        s,
        "nulls: the reference altered in ways that are not a limit"
    );
    for c in &f.nulls {
        let _ = writeln!(s, "{}", cell_row(c));
    }
    let _ = writeln!(s, "{}", "-".repeat(90));
    let _ = writeln!(
        s,
        "closure ablation: the same lazy settings with a different rule for unobserved ground\n\
         (the factorial above ran under '{}')",
        f.reference().coarse_rule.label()
    );
    for c in &f.ablation {
        let _ = writeln!(s, "{}", cell_row(c));
    }
    s.push('\n');
    s.push_str(&verdict(f));
    s
}

fn verdict(f: &Factorial) -> String {
    let mut s = String::new();
    let seed = f.null("seed").map_or(f64::NAN, |n| n.div_half);
    let perturb = f.null("perturb").map_or(f64::NAN, |n| n.div_half);
    let _ = writeln!(
        s,
        "at this seed the reseed null diverges {seed:.5} and the one-cell null {perturb:.5} over the second half."
    );
    let mut within = Vec::new();
    let mut beyond = Vec::new();
    for c in f.cells.iter().filter(|c| !c.is_reference()) {
        if c.div_half <= perturb.max(seed) {
            within.push(c.label.clone());
        } else {
            beyond.push(c.label.clone());
        }
    }
    if !within.is_empty() {
        let _ = writeln!(
            s,
            "settings whose late-time divergence is within the larger null: {}",
            within.join(", ")
        );
    }
    if !beyond.is_empty() {
        let _ = writeln!(s, "settings beyond both nulls: {}", beyond.join(", "));
    }
    let _ = writeln!(
        s,
        "one seed is an example. the ensemble below says how often this holds and by how much."
    );
    s.push_str(CONSEQUENCE);
    s.push('\n');
    s.push_str(FRAMING);
    s
}

const CELL_COLUMNS: &[&str] = &[
    "label",
    "kind",
    "coarse_rule",
    "discrete_space",
    "discrete_time",
    "speed_cap",
    "lazy_rendering",
    "influence_speed",
    "neighbor_visits",
    "cell_updates",
    "block_updates",
    "cells_rendered",
    "wall_ms",
    "peak_live_bytes",
    "work_ratio",
    "memory_ratio",
    "time_ratio",
    "div_half",
    "div_final",
    "div_whole",
    "final_live_fraction",
    "occupancy",
    "churn",
    "dispersion",
    "spatial_corr",
    "compressibility",
    "components",
    "entropy",
];

fn csv_row(kind: &str, c: &Cell) -> String {
    let p = c.profile;
    format!(
        "{},{kind},{},{},{},{},{},{:.6},{},{},{},{},{:.3},{},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6}",
        c.label,
        c.coarse_rule.label(),
        c.constraints.discrete_space,
        c.constraints.discrete_time,
        c.constraints.speed_cap,
        c.constraints.lazy_rendering,
        c.influence_speed,
        c.work.neighbor_visits,
        c.work.cell_updates,
        c.work.block_updates,
        c.work.cells_rendered,
        c.wall_ms,
        c.peak_live_bytes,
        c.work_ratio,
        c.memory_ratio,
        c.time_ratio,
        c.div_half,
        c.div_final,
        c.div_whole,
        c.final_live_fraction,
        p.occupancy,
        p.churn,
        p.dispersion,
        p.spatial_corr,
        p.compressibility,
        p.components,
        p.entropy,
    )
}

/// Every cell, null and ablation of one seed, one row each.
pub fn to_csv(f: &Factorial) -> String {
    let mut s = CELL_COLUMNS.join(",");
    s.push('\n');
    for c in &f.cells {
        let _ = writeln!(s, "{}", csv_row("factorial", c));
    }
    for c in &f.nulls {
        let _ = writeln!(s, "{}", csv_row("null", c));
    }
    for c in &f.ablation {
        let _ = writeln!(s, "{}", csv_row("ablation", c));
    }
    s
}

/// Divergence from the reference at every tick, one column per setting.
pub fn traces_to_csv(f: &Factorial) -> String {
    let cols: Vec<&Cell> = f
        .nulls
        .iter()
        .chain(f.cells.iter().filter(|c| !c.is_reference()))
        .chain(f.ablation.iter())
        .collect();
    let mut s = String::from("tick");
    for c in &cols {
        let _ = write!(s, ",{}", c.label);
    }
    s.push('\n');
    let n = cols.iter().map(|c| c.trace.len()).max().unwrap_or(0);
    for t in 0..n {
        let _ = write!(s, "{t}");
        for c in &cols {
            let _ = write!(
                s,
                ",{}",
                c.trace.get(t).map_or("".to_string(), |v| format!("{v:.6}"))
            );
        }
        s.push('\n');
    }
    s
}

fn cell_json(c: &Cell) -> String {
    let p = c.profile.as_array();
    let profile: Vec<String> = OBSERVABLES
        .iter()
        .zip(p)
        .map(|(k, v)| format!("\"{k}\": {}", json_f64(v)))
        .collect();
    format!(
        "{{\"label\": \"{}\", \"coarse_rule\": \"{}\", \"constraints\": {{\"discrete_space\": {}, \"discrete_time\": {}, \"speed_cap\": {}, \"lazy_rendering\": {}}}, \
         \"neighbor_visits\": {}, \"peak_live_bytes\": {}, \"work_ratio\": {}, \"memory_ratio\": {}, \
         \"div_half\": {}, \"div_final\": {}, \"div_whole\": {}, \"final_live_fraction\": {}, \"profile\": {{{}}}}}",
        c.label,
        c.coarse_rule.label(),
        c.constraints.discrete_space,
        c.constraints.discrete_time,
        c.constraints.speed_cap,
        c.constraints.lazy_rendering,
        c.work.neighbor_visits,
        c.peak_live_bytes,
        json_f64(c.work_ratio),
        json_f64(c.memory_ratio),
        json_f64(c.div_half),
        json_f64(c.div_final),
        json_f64(c.div_whole),
        json_f64(c.final_live_fraction),
        profile.join(", "),
    )
}

pub fn to_json(f: &Factorial) -> String {
    let list = |cells: &[Cell]| -> String {
        cells
            .iter()
            .map(cell_json)
            .collect::<Vec<_>>()
            .join(",\n    ")
    };
    format!(
        "{{\n  \"seed\": {},\n  \"coarse_rule\": \"{}\",\n  \"cells\": [\n    {}\n  ],\n  \"nulls\": [\n    {}\n  ],\n  \"ablation\": [\n    {}\n  ]\n}}\n",
        f.seed,
        f.reference().coarse_rule.label(),
        list(&f.cells),
        list(&f.nulls),
        list(&f.ablation),
    )
}

/// `factorial.csv`, `report.json` and `divergence_trace.csv` for one seed.
pub fn write(f: &Factorial, out_dir: &Path) -> io::Result<Written> {
    std::fs::create_dir_all(out_dir)?;
    let csv = out_dir.join("factorial.csv");
    let json = out_dir.join("report.json");
    std::fs::write(&csv, to_csv(f))?;
    std::fs::write(&json, to_json(f))?;
    std::fs::write(out_dir.join("divergence_trace.csv"), traces_to_csv(f))?;
    Ok(Written { csv, json })
}

// ---------------------------------------------------------------------------
// Across seeds
// ---------------------------------------------------------------------------

/// `mean ± sd [lo, hi]` where the bracket is the 95% interval of the mean.
pub fn summary_cell(s: &Summary, digits: usize) -> String {
    if s.n == 0 {
        return "n/a".to_string();
    }
    if s.n == 1 {
        return format!("{:.d$} (n=1)", s.mean, d = digits);
    }
    format!(
        "{:.d$} ± {:.d$} [{:.d$}, {:.d$}]",
        s.mean,
        s.sd,
        s.ci_lo,
        s.ci_hi,
        d = digits
    )
}

/// `k/n [lo, hi]` with a Wilson interval.
pub fn share_cell(k: usize, n: usize) -> String {
    let (lo, hi) = stats::wilson(k, n);
    format!("{k}/{n} [{lo:.2}, {hi:.2}]")
}

pub fn summary_json(s: &Summary) -> String {
    format!(
        "{{\"n\": {}, \"mean\": {}, \"median\": {}, \"sd\": {}, \"min\": {}, \"max\": {}, \"ci_lo\": {}, \"ci_hi\": {}}}",
        s.n,
        json_f64(s.mean),
        json_f64(s.median),
        json_f64(s.sd),
        json_f64(s.min),
        json_f64(s.max),
        json_f64(s.ci_lo),
        json_f64(s.ci_hi)
    )
}

/// The ensemble, as a cost–fidelity table and a factorial analysis.
pub fn ensemble_summary(ens: &FactorialEnsemble) -> String {
    let mut s = String::new();
    let n = ens.len();
    let seeds: Vec<u64> = ens.runs.iter().map(|(s, _)| *s).collect();
    let _ = writeln!(s, "ensemble: {n} seeds ({})", seed_list(&seeds));
    let _ = writeln!(
        s,
        "a seed selects an initial condition and nothing else; intervals below are over initial\n\
         conditions of one configuration. '±' is one standard deviation across seeds, brackets\n\
         are a 95% interval for the mean, and k/n counts carry a Wilson interval.\n"
    );

    // Nulls first: what the statistics read when nothing is a limit.
    let _ = writeln!(
        s,
        "nulls (second-half divergence from the reference, and distance D):"
    );
    let _ = writeln!(
        s,
        "{:<10} {:<30} {:<30}",
        "null", "div half: mean ± sd [95% ci]", "D: mean ± sd [95% ci]"
    );
    for null in NULLS {
        let _ = writeln!(
            s,
            "{:<10} {:<30} {:<30}",
            null,
            summary_cell(&ens.summary(null, |_, c| c.div_half), 5),
            summary_cell(&Summary::of(ens.distances(null)), 2)
        );
    }
    let pw = Summary::of(ens.pairwise_null.iter().copied());
    let _ = writeln!(
        s,
        "pairwise across {} reference pairs: div half {}\n",
        pw.n,
        summary_cell(&pw, 5)
    );
    let _ = writeln!(
        s,
        "D is the root mean square over seven observables of (setting - reference) / sd across\n\
         references of that observable. a reseed scores about sqrt(2); one cell flipped scores\n\
         whatever chaos makes of it by the second half. read every setting against those two rows.\n"
    );

    let _ = writeln!(
        s,
        "{:<22} {:>7} {:>7}  {:<26} {:<24} {:>8} {:>16} {:>16}",
        "setting",
        "work",
        "memory",
        "D: mean ± sd [95% ci]",
        "div half / reseed null",
        "z pair",
        "> reseed",
        "> one cell"
    );
    let _ = writeln!(s, "{}", "-".repeat(136));
    for label in cell_labels(ens) {
        if label == "all_off" {
            continue;
        }
        let d = Summary::of(ens.distances(&label));
        let ratio = Summary::of(ens.ratio_to_null(&label, "seed"));
        let (z, _) = ens.against_pairwise(&label);
        let (k1, n1) = ens.above_null(&label, "seed");
        let (k2, n2) = ens.above_null(&label, "perturb");
        let _ = writeln!(
            s,
            "{:<22} {:>6.3}x {:>6.3}x  {:<26} {:<24} {:>8.2} {:>16} {:>16}",
            label,
            ens.summary(&label, |_, c| c.work_ratio).mean,
            ens.summary(&label, |_, c| c.memory_ratio).mean,
            summary_cell(&d, 2),
            summary_cell(&ratio, 2),
            z,
            share_cell(k1, n1),
            share_cell(k2, n2),
        );
    }
    s.push('\n');

    // Per-observable z for the singles and all_on, so a reader can see *what*
    // changed rather than only how much.
    let _ = writeln!(
        s,
        "mean z per observable (setting - reference, in across-seed sd units):"
    );
    let _ = write!(s, "{:<22}", "setting");
    for o in OBSERVABLES {
        let _ = write!(s, " {:>13}", o);
    }
    s.push('\n');
    for label in NULLS.iter().map(|n| n.to_string()).chain(
        ["space", "time", "speed", "lazy", "all_on"]
            .iter()
            .map(|l| l.to_string()),
    ) {
        let z = ens.mean_z(&label);
        let _ = write!(s, "{:<22}", label);
        for v in z {
            let _ = write!(s, " {:>13.2}", v);
        }
        s.push('\n');
    }
    s.push('\n');

    // Factorial analysis.
    let _ = writeln!(
        s,
        "factorial main effects on D and on log2(work), with two-factor interactions on D:"
    );
    let _ = writeln!(
        s,
        "{:<8} {:<30} {:<30}",
        "factor", "effect on D: mean ± sd [ci]", "effect on log2 work (consequence)"
    );
    for (i, name) in FACTORS.iter().enumerate() {
        let on_d = ens.main_effect(i, |fac, c| ens.distance(fac, c));
        let on_w = ens.main_effect(i, |_, c| c.work_ratio.log2());
        let _ = writeln!(
            s,
            "{:<8} {:<30} {:<30}",
            name,
            summary_cell(&on_d, 2),
            summary_cell(&on_w, 3)
        );
    }
    for (a, name_a) in FACTORS.iter().enumerate() {
        for (b, name_b) in FACTORS.iter().enumerate().skip(a + 1) {
            let inter = ens.interaction(a, b, |fac, c| ens.distance(fac, c));
            let _ = writeln!(
                s,
                "{:<8} {:<30}",
                format!("{name_a}x{name_b}"),
                summary_cell(&inter, 2)
            );
        }
    }
    s.push('\n');

    // Cost against fidelity.
    let _ = writeln!(
        s,
        "cost against fidelity: work saved per unit of D, and the Pareto set"
    );
    let _ = writeln!(
        s,
        "{:<22} {:>8} {:>8} {:>14}",
        "setting", "work", "D", "saved per D"
    );
    for label in cell_labels(ens) {
        if label == "all_off" {
            continue;
        }
        let w = ens.summary(&label, |_, c| c.work_ratio).mean;
        let d = Summary::of(ens.distances(&label)).mean;
        let _ = writeln!(
            s,
            "{:<22} {:>7.3}x {:>8.2} {:>14.3}",
            label,
            w,
            d,
            if d > 0.0 { (1.0 - w) / d } else { f64::NAN }
        );
    }
    let _ = writeln!(
        s,
        "pareto set (no setting is both cheaper and closer): {}",
        ens.pareto().join(", ")
    );
    s.push('\n');

    // Closure ablation.
    let labels = ablation_labels(ens);
    if !labels.is_empty() {
        let _ = writeln!(
            s,
            "closure ablation: lazy settings under each rule for unobserved ground"
        );
        let _ = writeln!(
            s,
            "{:<22} {:>7}  {:<26} {:<24} {:>16}",
            "setting@closure",
            "work",
            "D: mean ± sd [95% ci]",
            "div half / reseed null",
            "> reseed"
        );
        for label in ["lazy".to_string(), "all_on".to_string()]
            .into_iter()
            .chain(labels)
        {
            let d = Summary::of(ens.distances(&label));
            let ratio = Summary::of(ens.ratio_to_null(&label, "seed"));
            let (k, nn) = ens.above_null(&label, "seed");
            let _ = writeln!(
                s,
                "{:<22} {:>6.3}x  {:<26} {:<24} {:>16}",
                label,
                ens.summary(&label, |_, c| c.work_ratio).mean,
                summary_cell(&d, 2),
                summary_cell(&ratio, 2),
                share_cell(k, nn),
            );
        }
        s.push('\n');
    }

    s.push_str(&ensemble_verdict(ens));
    s
}

fn ensemble_verdict(ens: &FactorialEnsemble) -> String {
    let mut s = String::new();
    let n = ens.len();
    let reseed_d = Summary::of(ens.distances("seed")).ci_hi;
    let mut within = Vec::new();
    let mut beyond = Vec::new();
    for label in cell_labels(ens) {
        if label == "all_off" {
            continue;
        }
        let d = Summary::of(ens.distances(&label));
        let (k, _) = ens.above_null(&label, "seed");
        if d.mean <= reseed_d && k * 2 <= n {
            within.push(format!(
                "{label} (D {:.2}, above reseed in {k}/{n})",
                d.mean
            ));
        } else {
            beyond.push(format!(
                "{label} (D {:.2}, above reseed in {k}/{n})",
                d.mean
            ));
        }
    }
    let _ = writeln!(
        s,
        "a setting is 'indistinguishable from a reseed' here only if its D is within the reseed\n\
         null's 95% interval and its late-time divergence beats the reseed in at most half the seeds.\n\
         that is a loose bar, chosen so that the negative claims are hard to make cheaply."
    );
    if within.is_empty() {
        let _ = writeln!(s, "no setting meets it.");
    } else {
        let _ = writeln!(s, "settings that meet it: {}", within.join("; "));
    }
    if !beyond.is_empty() {
        let _ = writeln!(s, "settings that do not: {}", beyond.join("; "));
    }
    let _ = writeln!(
        s,
        "which closure stands in for unobserved ground is an assumption; the ablation rows show\n\
         how much of each lazy result is that assumption."
    );
    s.push_str(CONSEQUENCE);
    s.push('\n');
    s.push_str(FRAMING);
    s
}

const ENSEMBLE_COLUMNS: &[&str] = &[
    "seed",
    "label",
    "kind",
    "coarse_rule",
    "work_ratio",
    "memory_ratio",
    "div_half",
    "div_final",
    "div_whole",
    "distance",
    "z_occupancy",
    "z_churn",
    "z_dispersion",
    "z_spatial_corr",
    "z_compressibility",
    "z_components",
    "z_entropy",
    "occupancy",
    "churn",
    "dispersion",
    "spatial_corr",
    "compressibility",
    "components",
    "entropy",
];

pub fn ensemble_to_csv(ens: &FactorialEnsemble) -> String {
    let mut s = ENSEMBLE_COLUMNS.join(",");
    s.push('\n');
    for (seed, f) in &ens.runs {
        let rows = f
            .cells
            .iter()
            .map(|c| ("factorial", c))
            .chain(f.nulls.iter().map(|c| ("null", c)))
            .chain(f.ablation.iter().map(|c| ("ablation", c)));
        for (kind, c) in rows {
            let z = ens.z_profile(f, c);
            let p = c.profile.as_array();
            let _ = write!(
                s,
                "{seed},{},{kind},{},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6}",
                c.label,
                c.coarse_rule.label(),
                c.work_ratio,
                c.memory_ratio,
                c.div_half,
                c.div_final,
                c.div_whole,
                ens.distance(f, c)
            );
            for v in z.iter().chain(p.iter()) {
                let _ = write!(s, ",{v:.6}");
            }
            s.push('\n');
        }
    }
    s
}

pub fn ensemble_to_json(ens: &FactorialEnsemble) -> String {
    let seeds: Vec<String> = ens.runs.iter().map(|(s, _)| s.to_string()).collect();
    let mut rows = Vec::new();
    let all: Vec<String> = cell_labels(ens)
        .into_iter()
        .chain(NULLS.iter().map(|n| n.to_string()))
        .chain(ablation_labels(ens))
        .collect();
    for label in all {
        let (z, tail) = ens.against_pairwise(&label);
        let (k1, n1) = ens.above_null(&label, "seed");
        let (k2, n2) = ens.above_null(&label, "perturb");
        let mean_z: Vec<String> = OBSERVABLES
            .iter()
            .zip(ens.mean_z(&label))
            .map(|(k, v)| format!("\"{k}\": {}", json_f64(v)))
            .collect();
        rows.push(format!(
            "    {{\"label\": \"{label}\", \"work_ratio\": {}, \"memory_ratio\": {}, \"div_half\": {}, \"div_final\": {}, \"div_whole\": {}, \
             \"distance\": {}, \"ratio_to_reseed\": {}, \"z_vs_pairwise\": {}, \"tail_vs_pairwise\": {}, \
             \"above_reseed\": {k1}, \"above_perturb\": {k2}, \"seeds\": {}, \"mean_z\": {{{}}}}}",
            summary_json(&ens.summary(&label, |_, c| c.work_ratio)),
            summary_json(&ens.summary(&label, |_, c| c.memory_ratio)),
            summary_json(&ens.summary(&label, |_, c| c.div_half)),
            summary_json(&ens.summary(&label, |_, c| c.div_final)),
            summary_json(&ens.summary(&label, |_, c| c.div_whole)),
            summary_json(&Summary::of(ens.distances(&label))),
            summary_json(&Summary::of(ens.ratio_to_null(&label, "seed"))),
            json_f64(z),
            json_f64(tail),
            n1.max(n2),
            mean_z.join(", "),
        ));
    }
    let mut effects = Vec::new();
    for (i, name) in FACTORS.iter().enumerate() {
        effects.push(format!(
            "    {{\"factor\": \"{name}\", \"on_distance\": {}, \"on_log2_work\": {}}}",
            summary_json(&ens.main_effect(i, |fac, c| ens.distance(fac, c))),
            summary_json(&ens.main_effect(i, |_, c| c.work_ratio.log2())),
        ));
    }
    let mut interactions = Vec::new();
    for (a, name_a) in FACTORS.iter().enumerate() {
        for (b, name_b) in FACTORS.iter().enumerate().skip(a + 1) {
            interactions.push(format!(
                "    {{\"factors\": [\"{name_a}\", \"{name_b}\"], \"on_distance\": {}}}",
                summary_json(&ens.interaction(a, b, |fac, c| ens.distance(fac, c)))
            ));
        }
    }
    let pareto: Vec<String> = ens.pareto().iter().map(|l| format!("\"{l}\"")).collect();
    let sd: Vec<String> = OBSERVABLES
        .iter()
        .zip(ens.reference_sd)
        .map(|(k, v)| format!("\"{k}\": {}", json_f64(v)))
        .collect();
    format!(
        "{{\n  \"seeds\": [{}],\n  \"pairwise_null\": {},\n  \"reference_sd\": {{{}}},\n  \"settings\": [\n{}\n  ],\n  \"main_effects\": [\n{}\n  ],\n  \"interactions\": [\n{}\n  ],\n  \"pareto\": [{}]\n}}\n",
        seeds.join(", "),
        summary_json(&Summary::of(ens.pairwise_null.iter().copied())),
        sd.join(", "),
        rows.join(",\n"),
        effects.join(",\n"),
        interactions.join(",\n"),
        pareto.join(", "),
    )
}

pub fn write_ensemble(ens: &FactorialEnsemble, out_dir: &Path) -> io::Result<Written> {
    std::fs::create_dir_all(out_dir)?;
    let csv: PathBuf = out_dir.join("ensemble.csv");
    let json: PathBuf = out_dir.join("ensemble.json");
    std::fs::write(&csv, ensemble_to_csv(ens))?;
    std::fs::write(&json, ensemble_to_json(ens))?;
    Ok(Written { csv, json })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::Degradation;
    use crate::config::{Config, ReportCfg, WorldCfg};
    use crate::constraints::{Constraints, Params};
    use crate::limits::{run_ensemble, run_factorial};
    use crate::observer::Probe;
    use crate::physics::Rules;

    fn cfg() -> Config {
        Config {
            world: WorldCfg {
                width: 32,
                height: 32,
                ticks: 10,
                seed: 5,
                init_density: 0.3,
                seeds: 2,
                seed_stride: 1000,
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
        }
    }

    #[test]
    fn the_verdicts_end_with_the_framing_sentence() {
        let f = run_factorial(&cfg(), |_| {});
        assert!(summary(&f).ends_with(FRAMING));
        let ens = run_ensemble(&cfg(), |_| {});
        assert!(ensemble_summary(&ens).ends_with(FRAMING));
        assert!(FRAMING.contains("says nothing about whether our universe works this way"));
    }

    #[test]
    fn the_csv_has_one_row_per_cell_null_and_ablation() {
        let f = run_factorial(&cfg(), |_| {});
        let csv = to_csv(&f);
        assert_eq!(csv.lines().count(), 1 + 16 + 4 + 4);
        assert_eq!(
            csv.lines().next().unwrap().split(',').count(),
            CELL_COLUMNS.len()
        );
        let traces = traces_to_csv(&f);
        assert_eq!(traces.lines().count(), 1 + 10);
    }

    #[test]
    fn the_json_is_balanced_and_names_the_nulls() {
        let f = run_factorial(&cfg(), |_| {});
        let j = to_json(&f);
        assert_eq!(
            j.chars().filter(|c| *c == '{').count(),
            j.chars().filter(|c| *c == '}').count()
        );
        for n in NULLS {
            assert!(j.contains(&format!("\"label\": \"{n}\"")));
        }
        let ens = run_ensemble(&cfg(), |_| {});
        let j = ensemble_to_json(&ens);
        assert_eq!(
            j.chars().filter(|c| *c == '{').count(),
            j.chars().filter(|c| *c == '}').count()
        );
        assert!(j.contains("\"pareto\""));
        assert!(j.contains("\"main_effects\""));
        let csv = ensemble_to_csv(&ens);
        assert_eq!(csv.lines().count(), 1 + 2 * (16 + 4 + 4));
    }

    #[test]
    fn counts_carry_their_interval() {
        assert!(share_cell(20, 20).starts_with("20/20 [0.84, 1.00]"));
        assert_eq!(summary_cell(&Summary::of([]), 2), "n/a");
        assert!(summary_cell(&Summary::of([1.0]), 2).contains("n=1"));
    }
}
