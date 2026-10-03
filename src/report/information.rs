//! The pipe's information report: bits per task per encoding, and the
//! controls.

use super::{Written, json_f64, seed_list};
use crate::information::{Analysis, ENCODINGS, Encoding, Measure, NOISE_RATES, TASKS, Task};
use crate::stats::Summary;
use std::fmt::Write as _;
use std::io;
use std::path::{Path, PathBuf};

/// The sentence every information verdict ends with.
pub const FRAMING: &str = "the pipe here is a quantised channel and nothing more; what survives it is what any \
channel of that width carries about a slowly varying aggregate. this says nothing about \
horizons in our universe.\n";

fn cell(m: &Measure) -> String {
    if m.n < 4 {
        return "    n/a".to_string();
    }
    format!("{:>5.2}/{:<4.2}", m.mi_corrected(), m.predictive)
}

/// The pinned seed: a grid of encodings against tasks.
pub fn summary(a: &Analysis) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "seed {}: {} ticks; the horizon's occupancy carries {:.2} bits of entropy at 64 levels",
        a.seed, a.ticks, a.input_entropy
    );
    let _ = writeln!(
        s,
        "each cell is 'plug-in MI minus its shuffle null / leave-one-out predictive bits', per tick.\n\
         the first says what symbol and target have in common; the second what a parent who must\n\
         learn the code from the rest of the history can use. targets are in {} quantile bins\n\
         (up to {:.2} bits) or 4 or 16 classes.\n",
        crate::information::TARGET_BINS,
        (crate::information::TARGET_BINS as f64).log2()
    );
    let _ = write!(s, "{:<14} {:>6} {:>8}", "encoding", "bits", "H(sym)");
    for t in TASKS {
        let _ = write!(s, " {:>16}", t.label());
    }
    s.push('\n');
    let _ = writeln!(s, "{}", "-".repeat(30 + 17 * TASKS.len()));
    for enc in ENCODINGS {
        let h = a
            .row(enc, Task::HorizonNow)
            .map_or(f64::NAN, |r| r.measure.h_symbol);
        let _ = write!(s, "{:<14} {:>6} {:>8.2}", enc.label(), enc.bits(), h);
        for t in TASKS {
            let m = a.row(enc, t).map(|r| r.measure);
            let _ = write!(s, " {:>16}", m.map_or("n/a".to_string(), |m| cell(&m)));
        }
        s.push('\n');
    }
    s.push('\n');
    let _ = writeln!(s, "controls, against global_now:");
    for (b, m) in &a.window_control {
        let _ = writeln!(
            s,
            "  uniform:{b:<3} from a window half a world away: {}",
            cell(m)
        );
    }
    let _ = writeln!(
        s,
        "  four random bits a tick:                   {}",
        cell(&a.noise_control)
    );
    let _ = writeln!(s, "uniform:4 under bit flips, against global_now:");
    for (eps, m) in &a.noise {
        let _ = writeln!(s, "  flip rate {eps:<5}: {}", cell(m));
    }
    s.push('\n');
    s.push_str(&verdict(a));
    s
}

fn verdict(a: &Analysis) -> String {
    let mut s = String::new();
    let hz = a
        .row(Encoding::Uniform(128), Task::HorizonNow)
        .map_or(f64::NAN, |r| r.measure.mi_corrected());
    let gl = a
        .row(Encoding::Uniform(128), Task::GlobalNow)
        .map_or(f64::NAN, |r| r.measure.mi_corrected());
    let fut = a
        .row(Encoding::Uniform(128), Task::GlobalFuture)
        .map_or(f64::NAN, |r| r.measure.mi_corrected());
    let arr = a
        .row(Encoding::Uniform(128), Task::DensestQuadrant)
        .map_or(f64::NAN, |r| r.measure.mi_corrected());
    let proj = a
        .row(Encoding::Projection(6), Task::DensestQuadrant)
        .map_or(f64::NAN, |r| r.measure.mi_corrected());
    let win = a
        .window_control
        .last()
        .map_or(f64::NAN, |(_, m)| m.mi_corrected());
    let _ = writeln!(
        s,
        "at full width the shipped encoding carries {hz:.2} bits about what it sent, {gl:.2} about\n\
         the whole child now, {fut:.2} about the child {} ticks ahead, and {arr:.2} about which\n\
         quadrant is densest. a window elsewhere carries {win:.2} about the whole child; a six-bit\n\
         projection of the arrangement carries {proj:.2} about the densest quadrant.",
        crate::information::FUTURE_LAG
    );
    let _ = writeln!(
        s,
        "that the magnitude carries occupancy and the hash carries none is how the message was\n\
         built. what had to be run is the number of bits each task survives on, and whether the\n\
         horizon does anything a window elsewhere does not. one seed is an example."
    );
    s.push_str(FRAMING);
    s
}

