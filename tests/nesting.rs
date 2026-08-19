//! Theory 2 end to end: build a chain from a config and check the claims that
//! make nesting coherent.
//!
//! The unit tests cover each piece. These use the shipped `configs/nesting.toml`
//! so that what is checked is the thing a reader actually runs.

use std::path::Path;

use universe_core::budget::Budget;
use universe_core::config::Config;
use universe_core::layer::{self, LayerSpec};
use universe_core::report;

fn cfg() -> Config {
    // Used exactly as shipped. Shortening the run would be tempting for speed
    // but changes the answer: the root budget is what the root world costs, so
    // fewer ticks means a poorer root and a shorter chain. The whole chain is
    // under ten million neighbour visits, which is a few tens of milliseconds.
    Config::load(Path::new("configs/nesting.toml")).expect("shipped config must load")
}

fn root_spec(c: &Config) -> LayerSpec {
    LayerSpec {
        width: c.world.width,
        height: c.world.height,
        ticks: c.world.ticks,
    }
}

/// The default `nest` uses: the root gets exactly what its own world costs.
fn root_budget(c: &Config) -> Budget {
    Budget::new(layer::predict_work(&root_spec(c), &c.observer, c))
}

fn chain(c: &Config) -> layer::Chain {
    layer::run_chain(c, root_budget(c), &c.nesting, |_, _| {})
}

#[test]
fn the_shipped_config_produces_a_chain_worth_looking_at() {
    // "Three interesting nested layers beat ten dead ones" is the project's
    // stated preference; this pins that the shipped config delivers some.
    let ch = chain(&cfg());
    assert!(
        ch.layers.len() >= 3,
        "expected at least three layers, got {}",
        ch.layers.len()
    );
    assert!(
        ch.productive_depth() >= 2,
        "at least two layers should still be doing something"
    );
}

#[test]
fn no_layer_outspends_its_host() {
    // The invariant that makes the whole containment story coherent.
    let ch = chain(&cfg());
    for l in &ch.layers {
        assert!(
            l.within_budget,
            "layer {} spent {} of a {} budget",
            l.layer.depth, l.work.neighbor_visits, l.layer.budget.work
        );
    }
}

#[test]
fn the_chain_is_no_deeper_than_the_closed_form_allows() {
    let ch = chain(&cfg());
    assert!(
        ch.layers.len() <= ch.predicted_max_depth,
        "built {} layers against a bound of {}",
        ch.layers.len(),
        ch.predicted_max_depth
    );
}

#[test]
fn the_total_cost_stays_under_the_geometric_bound() {
    let ch = chain(&cfg());
    assert!((ch.total_work as f64) <= ch.total_cost_bound);
}

#[test]
fn each_layer_is_poorer_and_no_larger_than_the_one_above() {
    let ch = chain(&cfg());
    for pair in ch.layers.windows(2) {
        assert!(
            pair[1].layer.budget.work < pair[0].layer.budget.work,
            "layer {} was not poorer than layer {}",
            pair[1].layer.depth,
            pair[0].layer.depth
        );
        assert!(pair[1].layer.spec.height <= pair[0].layer.spec.height);
    }
}

#[test]
fn a_chain_is_reproducible() {
    let c = cfg();
    let a = report::chain_to_csv(&chain(&c));
    let b = report::chain_to_csv(&chain(&c));
    assert_eq!(a, b, "same config and seed must give the same chain");
}

#[test]
fn a_different_seed_changes_universe_cores_but_not_their_shape() {
    // Budgets and world sizes are derived from the config, so reseeding must
    // leave the chain's structure alone while changing what happens inside it.
    let c = cfg();
    let mut other = c.clone();
    other.world.seed = c.world.seed.wrapping_add(1);

    let a = chain(&c);
    let b = chain(&other);

    assert_eq!(a.layers.len(), b.layers.len());
    for (x, y) in a.layers.iter().zip(b.layers.iter()) {
        assert_eq!(
            x.layer.spec, y.layer.spec,
            "shape is set by the budget, not the seed"
        );
        assert_eq!(x.work, y.work, "cost is set by the geometry, not the seed");
    }
    assert!(
        a.layers
            .iter()
            .zip(b.layers.iter())
            .any(|(x, y)| x.churn != y.churn),
        "the seed should still change what happens inside the layers"
    );
}

#[test]
fn the_csv_has_one_row_per_layer() {
    let ch = chain(&cfg());
    let csv = report::chain_to_csv(&ch);
    assert_eq!(
        csv.lines().count(),
        ch.layers.len() + 1,
        "header plus a row each"
    );
}

#[test]
fn the_json_is_balanced() {
    let json = report::chain_to_json(&chain(&cfg()));
    assert_eq!(
        json.chars().filter(|c| *c == '{').count(),
        json.chars().filter(|c| *c == '}').count()
    );
    assert!(json.contains("predicted_max_depth"));
}

#[test]
fn the_summary_says_what_it_does_not_show() {
    let s = report::chain_summary(&chain(&cfg()));
    assert!(
        s.contains("pipe between them is v0.3"),
        "the summary must not let mutual blindness read as a claim"
    );
}
