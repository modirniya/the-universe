//! CLI entry point. Argument parsing is done by hand: the dependency list is
//! deliberately short, and this binary has exactly one subcommand.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use the_universe::budget::Budget;
use the_universe::config::Config;
use the_universe::{experiment, layer, report};

const USAGE: &str = "\
the-universe — a runnable model of a simulation-hypothesis framework

USAGE:
    the-universe run  --config <FILE> [OPTIONS]
    the-universe nest --config <FILE> [OPTIONS]

COMMANDS:
    run     Compare an unconstrained universe against one with each limit in
            force, and report what the limits cost and what they changed.
            (Theory 1: limits as optimizations.)

    nest    Build a chain of universes, each running on a fraction of its
            host's budget, and report how deep it gets before it cannot
            afford another. (Theory 2: nesting and degradation.)

OPTIONS:
    --config <FILE>   Universe definition (TOML). Required.
    --out <DIR>       Where to write the report.
                      Defaults to the config's report.out_dir.
    --seed <N>        Override world.seed. Same seed, same universe.
    --ticks <N>       Override world.ticks.
    --budget <N>      nest only: root layer's work budget, in neighbour
                      visits. Defaults to what the root world costs.
    -h, --help        Print this.
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
    command: Command,
    config: PathBuf,
    out: Option<PathBuf>,
    seed: Option<u64>,
    ticks: Option<u64>,
    budget: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    /// Theory 1: what the limits cost and what they changed.
    Run,
    /// Theory 2: how deep a chain of universes gets.
    Nest,
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
    let command = match cmd.as_str() {
        "run" => Command::Run,
        "nest" => Command::Nest,
        other => {
            return Err(format!(
                "unknown command `{other}`; the commands are `run` and `nest`"
            ));
        }
    };

    let mut config = None;
    let mut out = None;
    let mut seed = None;
    let mut ticks = None;
    let mut budget = None;

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
            "--budget" => {
                if command != Command::Nest {
                    return Err("`--budget` applies to `nest`, not `run`".into());
                }
                let v = value()?;
                budget = Some(
                    v.parse()
                        .map_err(|_| format!("`--budget {v}` is not a number"))?,
                );
            }
            other => return Err(format!("unknown option `{other}`")),
        }
    }

    Ok(Some(Args {
        command,
        config: config.ok_or_else(|| {
            let name = if command == Command::Nest {
                "nest"
            } else {
                "run"
            };
            format!("`{name}` needs --config <FILE>")
        })?,
        out,
        seed,
        ticks,
        budget,
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

    match args.command {
        Command::Run => execute_run(&cfg, &out_dir),
        Command::Nest => execute_nest(&cfg, &out_dir, args.budget),
    }
}

fn execute_run(cfg: &Config, out_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
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

    let exp = experiment::run_all(cfg, |label| {
        println!("  running {label} ...");
    });

    println!();
    print!("{}", report::summary(&exp));

    let written = report::write(&exp, out_dir)?;
    println!(
        "\nwrote {} and {}",
        written.csv.display(),
        written.json.display()
    );
    Ok(())
}

fn execute_nest(
    cfg: &Config,
    out_dir: &Path,
    budget_override: Option<u64>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Without an explicit budget the root layer is given exactly what its own
    // world costs. That is the most honest default: the root is as rich as the
    // config says, and every layer below is poorer by the degradation rule
    // rather than by a number someone picked.
    let root_spec = layer::LayerSpec {
        width: cfg.world.width,
        height: cfg.world.height,
        ticks: cfg.world.ticks,
    };
    let root_budget = Budget::new(
        budget_override.unwrap_or_else(|| layer::predict_work(&root_spec, &cfg.observer, cfg)),
    );

    println!(
        "root universe: {}x{} base cells, {} ticks, seed {}",
        cfg.world.width, cfg.world.height, cfg.world.ticks, cfg.world.seed
    );
    println!(
        "degradation: each child gets {:.0}% of its host, viable above {} work units and {} cells per edge\n",
        cfg.nesting.fraction * 100.0,
        cfg.nesting.viable_work,
        cfg.nesting.viable_edge
    );

    let chain = layer::run_chain(cfg, root_budget, &cfg.nesting, |depth, spec| {
        println!("  layer {depth}: {}x{} ...", spec.width, spec.height);
    });

    println!();
    print!("{}", report::chain_summary(&chain));

    let written = report::write_chain(&chain, out_dir)?;
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

    #[test]
    fn nest_is_a_command() {
        let a = parse(argv("nest --config c.toml")).unwrap().unwrap();
        assert_eq!(a.command, Command::Nest);
    }

    #[test]
    fn budget_belongs_to_nest_only() {
        let a = parse(argv("nest --config c.toml --budget 5000"))
            .unwrap()
            .unwrap();
        assert_eq!(a.budget, Some(5000));
        let e = parse(argv("run --config c.toml --budget 5000")).unwrap_err();
        assert!(e.contains("applies to `nest`"), "{e}");
    }

    #[test]
    fn the_error_names_the_command_you_typed() {
        let e = parse(argv("nest")).unwrap_err();
        assert!(e.contains("`nest` needs --config"), "{e}");
    }
}
