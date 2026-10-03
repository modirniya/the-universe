//! The measure problem's report: the productive share under every prior,
//! criterion and setting.

use super::{Written, seed_list};
use crate::measure::{MeasureRun, PRIORS, SETTINGS, SettingRun, interval};
use crate::stats::Summary;
use std::fmt::Write as _;
use std::io;
use std::path::{Path, PathBuf};

/// The sentence every measure verdict ends with.
pub const FRAMING: &str = "a productive fraction is a property of a prior and a criterion as much as of the laws. \
nothing here bears on the constants of our universe.\n";

fn criterion_labels(run: &MeasureRun) -> Vec<&'static str> {
    run.criteria.iter().map(|c| c.label()).collect()
}

/// The pinned seed: priors against criteria, with Wilson intervals.
pub fn summary(run: &MeasureRun) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "seed {}: {} band laws and {} life-like rules scored, each once",
        run.seed,
        run.bands.len(),
        run.lifelike.len()
    );
    let _ = writeln!(
        s,
        "share of laws a criterion admits, under each way of counting laws; k/n [Wilson 95%]\n"
    );
    let _ = write!(s, "{:<18} {:>6}", "prior", "n");
    for c in criterion_labels(run) {
        let _ = write!(s, " {:>30}", c);
    }
    s.push('\n');
    let _ = writeln!(s, "{}", "-".repeat(26 + 31 * run.criteria.len()));
    for p in PRIORS {
        let (_, n) = run.fraction(p, 0);
        let _ = write!(s, "{:<18} {:>6}", p.label(), n);
        for c in 0..run.criteria.len() {
            let (k, n) = run.count(p, c);
            let (lo, hi) = interval(run, p, c);
            let _ = write!(
                s,
                " {:>30}",
                format!("{:.3} ({k}/{n}) [{lo:.2}, {hi:.2}]", run.fraction(p, c).0)
            );
        }
        s.push('\n');
    }
    let (lo, hi) = run.range();
    let _ = writeln!(
        s,
        "\nat this seed the productive share runs from {:.1}% to {:.1}% depending on the prior and the\n\
         criterion: a factor of {:.1}. the grid prior weights a law by how many grid points land on it;\n\
         the band prior weights every expressible band rule equally; the life-like prior samples rules\n\
         the band form cannot state. none is privileged by anything in the model.",
        lo * 100.0,
        hi * 100.0,
        if lo > 0.0 { hi / lo } else { f64::INFINITY }
    );
    s.push_str(FRAMING);
    s
}

/// Across seeds: mean ± sd [95% ci] per prior and criterion.
pub fn ensemble_summary(runs: &[(u64, MeasureRun)]) -> String {
    let mut s = String::new();
    let seeds: Vec<u64> = runs.iter().map(|(s, _)| *s).collect();
    let Some((_, first)) = runs.first() else {
        return s;
    };
    let _ = writeln!(
        s,
        "ensemble: {} seeds ({}); share of laws admitted, mean ± sd [95% ci] over seeds",
        runs.len(),
        seed_list(&seeds)
    );
    let _ = write!(s, "{:<18}", "prior");
    for c in criterion_labels(first) {
        let _ = write!(s, " {:>30}", c);
    }
    s.push('\n');
    for p in PRIORS {
        let _ = write!(s, "{:<18}", p.label());
        for c in 0..first.criteria.len() {
            let sm = Summary::of(runs.iter().map(|(_, r)| r.fraction(p, c).0));
            let _ = write!(s, " {:>30}", super::limits::summary_cell(&sm, 3));
        }
        s.push('\n');
    }
    let lo = Summary::of(runs.iter().map(|(_, r)| r.range().0));
    let hi = Summary::of(runs.iter().map(|(_, r)| r.range().1));
    let _ = writeln!(
        s,
        "\nlowest share over priors and criteria: {}; highest: {}",
        super::limits::summary_cell(&lo, 3),
        super::limits::summary_cell(&hi, 3)
    );
    let majority: Vec<String> = PRIORS
        .iter()
        .flat_map(|p| {
            (0..first.criteria.len()).filter_map(move |c| {
                let sm = Summary::of(runs.iter().map(|(_, r)| r.fraction(*p, c).0));
                (sm.mean >= 0.5)
                    .then(|| format!("{} under {}", p.label(), first.criteria[c].label()))
            })
        })
        .collect();
    if majority.is_empty() {
        let _ = writeln!(
            s,
            "productive laws are a minority under every prior and criterion."
        );
    } else {
        let _ = writeln!(
            s,
            "productive laws are a MAJORITY under: {}. 'fine-tuning' is the wrong word there.",
            majority.join("; ")
        );
    }
    s.push_str(FRAMING);
    s
}

