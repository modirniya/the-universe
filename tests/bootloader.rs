//! Theory 5 end to end, using the shipped `configs/boot.toml`.
//!
//! Two claims. That a bootloader is a real, findable thing — a pattern that
//! persists, stays localized, and travels. And that a chain seeded through the
//! pipe can end for want of *life* as well as for want of money, which gives
//! Theory 5 a limit on depth independent of the budget in `budget`.

use std::path::Path;

use universe_core::bootloader::{self, MIN_LIFETIME};
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
