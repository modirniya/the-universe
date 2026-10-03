//! Bootloader controls: the tracker's false-positive floor and the area
//! control for the chain.

use super::{json_f64, seed_list};
use crate::bootloader::{AreaControl, ShuffleControl};
use crate::stats::Summary;
use std::fmt::Write as _;
use std::io;
use std::path::{Path, PathBuf};

/// The shuffle control for one seed.
pub fn shuffle_summary(ctl: &ShuffleControl) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "tracker control: the root universe's frames in their real order and in a shuffled one"
    );
    let _ = writeln!(
        s,
        "{:<10} {:>8} {:>12} {:>14} {:>12}",
        "frames", "tracks", "bootloaders", "per 1000 cells", "transport"
    );
    for (label, sv) in [("real", &ctl.real), ("shuffled", &ctl.shuffled)] {
        let _ = writeln!(
            s,
            "{:<10} {:>8} {:>12} {:>14.3} {:>12.1}",
            label,
            sv.tracks,
            sv.bootloaders,
            sv.per_kilocell(),
            sv.transport
        );
    }
    let _ = writeln!(
        s,
        "nothing travels in shuffled frames, so {} bootloaders there are what the detector makes\n\
         out of debris lying near other debris; the real count exceeds that floor by {:.3} per\n\
         thousand cells.",
        ctl.shuffled.bootloaders,
        ctl.excess_density()
    );
    s
}

/// The shuffle control across seeds.
pub fn shuffle_ensemble_summary(runs: &[(u64, ShuffleControl)]) -> String {
    let mut s = String::new();
    let seeds: Vec<u64> = runs.iter().map(|(s, _)| *s).collect();
    let _ = writeln!(
        s,
        "tracker control across {} seeds ({}), bootloaders per thousand cells:",
        runs.len(),
        seed_list(&seeds)
    );
    let real = Summary::of(runs.iter().map(|(_, c)| c.real.per_kilocell()));
    let shuffled = Summary::of(runs.iter().map(|(_, c)| c.shuffled.per_kilocell()));
    let excess = Summary::of(runs.iter().map(|(_, c)| c.excess_density()));
    let ratio = Summary::of(runs.iter().map(|(_, c)| {
        if c.real.per_kilocell() > 0.0 {
            c.shuffled.per_kilocell() / c.real.per_kilocell()
        } else {
            f64::NAN
        }
    }));
    let _ = writeln!(s, "  real      {}", super::limits::summary_cell(&real, 3));
    let _ = writeln!(
        s,
        "  shuffled  {}",
        super::limits::summary_cell(&shuffled, 3)
    );
    let _ = writeln!(s, "  excess    {}", super::limits::summary_cell(&excess, 3));
    let _ = writeln!(
        s,
        "  shuffled as a share of real: {}",
        super::limits::summary_cell(&ratio, 2)
    );
    let above = runs
        .iter()
        .filter(|(_, c)| c.excess_density() > 0.0)
        .count();
    let _ = writeln!(
        s,
        "  real exceeded shuffled in {}; the share above is how much of any bootloader count\n\
         in this README could be the detector rather than the universe.",
        super::limits::share_cell(above, runs.len())
    );
    s
}

/// The area control for one chain.
pub fn area_summary(controls: &[AreaControl]) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "area control: each layer beside standalone universes of its size, bootloaders per thousand cells"
    );
    let _ = writeln!(
        s,
        "{:>5} {:>6} {:>12} {:>16} {:>16} {:>9}",
        "depth", "edge", "layer", "same size, root seed", "same size, own seed", "identical"
    );
    for c in controls {
        let _ = writeln!(
            s,
            "{:>5} {:>6} {:>12.3} {:>20.3} {:>19.3} {:>9}",
            c.depth,
            c.edge,
            c.layer_per_kilocell,
            c.root_seed_per_kilocell,
            c.own_seed_per_kilocell,
            c.identical()
        );
    }
    let _ = writeln!(
        s,
        "a layer at its own seed is a standalone universe of its size, by construction. the\n\
         root-seed column asks what the size alone does to the density of bootloaders; a\n\
         difference between it and the layer is the seed, not nesting."
    );
    s
}