/// The band family in other universes.
pub fn sensitivity_summary(runs: &[(u64, Vec<SettingRun>)]) -> String {
    let mut s = String::new();
    let Some((_, first)) = runs.first() else {
        return s;
    };
    let seeds: Vec<u64> = runs.iter().map(|(s, _)| *s).collect();
    let _ = writeln!(
        s,
        "sensitivity: the band family ({} laws) scored in other universes, {} seeds ({}); share\n\
         admitted, mean ± sd [95% ci] over seeds, and whether Conway itself passes in every seed",
        2116,
        runs.len(),
        seed_list(&seeds)
    );
    let labels = ["resembles conway", "compressibility", "perturbation growth"];
    let _ = write!(s, "{:<14}", "setting");
    for l in labels {
        let _ = write!(s, " {:>30} {:>7}", l, "conway");
    }
    s.push('\n');
    for (i, setting) in SETTINGS.iter().enumerate() {
        if first.get(i).is_none() {
            continue;
        }
        let _ = write!(s, "{:<14}", setting.label);
        for c in 0..3 {
            let sm = Summary::of(
                runs.iter()
                    .filter_map(|(_, v)| v.get(i).map(|r| r.fractions[c])),
            );
            let conway = runs
                .iter()
                .filter(|(_, v)| v.get(i).is_some_and(|r| r.conway_admitted[c]))
                .count();
            let _ = write!(
                s,
                " {:>30} {:>7}",
                super::limits::summary_cell(&sm, 3),
                format!("{conway}/{}", runs.len())
            );
        }
        s.push('\n');
    }
    let _ = writeln!(
        s,
        "\nthe criteria's bands were fixed on the baseline universe. a share that moves with the\n\
         setting is a share that depends on where the laws are tried, not only on the laws."
    );
    s
}

const COLUMNS: &[&str] = &[
    "seed",
    "prior",
    "criterion",
    "fraction",
    "admitted",
    "n",
    "wilson_lo",
    "wilson_hi",
];

pub fn to_csv(runs: &[(u64, MeasureRun)]) -> String {
    let mut s = COLUMNS.join(",");
    s.push('\n');
    for (seed, r) in runs {
        for p in PRIORS {
            for c in 0..r.criteria.len() {
                let (f, n) = r.fraction(p, c);
                let (k, _) = r.count(p, c);
                let (lo, hi) = interval(r, p, c);
                let _ = writeln!(
                    s,
                    "{seed},{},{},{:.6},{k},{n},{:.6},{:.6}",
                    p.label(),
                    r.criteria[c].label(),
                    f,
                    lo,
                    hi
                );
            }
        }
    }
    s
}

const LAW_COLUMNS: &[&str] = &[
    "family",
    "birth_mask",
    "survive_mask",
    "signature",
    "final_live",
    "activity",
    "dispersion",
    "compressibility",
    "growth",
    "resembles_conway",
    "compressible",
    "growing",
];

/// Every law of the pinned seed with its profile and verdicts, so the priors
/// can be re-weighted without re-running anything.
pub fn laws_to_csv(run: &MeasureRun) -> String {
    let mut s = LAW_COLUMNS.join(",");
    s.push('\n');
    for (family, laws) in [("bands", &run.bands), ("lifelike", &run.lifelike)] {
        for l in laws {
            let p = &l.profile;
            let _ = writeln!(
                s,
                "{family},{},{},{},{:.6},{:.6},{:.6},{:.6},{:.6},{},{},{}",
                l.birth,
                l.survive,
                l.signature,
                p.final_live,
                p.activity,
                p.dispersion,
                p.compressibility,
                p.growth,
                run.criteria[0].admits(p),
                run.criteria[1].admits(p),
                run.criteria[2].admits(p),
            );
        }
    }
    s
}

const SENS_COLUMNS: &[&str] = &[
    "seed",
    "setting",
    "edge",
    "ticks",
    "density",
    "criterion",
    "fraction",
    "conway_admitted",
];

pub fn sensitivity_to_csv(runs: &[(u64, Vec<SettingRun>)]) -> String {
    let mut s = SENS_COLUMNS.join(",");
    s.push('\n');
    let labels = ["resembles conway", "compressibility", "perturbation growth"];
    for (seed, v) in runs {
        for r in v {
            for (c, label) in labels.iter().enumerate() {
                let _ = writeln!(
                    s,
                    "{seed},{},{},{},{},{},{:.6},{}",
                    r.setting.label,
                    r.setting.edge,
                    r.setting.ticks,
                    r.setting.density,
                    label,
                    r.fractions[c],
                    r.conway_admitted[c]
                );
            }
        }
    }
    s
}