const COLUMNS: &[&str] = &[
    "seed",
    "kind",
    "encoding",
    "bits",
    "task",
    "n",
    "h_symbol",
    "h_target",
    "mi",
    "mi_null",
    "mi_corrected",
    "predictive",
];

fn row(seed: u64, kind: &str, encoding: &str, bits: u32, task: &str, m: &Measure) -> String {
    format!(
        "{seed},{kind},{encoding},{bits},{task},{},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6}",
        m.n,
        m.h_symbol,
        m.h_target,
        m.mi,
        m.mi_null,
        m.mi_corrected(),
        m.predictive
    )
}

fn rows(a: &Analysis, out: &mut String) {
    for r in &a.rows {
        let _ = writeln!(
            out,
            "{}",
            row(
                a.seed,
                "encoding",
                &r.encoding.label(),
                r.encoding.bits(),
                r.task.label(),
                &r.measure
            )
        );
    }
    for (b, m) in &a.window_control {
        let _ = writeln!(
            out,
            "{}",
            row(
                a.seed,
                "window_control",
                &format!("uniform:{b}"),
                *b,
                "global_now",
                m
            )
        );
    }
    let _ = writeln!(
        out,
        "{}",
        row(
            a.seed,
            "noise_control",
            "random:4",
            4,
            "global_now",
            &a.noise_control
        )
    );
    for (eps, m) in &a.noise {
        let _ = writeln!(
            out,
            "{}",
            row(
                a.seed,
                &format!("noise:{eps}"),
                "uniform:4",
                4,
                "global_now",
                m
            )
        );
    }
}

pub fn to_csv(a: &Analysis) -> String {
    let mut s = COLUMNS.join(",");
    s.push('\n');
    rows(a, &mut s);
    s
}

pub fn ensemble_to_csv(runs: &[(u64, Analysis)]) -> String {
    let mut s = COLUMNS.join(",");
    s.push('\n');
    for (_, a) in runs {
        rows(a, &mut s);
    }
    s
}

fn measure_json(m: &Measure) -> String {
    format!(
        "{{\"n\": {}, \"h_symbol\": {}, \"h_target\": {}, \"mi\": {}, \"mi_null\": {}, \"mi_corrected\": {}, \"predictive\": {}}}",
        m.n,
        json_f64(m.h_symbol),
        json_f64(m.h_target),
        json_f64(m.mi),
        json_f64(m.mi_null),
        json_f64(m.mi_corrected()),
        json_f64(m.predictive)
    )
}

pub fn to_json(a: &Analysis) -> String {
    let rows: Vec<String> = a
        .rows
        .iter()
        .map(|r| {
            format!(
                "    {{\"encoding\": \"{}\", \"bits\": {}, \"task\": \"{}\", \"measure\": {}}}",
                r.encoding.label(),
                r.encoding.bits(),
                r.task.label(),
                measure_json(&r.measure)
            )
        })
        .collect();
    let window: Vec<String> = a
        .window_control
        .iter()
        .map(|(b, m)| format!("    {{\"bits\": {b}, \"measure\": {}}}", measure_json(m)))
        .collect();
    let noise: Vec<String> = a
        .noise
        .iter()
        .map(|(e, m)| {
            format!(
                "    {{\"flip_rate\": {e}, \"measure\": {}}}",
                measure_json(m)
            )
        })
        .collect();
    format!(
        "{{\n  \"seed\": {},\n  \"ticks\": {},\n  \"input_entropy\": {},\n  \"rows\": [\n{}\n  ],\n  \"window_control\": [\n{}\n  ],\n  \"noise_control\": {},\n  \"noise\": [\n{}\n  ]\n}}\n",
        a.seed,
        a.ticks,
        json_f64(a.input_entropy),
        rows.join(",\n"),
        window.join(",\n"),
        measure_json(&a.noise_control),
        noise.join(",\n")
    )
}

