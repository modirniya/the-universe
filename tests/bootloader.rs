//! Theory 5 end to end, using the shipped `configs/boot.toml`.
//!
//! Two claims. That a bootloader is a real, findable thing — a pattern that
//! persists, stays localized, and travels. And that a chain seeded through the
//! pipe can end for want of *life* as well as for want of money, which gives
//! Theory 5 a limit on depth independent of the budget in `budget`.

use std::path::Path;

use universe_core::bootloader::{self, Ending, Gate, MIN_LIFETIME};
use universe_core::budget::Budget;
use universe_core::config::Config;
use universe_core::constraints::Constraints;
use universe_core::layer::{self, LayerSpec};
use universe_core::report;

fn cfg() -> Config {
    Config::load(Path::new("configs/boot.toml")).expect("shipped config must load")
}

fn root_budget(c: &Config) -> Budget {
    let root = LayerSpec {
        width: c.world.width,
        height: c.world.height,
        ticks: c.world.ticks,
    };
    Budget::new(layer::predict_work(&root, &c.observer, c))
}

fn chain(c: &Config) -> bootloader::BootChain {
    bootloader::run_boot_chain(c, root_budget(c), &c.nesting, |_, _| {})
}

#[test]
fn the_root_universe_produces_bootloaders() {
    let c = cfg();
    let s = bootloader::survey(&c, &c.rules, Constraints::ALL_ON);
    assert!(
        s.can_boot(),
        "a Conway soup this size should transport something"
    );
    assert!(s.transport > 0.0);
    assert!(s.longest_lifetime >= MIN_LIFETIME);
}

#[test]
fn a_chain_boots_more_than_one_layer() {
    let ch = chain(&cfg());
    assert!(ch.depth() >= 2, "reached only depth {}", ch.depth());
    assert!(ch.layers[0].booted_child, "the root should seed a child");
}

#[test]
fn every_child_is_seeded_by_its_parent() {
    // The loop the framework describes: no layer below the first uses the
    // creator's seed, and no two layers share one.
    let c = cfg();
    let ch = chain(&c);
    assert_eq!(ch.layers[0].seed, c.world.seed);
    for l in &ch.layers[1..] {
        assert_ne!(
            l.seed, c.world.seed,
            "layer {} reused the root seed",
            l.depth
        );
    }
    let mut seeds: Vec<u64> = ch.layers.iter().map(|l| l.seed).collect();
    seeds.sort_unstable();
    seeds.dedup();
    assert_eq!(seeds.len(), ch.layers.len(), "seeds must be distinct");
}

#[test]
fn poorer_layers_produce_less_life() {
    // Degradation is not only a budget story. A smaller universe transports
    // less, and the chain thins out as it descends.
    let ch = chain(&cfg());
    let first = &ch.layers[0].survey;
    let last = &ch.layers[ch.depth() - 1].survey;
    assert!(
        last.bootloaders <= first.bootloaders,
        "expected the deepest layer to boot no more than the first: {} vs {}",
        last.bootloaders,
        first.bootloaders
    );
}

#[test]
fn a_chain_can_die_of_sterility_rather_than_poverty() {
    // The finding that makes Theory 5 a depth limit in its own right. Given a
    // permissive floor on size and budget, the chain runs until a layer is too
    // small to produce anything that travels -- and stops there, with money
    // still in hand.
    let mut c = cfg();
    c.nesting.viable_edge = 4;
    c.nesting.viable_work = 2_000;

    let ch = bootloader::run_boot_chain(&c, root_budget(&c), &c.nesting, |_, _| {});
    let last = &ch.layers[ch.depth() - 1];
    assert!(
        !last.survey.can_boot(),
        "the deepest layer should be sterile"
    );
    assert!(
        ch.ended_because.contains("no bootloader"),
        "ended because: {}",
        ch.ended_because
    );
    assert!(
        c.nesting.child_of(last.budget).is_some(),
        "and it should still have been able to afford another layer"
    );
}

#[test]
fn a_sterile_layer_seeds_nothing() {
    let mut c = cfg();
    c.nesting.viable_edge = 4;
    c.nesting.viable_work = 2_000;
    let ch = bootloader::run_boot_chain(&c, root_budget(&c), &c.nesting, |_, _| {});
    let last = &ch.layers[ch.depth() - 1];
    assert!(!last.booted_child);
}

#[test]
fn transport_is_never_negative_zero() {
    // Rust folds float sums from -0.0, so a universe that transported nothing
    // reports "-0.0" unless it is normalised.
    let mut c = cfg();
    c.nesting.viable_edge = 4;
    c.nesting.viable_work = 2_000;
    let ch = bootloader::run_boot_chain(&c, root_budget(&c), &c.nesting, |_, _| {});
    for l in &ch.layers {
        assert!(l.survey.transport.is_sign_positive(), "layer {}", l.depth);
    }
}

