//! CLI entry point. Argument parsing is done by hand: the dependency list is
//! deliberately short, and this binary has exactly one subcommand.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use universe_core::bootloader::{self, Gate};
use universe_core::budget::Budget;
use universe_core::config::Config;
use universe_core::detector::{self, Inhabitant};
use universe_core::{experiment, information, layer, limits, pipe, report, sweep};

const USAGE: &str = "\
the-universe — a runnable model of a simulation-hypothesis framework

USAGE:
    the-universe run  --config <FILE> [OPTIONS]
    the-universe nest --config <FILE> [OPTIONS]
    the-universe pipe --config <FILE> [OPTIONS]
    the-universe detect --config <FILE> [OPTIONS]
    the-universe sweep --config <FILE> [OPTIONS]
    the-universe boot  --config <FILE> [OPTIONS]
    the-universe edge  --config <FILE> [OPTIONS]

COMMANDS:
    run     Run every setting of the four limits against the unconstrained
            universe and four null models, and report what each setting
            costs and how far it moves seven macro-scale observables.
            (Theory 1: limits as optimizations.)

    nest    Build a chain of universes, each running on a fraction of its
            host's budget, and report how deep it gets before it cannot
            afford another. (Theory 2: nesting and degradation.)

    pipe    Transmit a universe through a one-way serializing channel and
            report what survived, in bits: per task, per encoding, against a
            window elsewhere and against noise. (Theory 3: the horizon as a
            pipe.)

    detect  Measure a universe from inside it, with no access to its config,
            and test which limits an inhabitant can find: rules calibrated on
            half the seeds, false-positive rate and power measured on the
            other half, with a negative control. (Detection.)

    sweep   Vary the rule's constants across a grid, score what each setting
            produces, and report what share of the space is worth inhabiting.
            (Theory 6: fine-tuning.)

    boot    Build a chain in which every layer is seeded by what crossed its
            parent's horizon, and report where the chain stops -- for want of
            budget, or for want of anything alive enough to boot with.
            (Theory 5: bootloader life.)

    edge    Run the boot chain across a grid of degradation fractions and
            size floors, with and without the bootloader gate, and map why
            each chain stopped: budget, space, or sterility alone.
            (Theories 2 and 5: where poverty and sterility meet.)