/// The area control across seeds: the density by depth, and whether it falls.
pub fn area_ensemble_summary(runs: &[(u64, Vec<AreaControl>)]) -> String {
    let mut s = String::new();
    let seeds: Vec<u64> = runs.iter().map(|(s, _)| *s).collect();
    let _ = writeln!(
        s,
        "area control across {} seeds ({}), bootloaders per thousand cells by depth:",
        runs.len(),
        seed_list(&seeds)
    );
    let max_depth = runs.iter().map(|(_, v)| v.len()).max().unwrap_or(0);
    let _ = writeln!(
        s,
        "{:>5} {:>6} {:>30} {:>30}",
        "depth", "seeds", "layer: mean ± sd [95% ci]", "same size, root seed"
    );
    for d in 1..=max_depth {
        let at = |f: &dyn Fn(&AreaControl) -> f64| {
            Summary::of(
                runs.iter()
                    .filter_map(|(_, v)| v.iter().find(|c| c.depth == d).map(f)),
            )
        };
        let layer = at(&|c| c.layer_per_kilocell);
        let root = at(&|c| c.root_seed_per_kilocell);
        let _ = writeln!(
            s,
            "{d:>5} {:>6} {:>30} {:>30}",
            layer.n,
            super::limits::summary_cell(&layer, 3),
            super::limits::summary_cell(&root, 3)
        );
    }
    let falls = runs
        .iter()
        .filter(|(_, v)| {
            v.len() >= 2 && v.last().unwrap().layer_per_kilocell < v[0].layer_per_kilocell
        })
        .count();
    let counts_fall = runs.len();
    let _ = writeln!(
        s,
        "the deepest layer's density is below the root's in {} seeds. a count of bootloaders\n\
         falls down every chain because the world shrinks; whether the density does is this row.",
        super::limits::share_cell(falls, counts_fall)
    );
    s
}

const SHUFFLE_COLUMNS: &[&str] = &[
    "seed",
    "cells",
    "real_tracks",
    "real_bootloaders",
    "real_per_kilocell",
    "shuffled_tracks",
    "shuffled_bootloaders",
    "shuffled_per_kilocell",
];

pub fn shuffle_to_csv(runs: &[(u64, ShuffleControl)]) -> String {
    let mut s = SHUFFLE_COLUMNS.join(",");
    s.push('\n');
    for (seed, c) in runs {
        let _ = writeln!(
            s,
            "{seed},{},{},{},{:.6},{},{},{:.6}",
            c.real.cells,
            c.real.tracks,
            c.real.bootloaders,
            c.real.per_kilocell(),
            c.shuffled.tracks,
            c.shuffled.bootloaders,
            c.shuffled.per_kilocell()
        );
    }
    s
}

const AREA_COLUMNS: &[&str] = &[
    "seed",
    "depth",
    "edge",
    "layer_per_kilocell",
    "root_seed_per_kilocell",
    "own_seed_per_kilocell",
    "identical",
];

pub fn area_to_csv(runs: &[(u64, Vec<AreaControl>)]) -> String {
    let mut s = AREA_COLUMNS.join(",");
    s.push('\n');
    for (seed, v) in runs {
        for c in v {
            let _ = writeln!(
                s,
                "{seed},{},{},{:.6},{:.6},{:.6},{}",
                c.depth,
                c.edge,
                c.layer_per_kilocell,
                c.root_seed_per_kilocell,
                c.own_seed_per_kilocell,
                c.identical()
            );
        }
    }
    s
}

pub fn write_shuffle(runs: &[(u64, ShuffleControl)], out_dir: &Path) -> io::Result<PathBuf> {
    std::fs::create_dir_all(out_dir)?;
    let path = out_dir.join("shuffle_control.csv");
    std::fs::write(&path, shuffle_to_csv(runs))?;
    Ok(path)
}