pub fn write(a: &Analysis, out_dir: &Path) -> io::Result<Written> {
    std::fs::create_dir_all(out_dir)?;
    let csv: PathBuf = out_dir.join("information.csv");
    let json: PathBuf = out_dir.join("information.json");
    std::fs::write(&csv, to_csv(a))?;
    std::fs::write(&json, to_json(a))?;
    Ok(Written { csv, json })
}

/// Across seeds: the key cells, as `mean ± sd [95% ci]`.
pub fn ensemble_summary(runs: &[(u64, Analysis)]) -> String {
    let mut s = String::new();
    let seeds: Vec<u64> = runs.iter().map(|(s, _)| *s).collect();
    let _ = writeln!(
        s,
        "ensemble: {} seeds ({}); corrected MI in bits per tick, mean ± sd [95% ci]",
        runs.len(),
        seed_list(&seeds)
    );
    let of = |f: &dyn Fn(&Analysis) -> f64| Summary::of(runs.iter().map(|(_, a)| f(a)));
    let fmt = |sm: &Summary| super::limits::summary_cell(sm, 2);
    let _ = writeln!(
        s,
        "{:<14} {:>5} {}",
        "encoding",
        "bits",
        TASKS
            .iter()
            .map(|t| format!("{:>28}", t.label()))
            .collect::<String>()
    );
    for enc in [
        Encoding::Uniform(1),
        Encoding::Uniform(2),
        Encoding::Uniform(3),
        Encoding::Uniform(4),
        Encoding::Uniform(6),
        Encoding::Uniform(128),
        Encoding::Adaptive(4),
        Encoding::Hash(6),
        Encoding::Projection(6),
        Encoding::Quadrants,
    ] {
        let _ = write!(s, "{:<14} {:>5}", enc.label(), enc.bits());
        for t in TASKS {
            let sm = of(&|a| a.row(enc, t).map_or(f64::NAN, |r| r.measure.mi_corrected()));
            let _ = write!(s, " {:>28}", fmt(&sm));
        }
        s.push('\n');
    }
    s.push('\n');
    let _ = writeln!(s, "leave-one-out predictive bits for the shipped encoding:");
    for enc in [
        Encoding::Uniform(2),
        Encoding::Uniform(4),
        Encoding::Uniform(128),
    ] {
        let _ = write!(s, "{:<14} {:>5}", enc.label(), enc.bits());
        for t in TASKS {
            let sm = of(&|a| a.row(enc, t).map_or(f64::NAN, |r| r.measure.predictive));
            let _ = write!(s, " {:>28}", fmt(&sm));
        }
        s.push('\n');
    }
    s.push('\n');
    let _ = writeln!(s, "controls, against global_now (corrected MI):");
    for (i, b) in [2u32, 4, 6, 128].iter().enumerate() {
        let sm = of(&|a| {
            a.window_control
                .get(i)
                .map_or(f64::NAN, |(_, m)| m.mi_corrected())
        });
        let _ = writeln!(s, "  uniform:{b:<3} from a window elsewhere  {}", fmt(&sm));
    }
    let _ = writeln!(
        s,
        "  four random bits a tick              {}",
        fmt(&of(&|a| a.noise_control.mi_corrected()))
    );
    let _ = writeln!(
        s,
        "uniform:4 under bit flips, against global_now (corrected MI):"
    );
    for (i, eps) in NOISE_RATES.iter().enumerate() {
        let sm = of(&|a| a.noise.get(i).map_or(f64::NAN, |(_, m)| m.mi_corrected()));
        let _ = writeln!(s, "  flip rate {eps:<5}  {}", fmt(&sm));
    }
    let _ = writeln!(
        s,
        "input entropy of the horizon's occupancy at 64 levels: {}\n",
        fmt(&of(&|a| a.input_entropy))
    );

    // Where the shipped encoding stops gaining: the narrowest width whose
    // corrected MI on global_now is within 90% of the full width's, per seed.
    let widths: Vec<f64> = runs
        .iter()
        .map(|(_, a)| {
            let full = a
                .row(Encoding::Uniform(128), Task::GlobalNow)
                .map_or(f64::NAN, |r| r.measure.mi_corrected());
            [1u32, 2, 3, 4, 6, 128]
                .iter()
                .find(|b| {
                    a.row(Encoding::Uniform(**b), Task::GlobalNow)
                        .is_some_and(|r| r.measure.mi_corrected() >= 0.9 * full)
                })
                .map_or(f64::NAN, |b| f64::from(*b))
        })
        .collect();
    let _ = writeln!(
        s,
        "narrowest width keeping 90% of the full-width information about global_now: {}",
        fmt(&Summary::of(widths))
    );
    let horizon = of(&|a| {
        a.row(Encoding::Uniform(128), Task::GlobalNow)
            .map_or(f64::NAN, |r| r.measure.mi_corrected())
    });
    let window = of(&|a| {
        a.window_control
            .last()
            .map_or(f64::NAN, |(_, m)| m.mi_corrected())
    });
    let _ = writeln!(
        s,
        "the horizon carries {} bits about the whole child; a window elsewhere carries {}.\n\
         if those agree, the horizon is a window and the mechanism adds nothing.",
        fmt(&horizon),
        fmt(&window)
    );
    s.push_str(FRAMING);
    s
}