OPTIONS:
    --config <FILE>   Universe definition (TOML). Required.
    --out <DIR>       Where to write the report.
                      Defaults to the config's report.out_dir.
    --seed <N>        Override world.seed. Same seed, same universe.
    --ticks <N>       Override world.ticks.
    --seeds <N>       Override world.seeds: run an ensemble of N universes,
                      the pinned seed first, and summarise each finding
                      across them. One seed is an example, not a finding.
    --budget <N>      nest only: root layer's work budget, in neighbour
                      visits. Defaults to what the root world costs.
    --steps <N>       sweep only: grid resolution per axis. Default 21.
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
    seeds: Option<usize>,
    budget: Option<u64>,
    steps: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    /// Theory 1: what the limits cost and what they changed.
    Run,
    /// Theory 2: how deep a chain of universes gets.
    Nest,
    /// Theory 3: what survives a one-way serializing channel.
    Pipe,
    /// Detection: which limits are findable from inside.
    Detect,
    /// Theory 6: how narrow the productive band of constants is.
    Sweep,
    /// Theory 5: a chain booted from inside, layer by layer.
    Boot,
    /// Theories 2 and 5: where a chain dies of poverty and where of sterility.
    Edge,
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
        "pipe" => Command::Pipe,
        "detect" => Command::Detect,
        "sweep" => Command::Sweep,
        "boot" => Command::Boot,
        "edge" => Command::Edge,
        other => {
            return Err(format!(
                "unknown command `{other}`; the commands are `run`, `nest`, `pipe`, `detect`, `sweep`, `boot` and `edge`"
            ));
        }
    };

    let mut config = None;
    let mut out = None;
    let mut seed = None;
    let mut ticks = None;
    let mut seeds = None;
    let mut budget = None;
    let mut steps = None;

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
            "--seeds" => {
                let v = value()?;
                seeds = Some(
                    v.parse()
                        .map_err(|_| format!("`--seeds {v}` is not a number"))?,
                );
            }
            "--steps" => {
                if command != Command::Sweep {
                    return Err("`--steps` applies to `sweep`, not other commands".into());
                }
                let v = value()?;
                steps = Some(
                    v.parse::<usize>()
                        .map_err(|_| format!("`--steps {v}` is not a number"))?,
                );
            }
            "--budget" => {
                if !matches!(command, Command::Nest | Command::Boot | Command::Edge) {
                    return Err("`--budget` applies to `nest` and `boot`, not `run`".into());
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
        seeds,
        budget,
        steps,
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
    if let Some(n) = args.seeds {
        cfg.world.seeds = n;
    }
    cfg.validate()?;

    let out_dir = args
        .out
        .clone()
        .unwrap_or_else(|| PathBuf::from(&cfg.report.out_dir));

    match args.command {
        Command::Run => execute_run(&cfg, &out_dir),
        Command::Nest => execute_nest(&cfg, &out_dir, args.budget),
        Command::Pipe => execute_pipe(&cfg, &out_dir),
        Command::Detect => execute_detect(&cfg, &out_dir),
        Command::Sweep => execute_sweep(&cfg, &out_dir, args.steps),
        Command::Boot => execute_boot(&cfg, &out_dir, args.budget),
        Command::Edge => execute_edge(&cfg, &out_dir, args.budget),
    }
}

/// Every ensemble member after the pinned one, with the pinned result put back
/// in front. The pinned seed was already run on its own, with its progress
/// printed, so it is not run twice.
fn with_rest<T: Send>(cfg: &Config, pinned: T, f: impl Fn(&Config) -> T + Sync) -> Vec<(u64, T)> {
    let mut all = vec![(cfg.world.seed, pinned)];
    if cfg.world.seeds > 1 {
        let mut rest = cfg.clone();
        rest.world.seed = cfg.world.seed.wrapping_add(cfg.world.seed_stride);
        rest.world.seeds = cfg.world.seeds - 1;
        println!("\nrunning {} more seeds ...", rest.world.seeds);
        all.extend(experiment::per_seed(&rest, f));
    }
    all
}

fn print_ensemble(summary: String, written: std::io::Result<PathBuf>) -> std::io::Result<()> {
    println!();
    print!("{summary}");
    println!("\nwrote {}", written?.display());
    Ok(())
}

fn execute_edge(
    cfg: &Config,
    out_dir: &Path,
    budget_override: Option<u64>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root_spec = layer::LayerSpec {
        width: cfg.world.width,
        height: cfg.world.height,
        ticks: cfg.world.ticks,
    };
    let root_budget = Budget::new(
        budget_override.unwrap_or_else(|| layer::predict_work(&root_spec, &cfg.observer, cfg)),
    );
    println!(
        "root universe: {}x{} base cells, {} ticks, seed {}; viable_work held at {}\n",
        cfg.world.width, cfg.world.height, cfg.world.ticks, cfg.world.seed, cfg.nesting.viable_work
    );

    let cells = bootloader::map_endings(cfg, root_budget);
    print!("{}", report::edge_summary(&cells));

    let runs = with_rest(cfg, cells, |c| bootloader::map_endings(c, root_budget));
    if runs.len() > 1 {
        println!();
        print!("{}", report::edge_ensemble_summary(&runs));
    }
    println!("\nwrote {}", report::write_edge(&runs, out_dir)?.display());
    Ok(())
}

fn execute_boot(
    cfg: &Config,
    out_dir: &Path,
    budget_override: Option<u64>,
) -> Result<(), Box<dyn std::error::Error>> {
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
        "horizon {}x{} at ({}, {}), logging threshold {:.2}\n",
        cfg.horizon.width, cfg.horizon.height, cfg.horizon.x, cfg.horizon.y, cfg.horizon.threshold
    );

    let chain = bootloader::run_boot_chain(cfg, root_budget, &cfg.nesting, |depth, spec| {
        println!("  layer {depth}: {}x{} ...", spec.width, spec.height);
    });

    println!();
    print!("{}", report::boot_summary(&chain));

    let written = report::write_boot(&chain, out_dir)?;
    println!(
        "\nwrote {} and {}",
        written.csv.display(),
        written.json.display()
    );

    // The ablation: the same chain with the bootloader gate removed.
    let ungated =
        bootloader::run_boot_chain_with(cfg, root_budget, &cfg.nesting, Gate::Open, |_, _| {});
    println!();
    print!("{}", report::gate_ablation(&chain, &ungated));
    println!(
        "wrote {}",
        report::write_gate(&chain, &ungated, out_dir)?.display()
    );

    if cfg.world.seeds > 1 {
        let runs = with_rest(cfg, (chain, ungated), |c| {
            let run =
                |gate| bootloader::run_boot_chain_with(c, root_budget, &c.nesting, gate, |_, _| {});
            (run(Gate::Bootloader), run(Gate::Open))
        });
        let gated: Vec<(u64, bootloader::BootChain)> =
            runs.iter().map(|(s, (g, _))| (*s, g.clone())).collect();
        print_ensemble(
            report::boot_ensemble_summary(&gated),
            report::write_boot_ensemble(&gated, out_dir),
        )?;
        println!();
        print!("{}", report::gate_ensemble_summary(&runs));
        println!(
            "wrote {}",
            report::write_gate_ensemble(&runs, out_dir)?.display()
        );
    }
    Ok(())
}