pub fn write_area(runs: &[(u64, Vec<AreaControl>)], out_dir: &Path) -> io::Result<PathBuf> {
    std::fs::create_dir_all(out_dir)?;
    let path = out_dir.join("area_control.csv");
    std::fs::write(&path, area_to_csv(runs))?;
    Ok(path)
}

/// The controls of the pinned seed as one JSON object.
pub fn controls_json(shuffle: &ShuffleControl, area: &[AreaControl]) -> String {
    let rows: Vec<String> = area
        .iter()
        .map(|c| {
            format!(
                "    {{\"depth\": {}, \"edge\": {}, \"layer_per_kilocell\": {}, \"root_seed_per_kilocell\": {}, \"own_seed_per_kilocell\": {}}}",
                c.depth,
                c.edge,
                json_f64(c.layer_per_kilocell),
                json_f64(c.root_seed_per_kilocell),
                json_f64(c.own_seed_per_kilocell)
            )
        })
        .collect();
    format!(
        "{{\n  \"shuffle\": {{\"cells\": {}, \"real_bootloaders\": {}, \"shuffled_bootloaders\": {}, \"real_per_kilocell\": {}, \"shuffled_per_kilocell\": {}}},\n  \"area\": [\n{}\n  ]\n}}\n",
        shuffle.real.cells,
        shuffle.real.bootloaders,
        shuffle.shuffled.bootloaders,
        json_f64(shuffle.real.per_kilocell()),
        json_f64(shuffle.shuffled.per_kilocell()),
        rows.join(",\n")
    )
}

pub fn write_controls(
    shuffle: &ShuffleControl,
    area: &[AreaControl],
    out_dir: &Path,
) -> io::Result<PathBuf> {
    std::fs::create_dir_all(out_dir)?;
    let path = out_dir.join("controls.json");
    std::fs::write(&path, controls_json(shuffle, area))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootloader::BootSurvey;

    fn survey(boots: usize) -> BootSurvey {
        BootSurvey {
            tracks: 10,
            bootloaders: boots,
            transport: boots as f64 * 5.0,
            longest_lifetime: 20,
            cells: 1000,
        }
    }

    #[test]
    fn the_shuffle_report_states_the_floor() {
        let ctl = ShuffleControl {
            real: survey(8),
            shuffled: survey(2),
        };
        let s = shuffle_summary(&ctl);
        assert!(s.contains("exceeds that floor by 6.000"));
        let runs = vec![(1u64, ctl.clone()), (2, ctl)];
        let e = shuffle_ensemble_summary(&runs);
        assert!(e.contains("2/2"));
        assert_eq!(shuffle_to_csv(&runs).lines().count(), 3);
    }

    #[test]
    fn the_area_report_counts_falling_densities() {
        let a = AreaControl {
            depth: 1,
            edge: 128,
            layer_per_kilocell: 8.0,
            root_seed_per_kilocell: 8.0,
            own_seed_per_kilocell: 8.0,
        };
        let b = AreaControl {
            depth: 2,
            edge: 55,
            layer_per_kilocell: 10.0,
            root_seed_per_kilocell: 9.0,
            own_seed_per_kilocell: 10.0,
        };
        let runs = vec![(1u64, vec![a, b])];
        let e = area_ensemble_summary(&runs);
        assert!(e.contains("below the root's in 0/1"));
        assert!(area_summary(&[a, b]).contains("by construction"));
        assert_eq!(area_to_csv(&runs).lines().count(), 3);
        let j = controls_json(
            &ShuffleControl {
                real: survey(3),
                shuffled: survey(1),
            },
            &[a, b],
        );
        assert_eq!(
            j.chars().filter(|c| *c == '{').count(),
            j.chars().filter(|c| *c == '}').count()
        );
    }
}
