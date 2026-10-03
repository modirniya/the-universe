//! Where a run's numbers came from, written beside them.
//!
//! Every documented command writes `metadata.json` into its output directory
//! with the source revision, whether the tree was clean, the compiler and
//! target, the exact configuration text and a fingerprint of it, the seeds
//! that ran, and version numbers for the experiment's design and for its
//! metrics. A result without this file is a number without a provenance, and
//! `analysis/claims.toml` records the commit each claim was last verified at
//! so the two can be compared.
//!
//! Nothing here reads a clock. A run is identified by what produced it, not by
//! when; the project's determinism rule keeps wall-clock time out of every
//! artifact except the one column that is explicitly a measurement.

use crate::config::Config;
use crate::experiment::ensemble_seeds;
use crate::rng::Rng;
use std::fmt::Write as _;
use std::io;
use std::path::{Path, PathBuf};

/// The design version of each command's experiment, bumped whenever what the
/// command measures or how it decides changes. A claim verified under one
/// design version does not carry over to the next.
pub fn experiment_version(command: &str) -> u32 {
    match command {
        // v1.0: factorial, four nulls, seven observables, closure ablation.
        "run" => 2,
        // v1.0: termination map, size control, churn against size.
        "nest" => 2,
        // v1.0: the information analysis beside the correlation sweep.
        "pipe" => 2,
        // v1.0: hypothesis tests with calibration/evaluation split and controls.
        "detect" => 2,
        "sweep" => 1,
        // v1.0: shuffle and area controls, densities.
        "boot" => 2,
        "edge" => 1,
        "measure" => 1,
        _ => 0,
    }
}

/// The version of the metric definitions a command reports. Bumped when a
/// statistic's definition changes (not when a new one is added).
pub const METRIC_VERSION: u32 = 2;

/// Everything recorded about a run.
#[derive(Clone, Debug)]
pub struct Provenance {
    pub command: String,
    pub config_path: String,
    pub config_text: String,
    pub config_fingerprint: u64,
    pub seeds: Vec<u64>,
    pub crate_version: &'static str,
    pub commit: &'static str,
    pub dirty: &'static str,
    pub rustc: &'static str,
    pub target: &'static str,
    pub profile: &'static str,
    pub experiment_version: u32,
    pub metric_version: u32,
}

/// Fold a text into one number with the project's own generator, so two
/// configs agree here exactly when their text agrees byte for byte.
pub fn fingerprint_text(text: &str) -> u64 {
    let mut h = Rng::derive(0x434F_4E46_4947, text.len() as u64, 0, 0).next_u64();
    for (i, b) in text.bytes().enumerate() {
        h = Rng::derive(h, u64::from(b), i as u64, 1).next_u64();
    }
    h
}

impl Provenance {
    pub fn new(command: &str, config_path: &Path, config_text: &str, cfg: &Config) -> Provenance {
        Provenance {
            command: command.to_string(),
            config_path: config_path.display().to_string(),
            config_text: config_text.to_string(),
            config_fingerprint: fingerprint_text(config_text),
            seeds: ensemble_seeds(cfg),
            crate_version: env!("CARGO_PKG_VERSION"),
            commit: env!("UNIVERSE_GIT_COMMIT"),
            dirty: env!("UNIVERSE_GIT_DIRTY"),
            rustc: env!("UNIVERSE_RUSTC"),
            target: env!("UNIVERSE_TARGET"),
            profile: env!("UNIVERSE_PROFILE"),
            experiment_version: experiment_version(command),
            metric_version: METRIC_VERSION,
        }
    }

    pub fn to_json(&self) -> String {
        let seeds: Vec<String> = self.seeds.iter().map(|s| s.to_string()).collect();
        let mut s = String::from("{\n");
        let _ = writeln!(s, "  \"command\": \"{}\",", self.command);
        let _ = writeln!(s, "  \"crate_version\": \"{}\",", self.crate_version);
        let _ = writeln!(s, "  \"experiment_version\": {},", self.experiment_version);
        let _ = writeln!(s, "  \"metric_version\": {},", self.metric_version);
        let _ = writeln!(s, "  \"commit\": \"{}\",", self.commit);
        let _ = writeln!(s, "  \"dirty\": {},", json_flag(self.dirty));
        let _ = writeln!(s, "  \"rustc\": \"{}\",", self.rustc);
        let _ = writeln!(s, "  \"target\": \"{}\",", self.target);
        let _ = writeln!(s, "  \"profile\": \"{}\",", self.profile);
        let _ = writeln!(s, "  \"config_path\": \"{}\",", escape(&self.config_path));
        let _ = writeln!(
            s,
            "  \"config_fingerprint\": \"{:016x}\",",
            self.config_fingerprint
        );
        let _ = writeln!(s, "  \"seeds\": [{}],", seeds.join(", "));
        let _ = writeln!(s, "  \"config_text\": \"{}\"", escape(&self.config_text));
        s.push_str("}\n");
        s
    }

    /// Write `metadata.json` into an output directory.
    pub fn write(&self, out_dir: &Path) -> io::Result<PathBuf> {
        std::fs::create_dir_all(out_dir)?;
        let path = out_dir.join("metadata.json");
        std::fs::write(&path, self.to_json())?;
        Ok(path)
    }
}

fn json_flag(v: &str) -> &str {
    match v {
        "true" | "false" => v,
        _ => "null",
    }
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fingerprint_is_byte_exact() {
        assert_eq!(fingerprint_text("a = 1\n"), fingerprint_text("a = 1\n"));
        assert_ne!(fingerprint_text("a = 1\n"), fingerprint_text("a = 1 \n"));
    }

    #[test]
    fn escaping_round_trips_the_awkward_characters() {
        let e = escape("say \"hi\"\n\ttab\\");
        assert_eq!(e, "say \\\"hi\\\"\\n\\ttab\\\\");
    }

    #[test]
    fn every_command_has_a_design_version_and_the_json_is_balanced() {
        for c in [
            "run", "nest", "pipe", "detect", "sweep", "boot", "edge", "measure",
        ] {
            assert!(experiment_version(c) >= 1, "{c}");
        }
        assert_eq!(experiment_version("nonsense"), 0);
        let cfg: Config = toml::from_str(
            "[world]\nwidth = 8\nheight = 8\nticks = 1\nseed = 3\ninit_density = 0.3\nseeds = 2\n[observer]\nx = 0\ny = 0\nwidth = 4\nheight = 4\n",
        )
        .unwrap();
        let p = Provenance::new("run", Path::new("x.toml"), "text \"q\"", &cfg);
        assert_eq!(p.seeds, vec![3, 1003]);
        let j = p.to_json();
        assert_eq!(
            j.chars().filter(|c| *c == '{').count(),
            j.chars().filter(|c| *c == '}').count()
        );
        assert!(j.contains("\"experiment_version\": 2"));
        assert!(j.contains("\\\"q\\\""));
        assert!(!p.commit.is_empty());
    }
}