fn execute_sweep(
    cfg: &Config,
    out_dir: &Path,
    steps: Option<usize>,
) -> Result<(), Box<dyn std::error::Error>> {
    let steps = steps.unwrap_or(21).max(2);
    let (min, max) = (0.05, 0.65);

    println!(
        "each universe: {}x{} base cells, {} ticks, seed {}",
        cfg.world.width, cfg.world.height, cfg.world.ticks, cfg.world.seed
    );
    println!(
        "sweeping {} settings of the rule's constants\n",
        steps * steps
    );

    let sw = sweep::run_sweep(cfg, steps, min, max, |done, total| {
        if done % 4 == 0 || done == total {
            println!("  row {done}/{total}");
        }
    });

    println!();
    print!("{}", report::sweep_summary(&sw));

    let written = report::write_sweep(&sw, out_dir)?;
    println!(
        "\nwrote {} and {}",
        written.csv.display(),
        written.json.display()
    );

    if cfg.world.seeds > 1 {
        let runs = with_rest(cfg, sw, |c| sweep::run_sweep(c, steps, min, max, |_, _| {}));
        print_ensemble(
            report::sweep_ensemble_summary(&runs),
            report::write_sweep_ensemble(&runs, out_dir),
        )?;
    }

    // Does the answer survive a change of criterion, and of which constants
    // are swept?
    println!("\n== sensitivity ==\n");
    let sens = sweep::run_sensitivity(cfg, steps, min, max);
    print!("{}", report::sensitivity_summary(&sens));
    let written = report::write_sensitivity(&sens, out_dir)?;
    println!(
        "\nwrote {} and {}",
        written.csv.display(),
        written.json.display()
    );
    if cfg.world.seeds > 1 {
        let runs = with_rest(cfg, sens, |c| sweep::run_sensitivity(c, steps, min, max));
        print_ensemble(
            report::sensitivity_ensemble_summary(&runs),
            report::write_sensitivity_ensemble(&runs, out_dir),
        )?;
    }
    Ok(())
}

fn execute_detect(cfg: &Config, out_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // Straddling the observed region and the coarse ground beyond it, so that
    // there is coarse ground in reach at all. Whether the inhabitant can still
    // see it once it looks is the question.
    let who = Inhabitant {
        x: cfg.observer.x + cfg.observer.width / 2,
        y: cfg.observer.y + cfg.observer.height / 2,
        width: cfg.observer.width,
        height: cfg.observer.height,
    };

    println!(
        "universe: {}x{} base cells, {} ticks, {} seeds from {}",
        cfg.world.width, cfg.world.height, cfg.world.ticks, cfg.world.seeds, cfg.world.seed
    );
    println!(
        "inhabitant: {}x{} region at ({}, {}); each seed measures {} universes under two gazes\n",
        who.width,
        who.height,
        who.x,
        who.y,
        2 * (2 + detector::LIMITS.len() + 1)
    );

    let survey = detector::survey(cfg, &who, |seed| println!("  measuring seed {seed} ..."));
    println!();
    print!("{}", report::detect::summary(&survey));

    let written = report::detect::write(&survey, out_dir)?;
    println!(
        "\nwrote {}, {} and {}",
        written.csv.display(),
        written.json.display(),
        out_dir.join("evidence.csv").display()
    );
    Ok(())
}

fn execute_pipe(cfg: &Config, out_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "child universe: {}x{} base cells, {} ticks, seed {}",
        cfg.world.width, cfg.world.height, cfg.world.ticks, cfg.world.seed
    );
    println!("transmitting one message per tick through the horizon\n");

    let relay = pipe::run_relay(cfg, &cfg.horizon);

    print!("{}", report::pipe_summary(&relay));

    let written = report::write_pipe(&relay, out_dir)?;
    println!(
        "\nwrote {} and {}",
        written.csv.display(),
        written.json.display()
    );

    if cfg.world.seeds > 1 {
        let runs = with_rest(cfg, relay, |c| pipe::run_relay(c, &c.horizon));
        print_ensemble(
            report::pipe_ensemble_summary(&runs),
            report::write_pipe_ensemble(&runs, out_dir),
        )?;
    }

    // What crossed, in bits: per task, per encoding, against the controls.
    println!("\n== information ==\n");
    let analysis = information::analyse(cfg, &cfg.horizon);
    print!("{}", report::information::summary(&analysis));
    let written = report::information::write(&analysis, out_dir)?;
    println!(
        "\nwrote {} and {}",
        written.csv.display(),
        written.json.display()
    );
    if cfg.world.seeds > 1 {
        let runs = with_rest(cfg, analysis, |c| information::analyse(c, &c.horizon));
        print_ensemble(
            report::information::ensemble_summary(&runs),
            report::information::write_ensemble(&runs, out_dir),
        )?;
    }
    Ok(())
}