#[test]
fn a_chain_is_reproducible() {
    let c = cfg();
    assert_eq!(
        report::boot_to_csv(&chain(&c)),
        report::boot_to_csv(&chain(&c))
    );
}

#[test]
fn the_csv_has_one_row_per_layer() {
    let c = cfg();
    let ch = chain(&c);
    assert_eq!(report::boot_to_csv(&ch).lines().count(), ch.depth() + 1);
}

#[test]
fn the_summary_calls_it_a_precondition_not_an_achievement() {
    let text = report::boot_summary(&chain(&cfg()));
    assert!(
        text.contains("precondition for booting"),
        "the report must not claim this builds a computer"
    );
    assert!(text.contains("nothing in this model builds a computer"));
}

// ---------------------------------------------------------------------------
// The ablation: what the bootloader gate actually does
// ---------------------------------------------------------------------------

fn permissive() -> Config {
    Config::load(Path::new("configs/boot-permissive.toml")).expect("shipped config must load")
}

fn gated_and_open(c: &Config) -> (bootloader::BootChain, bootloader::BootChain) {
    let run =
        |gate| bootloader::run_boot_chain_with(c, root_budget(c), &c.nesting, gate, |_, _| {});
    (run(Gate::Bootloader), run(Gate::Open))
}

#[test]
fn under_the_shipped_floors_the_gate_never_fires() {
    // Every layer has a bootloader before the spatial floor ends the chain, so
    // removing the gate changes nothing at all.
    let (g, u) = gated_and_open(&cfg());
    assert!(g.same_layers(&u));
    assert!(g.layers.iter().all(|l| l.survey.can_boot()));
}

#[test]
fn bootloaders_decide_whether_a_child_exists_never_what_it_is() {
    // The child's seed is hashed from what crossed the horizon. An ungated
    // chain must therefore rebuild every layer the gated one built, exactly.
    let mut c = permissive();
    c.world.seed = 1042;
    let (g, u) = gated_and_open(&c);
    assert!(
        g.is_prefix_of(&u),
        "the gate must only ever shorten a chain"
    );
    assert!(
        u.depth() > g.depth(),
        "at seed 1042 the gate is what stops the chain"
    );
    let last = g.layers.last().unwrap();
    assert!(
        !last.survey.can_boot(),
        "the gated chain stopped on a sterile layer"
    );
}

#[test]
fn the_ablation_report_says_when_the_gate_fired_harmlessly() {
    // At seed 42 under permissive floors the last layer is sterile, but no
    // smaller world is viable, so both chains stop at the same place.
    let (g, u) = gated_and_open(&permissive());
    assert!(g.same_layers(&u));
    let text = report::gate_ablation(&g, &u);
    assert!(text.contains("would have stopped there anyway"), "{text}");
    assert!(!text.contains("never\nfired"));
}

// ---------------------------------------------------------------------------
// Where a chain dies. The whole map is pinned in CI from a release build; these
// pin one cell of each kind at seed 42, which is affordable in a debug build.
// ---------------------------------------------------------------------------

fn edge_cfg() -> Config {
    Config::load(Path::new("configs/edge.toml")).expect("shipped config must load")
}

#[test]
fn poor_children_die_of_poverty() {
    let c = edge_cfg();
    let cell = bootloader::ending_at(&c, root_budget(&c), 0.10, 2);
    assert_eq!(cell.ending, Ending::Budget, "{cell:?}");
}

#[test]
fn a_high_size_floor_ends_chains_on_space() {
    let c = edge_cfg();
    let cell = bootloader::ending_at(&c, root_budget(&c), 0.10, 24);
    assert_eq!(cell.ending, Ending::Space, "{cell:?}");
}

#[test]
fn rich_children_with_a_low_floor_die_of_sterility_alone() {
    let c = edge_cfg();
    let cell = bootloader::ending_at(&c, root_budget(&c), 0.50, 2);
    assert_eq!(cell.ending, Ending::Sterile, "{cell:?}");
    assert!(cell.ungated_depth > cell.gated_depth);
}

#[test]
fn a_sterile_label_is_not_credited_when_another_limit_also_binds() {
    // The chain's own label says sterility; the ablation says the chain would
    // have stopped at the same depth without the gate.
    let c = edge_cfg();
    let cell = bootloader::ending_at(&c, root_budget(&c), 0.20, 2);
    assert_eq!(cell.ending, Ending::Tied, "{cell:?}");
    assert_eq!(cell.ungated_depth, cell.gated_depth);
}
