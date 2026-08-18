//! CLI entry point. Argument parsing is done by hand: the dependency list is
//! deliberately short, and this binary has exactly one subcommand.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use the_universe::config::Config;
use the_universe::experiment;
use the_universe::report;

const USAGE: &str = "\
the-universe — a runnable model of a simulation-hypothesis framework

USAGE:
    the-universe run --config <FILE> [OPTIONS]

OPTIONS:
    --config <FILE>   Universe definition (TOML). Required.
    --out <DIR>       Where to write runs.csv and report.json.
                      Defaults to the config's report.out_dir.
    --seed <N>        Override world.seed. Same seed, same universe.
    --ticks <N>       Override world.ticks.
    -h, --help        Print this.

The run compares an unconstrained universe against one with each limit in
force, and reports what the limits cost and what they changed.
";

fn main() -> ExitCode {
    match parse(std::env::args().skip(1).collect()) {
        Ok(None) => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Ok(Some(args)) => match execute(&args) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        },
        Err(e) => {
            eprintln!("error: {e}\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

#[derive(Debug)]
struct Args {
    config: PathBuf,
    out: Option<PathBuf>,
    seed: Option<u64>,
    ticks: Option<u64>,
}

/// `Ok(None)` means help was requested.
fn parse(argv: Vec<String>) -> Result<Option<Args>, String> {
    let mut it = argv.into_iter().peekable();
    let Some(cmd) = it.next() else {
        return Ok(None);
    };
    if cmd == "-h" || cmd == "--help" || cmd == "help" {
        return Ok(None);
    }
    if cmd != "run" {
        return Err(format!(
            "unknown command `{cmd}`; the only command is `run`"
        ));
    }

    let mut config = None;
    let mut out = None;
    let mut seed = None;
    let mut ticks = None;

    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("`{flag}` needs a value"));
        match flag.as_str() {
            "-h" | "--help" => return Ok(None),
            "--config" => config = Some(PathBuf::from(value()?)),
            "--out" => out = Some(PathBuf::from(value()?)),
            "--seed" => {
                let v = value()?;
                seed = Some(
                    v.parse()
                        .map_err(|_| format!("`--seed {v}` is not a number"))?,
                );
            }
            "--ticks" => {
                let v = value()?;
                ticks = Some(
                    v.parse()
                        .map_err(|_| format!("`--ticks {v}` is not a number"))?,
                );
            }
            other => return Err(format!("unknown option `{other}`")),
        }
    }

    Ok(Some(Args {
        config: config.ok_or("`run` needs --config <FILE>")?,
        out,
        seed,
        ticks,
    }))
}

fn execute(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    let mut cfg = Config::load(&args.config)?;
    if let Some(s) = args.seed {
        cfg.world.seed = s;
    }
    if let Some(t) = args.ticks {
        cfg.world.ticks = t;
    }
    cfg.validate()?;

    let out_dir = args
        .out
        .clone()
        .unwrap_or_else(|| PathBuf::from(&cfg.report.out_dir));

    println!(
        "universe: {}x{} base cells, {} ticks, seed {}",
        cfg.world.width, cfg.world.height, cfg.world.ticks, cfg.world.seed
    );
    println!(
        "probe: fixed {}x{} window at ({}, {}), covering {:.1}% of the world\n",
        cfg.observer.width,
        cfg.observer.height,
        cfg.observer.x,
        cfg.observer.y,
        cfg.observer.coverage(cfg.world.width, cfg.world.height) * 100.0
    );

    let exp = experiment::run_all(&cfg, |label| {
        println!("  running {label} ...");
    });

    println!();
    print!("{}", report::summary(&exp));

    let written = report::write(&exp, Path::new(&out_dir))?;
    println!(
        "\nwrote {} and {}",
        written.csv.display(),
        written.json.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn bare_invocation_prints_help() {
        assert!(parse(vec![]).unwrap().is_none());
        assert!(parse(argv("--help")).unwrap().is_none());
    }

    #[test]
    fn run_requires_a_config() {
        let e = parse(argv("run")).unwrap_err();
        assert!(e.contains("--config"));
    }

    #[test]
    fn overrides_are_parsed() {
        let a = parse(argv("run --config c.toml --seed 9 --ticks 50 --out here"))
            .unwrap()
            .unwrap();
        assert_eq!(a.config, PathBuf::from("c.toml"));
        assert_eq!(a.seed, Some(9));
        assert_eq!(a.ticks, Some(50));
        assert_eq!(a.out, Some(PathBuf::from("here")));
    }

    #[test]
    fn a_flag_without_its_value_is_an_error() {
        assert!(parse(argv("run --config")).is_err());
    }

    #[test]
    fn non_numeric_seed_is_rejected_by_name() {
        let e = parse(argv("run --config c.toml --seed later")).unwrap_err();
        assert!(e.contains("not a number"), "{e}");
    }

    #[test]
    fn unknown_command_is_rejected() {
        assert!(parse(argv("simulate --config c.toml")).is_err());
    }
}
