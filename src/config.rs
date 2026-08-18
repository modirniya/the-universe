//! The creator's input file. Everything a run needs, and nothing it computes.
//!
//! A config plus a seed fully determines a universe. If that stops being true,
//! the project's central engineering rule has been broken.

use crate::constraints::{Constraints, Params};
use crate::observer::Probe;
use crate::physics::Rules;
use serde::Deserialize;
use std::path::Path;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub world: WorldCfg,
    #[serde(default)]
    pub rules: Rules,
    #[serde(default = "Constraints::all_on")]
    pub constraints: Constraints,
    #[serde(default)]
    pub params: Params,
    pub observer: Probe,
    #[serde(default)]
    pub report: ReportCfg,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldCfg {
    /// Width in base cells. Subdividing space multiplies this internally.
    pub width: usize,
    /// Height in base cells.
    pub height: usize,
    /// Ticks to run. A tick is one unit of the universe's own time.
    pub ticks: u64,
    /// The creator's seed. Same seed, same universe.
    pub seed: u64,
    /// Fraction of cells alive at the input event.
    pub init_density: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportCfg {
    /// Edge of the macro grid used for cross-run comparison: the scale at
    /// which an outside observer notices anything at all.
    pub macro_grid: usize,
    pub out_dir: String,
}

impl Default for ReportCfg {
    fn default() -> Self {
        ReportCfg {
            macro_grid: 16,
            out_dir: "out".to_string(),
        }
    }
}

impl Constraints {
    fn all_on() -> Constraints {
        Constraints::ALL_ON
    }
}

#[derive(Debug)]
pub enum ConfigError {
    Read(std::io::Error),
    Parse(toml::de::Error),
    Invalid(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Read(e) => write!(f, "could not read config: {e}"),
            ConfigError::Parse(e) => write!(f, "could not parse config: {e}"),
            ConfigError::Invalid(m) => write!(f, "config is not runnable: {m}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl Config {
    pub fn load(path: &Path) -> Result<Config, ConfigError> {
        let text = std::fs::read_to_string(path).map_err(ConfigError::Read)?;
        let cfg: Config = toml::from_str(&text).map_err(ConfigError::Parse)?;
        cfg.validate()?;
        Ok(cfg)
    }

    /// Reject configs that would produce a meaningless run, with a message
    /// that names the field rather than the symptom.
    pub fn validate(&self) -> Result<(), ConfigError> {
        let bad = |m: String| Err(ConfigError::Invalid(m));
        if self.world.width == 0 || self.world.height == 0 {
            return bad("world.width and world.height must be greater than 0".into());
        }
        if self.world.ticks == 0 {
            return bad("world.ticks must be greater than 0".into());
        }
        if !(0.0..=1.0).contains(&self.world.init_density) {
            return bad(format!(
                "world.init_density must be in [0, 1], got {}",
                self.world.init_density
            ));
        }
        if self.params.subdivision == 0 || self.params.substeps == 0 {
            return bad("params.subdivision and params.substeps must be at least 1".into());
        }
        if self.params.block_size == 0 {
            return bad("params.block_size must be at least 1".into());
        }
        if self.params.capped_radius == 0 || self.params.uncapped_radius == 0 {
            return bad("influence radii must be at least 1 cell".into());
        }
        if self.params.capped_radius > self.params.uncapped_radius {
            return bad(format!(
                "params.capped_radius ({}) exceeds params.uncapped_radius ({}): \
                 the speed cap would make the universe more expensive, not less",
                self.params.capped_radius, self.params.uncapped_radius
            ));
        }
        if self.report.macro_grid == 0 {
            return bad("report.macro_grid must be at least 1".into());
        }
        if self.observer.width == 0 || self.observer.height == 0 {
            return bad(
                "observer.width and observer.height must be greater than 0: a probe that \
                 observes nothing never renders anything"
                    .into(),
            );
        }
        if self.observer.x >= self.world.width || self.observer.y >= self.world.height {
            return bad("observer origin lies outside the world".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"
[world]
width = 32
height = 32
ticks = 10
seed = 1
init_density = 0.3

[observer]
x = 0
y = 0
width = 8
height = 8
"#;

    fn parse(s: &str) -> Result<Config, ConfigError> {
        let cfg: Config = toml::from_str(s).map_err(ConfigError::Parse)?;
        cfg.validate()?;
        Ok(cfg)
    }

    #[test]
    fn defaults_fill_in_the_optional_tables() {
        let cfg = parse(MINIMAL).expect("minimal config should load");
        assert_eq!(cfg.constraints, Constraints::ALL_ON);
        assert_eq!(cfg.report.macro_grid, 16);
        assert_eq!(cfg.params.block_size, 16);
        assert!((cfg.rules.birth_lo - 0.3125).abs() < 1e-12);
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let s = MINIMAL.replace("[world]", "[world]\nwidht = 32");
        assert!(matches!(parse(&s), Err(ConfigError::Parse(_))));
    }

    #[test]
    fn zero_ticks_is_refused() {
        let s = MINIMAL.replace("ticks = 10", "ticks = 0");
        assert!(matches!(parse(&s), Err(ConfigError::Invalid(_))));
    }

    #[test]
    fn density_outside_the_unit_interval_is_refused() {
        let s = MINIMAL.replace("init_density = 0.3", "init_density = 1.5");
        assert!(matches!(parse(&s), Err(ConfigError::Invalid(_))));
    }

    #[test]
    fn probe_outside_the_world_is_refused() {
        let s = MINIMAL.replace("x = 0", "x = 999");
        assert!(matches!(parse(&s), Err(ConfigError::Invalid(_))));
    }

    #[test]
    fn an_inverted_speed_cap_is_refused() {
        let s = format!(
            "{MINIMAL}\n[params]\ncapped_radius = 5\nuncapped_radius = 2\nsubdivision = 2\nsubsteps = 2\nblock_size = 16\n"
        );
        match parse(&s) {
            Err(ConfigError::Invalid(m)) => assert!(m.contains("more expensive")),
            other => panic!("expected a validation error, got {other:?}"),
        }
    }
}