pub fn to_json(runs: &[(u64, MeasureRun)]) -> String {
    let Some((_, first)) = runs.first() else {
        return "{}\n".to_string();
    };
    let mut rows = Vec::new();
    for p in PRIORS {
        for c in 0..first.criteria.len() {
            let sm = Summary::of(runs.iter().map(|(_, r)| r.fraction(p, c).0));
            let (_, n) = first.fraction(p, c);
            rows.push(format!(
                "    {{\"prior\": \"{}\", \"criterion\": \"{}\", \"n\": {n}, \"share\": {}}}",
                p.label(),
                first.criteria[c].label(),
                super::limits::summary_json(&sm)
            ));
        }
    }
    let lo = Summary::of(runs.iter().map(|(_, r)| r.range().0));
    let hi = Summary::of(runs.iter().map(|(_, r)| r.range().1));
    let seeds: Vec<String> = runs.iter().map(|(s, _)| s.to_string()).collect();
    format!(
        "{{\n  \"seeds\": [{}],\n  \"band_laws\": {},\n  \"lifelike_sample\": {},\n  \"lowest_share\": {},\n  \"highest_share\": {},\n  \"shares\": [\n{}\n  ]\n}}\n",
        seeds.join(", "),
        first.bands.len(),
        first.lifelike.len(),
        super::limits::summary_json(&lo),
        super::limits::summary_json(&hi),
        rows.join(",\n")
    )
}

pub fn write(runs: &[(u64, MeasureRun)], out_dir: &Path) -> io::Result<Written> {
    std::fs::create_dir_all(out_dir)?;
    let csv: PathBuf = out_dir.join("measure.csv");
    let json: PathBuf = out_dir.join("measure.json");
    std::fs::write(&csv, to_csv(runs))?;
    std::fs::write(&json, to_json(runs))?;
    if let Some((_, first)) = runs.first() {
        std::fs::write(out_dir.join("laws.csv"), laws_to_csv(first))?;
    }
    Ok(Written { csv, json })
}

pub fn write_sensitivity(runs: &[(u64, Vec<SettingRun>)], out_dir: &Path) -> io::Result<PathBuf> {
    std::fs::create_dir_all(out_dir)?;
    let path = out_dir.join("measure_sensitivity.csv");
    std::fs::write(&path, sensitivity_to_csv(runs))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::Degradation;
    use crate::config::{Config, ReportCfg, WorldCfg};
    use crate::constraints::{Constraints, Params};
    use crate::measure::{SETTINGS, run_measure, run_setting};
    use crate::observer::Probe;
    use crate::physics::Rules;

    fn cfg() -> Config {
        Config {
            world: WorldCfg {
                width: 16,
                height: 16,
                ticks: 10,
                seed: 42,
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
                x: 0,
                y: 0,
                width: 16,
                height: 16,
            },
            report: ReportCfg {
                macro_grid: 4,
                out_dir: "out".into(),
            },
            nesting: Degradation::default(),
            horizon: crate::pipe::Horizon::default(),
        }
    }

    #[test]
    fn the_reports_are_well_formed_and_end_with_the_framing() {
        let run = run_measure(&cfg());
        let runs = vec![(42u64, run.clone())];
        assert!(summary(&run).ends_with(FRAMING));
        assert!(ensemble_summary(&runs).ends_with(FRAMING));
        let csv = to_csv(&runs);
        assert_eq!(csv.lines().count(), 1 + PRIORS.len() * 3);
        for line in csv.lines() {
            assert_eq!(line.split(',').count(), COLUMNS.len());
        }
        let laws = laws_to_csv(&run);
        assert_eq!(
            laws.lines().count(),
            1 + run.bands.len() + run.lifelike.len()
        );
        let j = to_json(&runs);
        assert_eq!(
            j.chars().filter(|c| *c == '{').count(),
            j.chars().filter(|c| *c == '}').count()
        );
        let mut c = cfg();
        c.world.ticks = 6;
        let s = run_setting(&c, SETTINGS[1]);
        let sens = vec![(42u64, vec![s])];
        assert!(sensitivity_summary(&sens).contains("sensitivity"));
        assert_eq!(sensitivity_to_csv(&sens).lines().count(), 4);
    }
}