fn execute_run(cfg: &Config, out_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "universe: {}x{} base cells, {} ticks, seed {}; unobserved ground closes under '{}'",
        cfg.world.width,
        cfg.world.height,
        cfg.world.ticks,
        cfg.world.seed,
        cfg.params.coarse_rule.label()
    );
    println!(
        "probe: fixed {}x{} window at ({}, {}), covering {:.1}% of the world",
        cfg.observer.width,
        cfg.observer.height,
        cfg.observer.x,
        cfg.observer.y,
        cfg.observer.coverage(cfg.world.width, cfg.world.height) * 100.0
    );
    println!(
        "design: all 16 settings of the four limits, 4 nulls, and the lazy settings under the\n\
         other two closures -- 24 universes per seed\n"
    );

    if cfg.world.seeds == 1 {
        let f = limits::run_factorial(cfg, |label| println!("  running {label} ..."));
        println!();
        print!("{}", report::limits::summary(&f));
        let written = report::limits::write(&f, out_dir)?;
        println!(
            "\nwrote {}, {} and {}",
            written.csv.display(),
            written.json.display(),
            out_dir.join("divergence_trace.csv").display()
        );
        return Ok(());
    }

    let ens = limits::run_ensemble(cfg, |seed| {
        println!("  running every setting at seed {seed} ...");
    });
    let pinned = &ens.runs[0].1;

    println!("\npinned seed {}:\n", ens.runs[0].0);
    print!("{}", report::limits::summary(pinned));
    println!();
    print!("{}", report::limits::ensemble_summary(&ens));

    let written = report::limits::write(pinned, out_dir)?;
    let ensemble = report::limits::write_ensemble(&ens, out_dir)?;
    println!(
        "\nwrote {}, {}, {} and {}",
        written.csv.display(),
        written.json.display(),
        ensemble.csv.display(),
        ensemble.json.display()
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

    if cfg.world.seeds > 1 {
        let runs = with_rest(cfg, chain, |c| {
            layer::run_chain(c, root_budget, &c.nesting, |_, _| {})
        });
        print_ensemble(
            report::chain_ensemble_summary(&runs),
            report::write_chain_ensemble(&runs, out_dir),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn seeds_is_accepted_by_every_command() {
        for cmd in ["run", "nest", "pipe", "detect", "sweep", "boot"] {
            let a = parse(argv(&format!("{cmd} --config c.toml --seeds 5")))
                .unwrap()
                .unwrap();
            assert_eq!(a.seeds, Some(5));
        }
        assert!(parse(argv("run --config c.toml --seeds many")).is_err());
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
    fn boot_is_a_command() {
        let a = parse(argv("boot --config c.toml")).unwrap().unwrap();
        assert_eq!(a.command, Command::Boot);
    }

    #[test]
    fn sweep_is_a_command() {
        let a = parse(argv("sweep --config c.toml --steps 9"))
            .unwrap()
            .unwrap();
        assert_eq!(a.command, Command::Sweep);
        assert_eq!(a.steps, Some(9));
    }

    #[test]
    fn steps_belongs_to_sweep_only() {
        let e = parse(argv("run --config c.toml --steps 9")).unwrap_err();
        assert!(e.contains("applies to `sweep`"), "{e}");
    }

    #[test]
    fn detect_is_a_command() {
        let a = parse(argv("detect --config c.toml")).unwrap().unwrap();
        assert_eq!(a.command, Command::Detect);
    }

    #[test]
    fn pipe_is_a_command() {
        let a = parse(argv("pipe --config c.toml")).unwrap().unwrap();
        assert_eq!(a.command, Command::Pipe);
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
        assert!(parse(argv("boot --config c.toml --budget 5000")).is_ok());
    }

    #[test]
    fn the_error_names_the_command_you_typed() {
        let e = parse(argv("nest")).unwrap_err();
        assert!(e.contains("`nest` needs --config"), "{e}");
    }
}