pub fn write_ensemble(runs: &[(u64, Analysis)], out_dir: &Path) -> io::Result<PathBuf> {
    std::fs::create_dir_all(out_dir)?;
    let path = out_dir.join("information_ensemble.csv");
    std::fs::write(&path, ensemble_to_csv(runs))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::information::ChildTrace;
    use crate::information::analyse_trace;
    use crate::rng::Rng;

    fn trace(seed: u64) -> ChildTrace {
        let mut rng = Rng::new(seed);
        let mut occ = 0.3;
        let mut t = ChildTrace {
            horizon_occupancy: Vec::new(),
            global_occupancy: Vec::new(),
            quadrants: Vec::new(),
            digest: Vec::new(),
            projections: Vec::new(),
            window_occupancy: Vec::new(),
        };
        for _ in 0..120 {
            occ = (occ + 0.02 * (rng.next_f64() - 0.5)).clamp(0.05, 0.6);
            t.horizon_occupancy.push(occ);
            t.global_occupancy.push(occ);
            t.quadrants.push([occ, occ * 0.5, occ * 1.5, occ]);
            t.digest.push(rng.next_u64());
            t.projections.push((rng.next_u64() & 0xFF) as u32);
            t.window_occupancy.push(occ);
        }
        t
    }

    #[test]
    fn the_summaries_end_with_the_framing_and_the_files_are_well_formed() {
        let a = analyse_trace(&trace(1), 1);
        assert!(summary(&a).ends_with(FRAMING));
        let csv = to_csv(&a);
        for line in csv.lines() {
            assert_eq!(line.split(',').count(), COLUMNS.len(), "{line}");
        }
        assert_eq!(
            csv.lines().count(),
            1 + ENCODINGS.len() * TASKS.len() + 4 + 1 + NOISE_RATES.len()
        );
        let j = to_json(&a);
        assert_eq!(
            j.chars().filter(|c| *c == '{').count(),
            j.chars().filter(|c| *c == '}').count()
        );
        let runs = vec![(1u64, a.clone()), (2, analyse_trace(&trace(2), 2))];
        let e = ensemble_summary(&runs);
        assert!(e.ends_with(FRAMING));
        assert!(e.contains("2 seeds"));
        assert_eq!(
            ensemble_to_csv(&runs).lines().count(),
            2 * (csv.lines().count() - 1) + 1
        );
    }
}
