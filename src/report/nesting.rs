//! Nesting's additional reports: the termination map, the size control and
//! the churn-against-size curve.

use super::{Written, json_f64, seed_list};
use crate::layer::{
    MAP_BLOCKS, MAP_FLOORS, MAP_FRACTIONS, MAP_ROOTS, SizeControl, SizeRow, Termination,
    TerminationCell,
};
use crate::stats::Summary;
use std::fmt::Write as _;
use std::io;
use std::path::{Path, PathBuf};

/// One root edge and block size: a grid of floors against fractions.
fn grid(cells: &[TerminationCell], root: usize, block: usize) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "root {root}x{root}, blocks of {block}:");
    let _ = write!(s, "  floor\\frac ");
    for f in MAP_FRACTIONS {
        let _ = write!(s, "{f:>5.2}");
    }
    s.push('\n');
    for floor in MAP_FLOORS {
        let _ = write!(s, "  edge {floor:>4}  ");
        for f in MAP_FRACTIONS {
            let cell = cells.iter().find(|c| {
                c.root_edge == root
                    && c.block_size == block
                    && c.viable_edge == *floor
                    && c.fraction == *f
            });
            let _ = write!(
                s,
                "{:>5}",
                cell.map_or("?".to_string(), |c| format!(
                    "{}{}",
                    c.ending.glyph(),
                    c.built_depth
                ))
            );
        }
        s.push('\n');
    }
    s
}

/// The whole map, as text.
pub fn termination_summary(cells: &[TerminationCell]) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "where a plain chain ends, computed from the definitions alone (no physics is run):\n\
         each cell is the ending and the depth built. $ budget, # space, q block quantisation\n\
         (a world above the floor would fit by area, but none fits by block)."
    );
    for root in MAP_ROOTS {
        for block in MAP_BLOCKS {
            s.push_str(&grid(cells, *root, *block));
        }
    }
    let n = cells.len();
    let count = |t: Termination| cells.iter().filter(|c| c.ending == t).count();
    let truncated = cells.iter().filter(|c| c.truncated()).count();
    let _ = writeln!(
        s,
        "\nover {n} settings: budget ends {}, space ends {}, quantisation ends {}; integer flooring\n\
         cost a layer the closed form allowed in {} of the budget endings.",
        count(Termination::Budget),
        count(Termination::Space),
        count(Termination::Quantisation),
        truncated
    );
    let _ = writeln!(
        s,
        "all of this follows from the degradation rule, the size floor and the block partition;\n\
         the map shows which one binds where, which the rules do not say by themselves."
    );
    s
}

const MAP_COLUMNS: &[&str] = &[
    "root_edge",
    "block_size",
    "fraction",
    "viable_edge",
    "built_depth",
    "closed_form_depth",
    "ending",
    "truncated",
];

pub fn termination_to_csv(cells: &[TerminationCell]) -> String {
    let mut s = MAP_COLUMNS.join(",");
    s.push('\n');
    for c in cells {
        let _ = writeln!(
            s,
            "{},{},{:.2},{},{},{},{},{}",
            c.root_edge,
            c.block_size,
            c.fraction,
            c.viable_edge,
            c.built_depth,
            c.closed_form_depth,
            c.ending.code(),
            c.truncated()
        );
    }
    s
}

pub fn write_terminations(cells: &[TerminationCell], out_dir: &Path) -> io::Result<PathBuf> {
    std::fs::create_dir_all(out_dir)?;
    let path = out_dir.join("terminations.csv");
    std::fs::write(&path, termination_to_csv(cells))?;
    Ok(path)
}

/// The size control, for one seed.
pub fn size_control_summary(controls: &[SizeControl]) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "size control: each layer beside a standalone universe of its own size, seed and probe"
    );
    let _ = writeln!(
        s,
        "{:>5} {:>6} {:>12} {:>12} {:>12} {:>12} {:>9}",
        "depth", "edge", "layer churn", "alone churn", "layer work", "alone work", "identical"
    );
    for c in controls {
        let _ = writeln!(
            s,
            "{:>5} {:>6} {:>12.5} {:>12.5} {:>12} {:>12} {:>9}",
            c.depth,
            c.edge,
            c.layer_churn,
            c.standalone_churn,
            c.layer_work,
            c.standalone_work,
            c.identical()
        );
    }
    let all = controls.iter().all(|c| c.identical());
    let _ = writeln!(
        s,
        "{}",
        if all {
            "identical in every layer, by construction: nothing about a layer's host reaches it, so a\n\
             trend down the chain is a trend in world size and not a signature of nesting."
        } else {
            "WARNING: a layer differed from a standalone universe of its size; something about the\n\
             host is reaching the child, which the model does not intend."
        }
    );
    s
}

/// The churn-against-size curve across seeds.
pub fn size_curve_summary(runs: &[(u64, Vec<SizeRow>)]) -> String {
    let mut s = String::new();
    let seeds: Vec<u64> = runs.iter().map(|(s, _)| *s).collect();
    let _ = writeln!(
        s,
        "churn against world size for standalone universes, {} seeds ({}):",
        runs.len(),
        seed_list(&seeds)
    );
    let _ = writeln!(
        s,
        "{:>6} {:>10} {:>30} {:>30}",
        "edge", "resolved", "churn: mean ± sd [95% ci]", "occupancy: mean ± sd [95% ci]"
    );
    let edges: Vec<usize> = runs
        .first()
        .map(|(_, rows)| rows.iter().map(|r| r.edge).collect())
        .unwrap_or_default();
    for edge in edges {
        let churn = Summary::of(
            runs.iter()
                .filter_map(|(_, rows)| rows.iter().find(|r| r.edge == edge).map(|r| r.churn)),
        );
        let occ = Summary::of(
            runs.iter()
                .filter_map(|(_, rows)| rows.iter().find(|r| r.edge == edge).map(|r| r.occupancy)),
        );
        let share = runs
            .first()
            .and_then(|(_, rows)| rows.iter().find(|r| r.edge == edge))
            .map_or(f64::NAN, |r| r.resolved_share);
        let _ = writeln!(
            s,
            "{edge:>6} {share:>10.2} {:>30} {:>30}",
            super::limits::summary_cell(&churn, 5),
            super::limits::summary_cell(&occ, 4)
        );
    }
    let _ = writeln!(
        s,
        "'resolved' is the share of blocks the rescaled probe resolves at that size: block\n\
         quantisation changes what fraction of a world is computed in full, so churn at the macro\n\
         scale is a function of size, of the partition and of the closure for unobserved ground,\n\
         before anything is said about nesting."
    );
    s
}

const SIZE_COLUMNS: &[&str] = &[
    "seed",
    "edge",
    "churn",
    "occupancy",
    "resolved_share",
    "work",
];

pub fn size_curve_to_csv(runs: &[(u64, Vec<SizeRow>)]) -> String {
    let mut s = SIZE_COLUMNS.join(",");
    s.push('\n');
    for (seed, rows) in runs {
        for r in rows {
            let _ = writeln!(
                s,
                "{seed},{},{:.6},{:.6},{:.6},{}",
                r.edge, r.churn, r.occupancy, r.resolved_share, r.work
            );
        }
    }
    s
}

pub fn write_size_curve(runs: &[(u64, Vec<SizeRow>)], out_dir: &Path) -> io::Result<PathBuf> {
    std::fs::create_dir_all(out_dir)?;
    let path = out_dir.join("size_curve.csv");
    std::fs::write(&path, size_curve_to_csv(runs))?;
    Ok(path)
}

pub fn size_control_json(controls: &[SizeControl]) -> String {
    let rows: Vec<String> = controls
        .iter()
        .map(|c| {
            format!(
                "    {{\"depth\": {}, \"edge\": {}, \"layer_churn\": {}, \"standalone_churn\": {}, \"layer_work\": {}, \"standalone_work\": {}, \"identical\": {}}}",
                c.depth,
                c.edge,
                json_f64(c.layer_churn),
                json_f64(c.standalone_churn),
                c.layer_work,
                c.standalone_work,
                c.identical()
            )
        })
        .collect();
    format!("[\n{}\n]\n", rows.join(",\n"))
}

pub fn write_size_control(controls: &[SizeControl], out_dir: &Path) -> io::Result<Written> {
    std::fs::create_dir_all(out_dir)?;
    let json = out_dir.join("size_control.json");
    std::fs::write(&json, size_control_json(controls))?;
    Ok(Written {
        csv: out_dir.join("size_curve.csv"),
        json,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(
        root: usize,
        block: usize,
        f: f64,
        floor: usize,
        ending: Termination,
    ) -> TerminationCell {
        TerminationCell {
            root_edge: root,
            block_size: block,
            fraction: f,
            viable_edge: floor,
            built_depth: 2,
            closed_form_depth: 3,
            ending,
        }
    }

    #[test]
    fn the_map_prints_every_grid_and_the_csv_is_rectangular() {
        let mut cells = Vec::new();
        for &r in MAP_ROOTS {
            for &b in MAP_BLOCKS {
                for &fl in MAP_FLOORS {
                    for &f in MAP_FRACTIONS {
                        cells.push(cell(r, b, f, fl, Termination::Budget));
                    }
                }
            }
        }
        let s = termination_summary(&cells);
        assert_eq!(
            s.matches("root ").count(),
            MAP_ROOTS.len() * MAP_BLOCKS.len()
        );
        let csv = termination_to_csv(&cells);
        assert_eq!(csv.lines().count(), cells.len() + 1);
        for line in csv.lines() {
            assert_eq!(line.split(',').count(), MAP_COLUMNS.len());
        }
        assert!(
            cells[0].truncated(),
            "built 2 of an allowed 3 on a budget ending"
        );
    }

    #[test]
    fn the_size_control_says_when_it_is_a_tautology() {
        let c = SizeControl {
            depth: 1,
            edge: 64,
            layer_churn: 0.1,
            standalone_churn: 0.1,
            layer_work: 10,
            standalone_work: 10,
        };
        assert!(c.identical());
        assert!(size_control_summary(&[c]).contains("by construction"));
        let d = SizeControl {
            standalone_churn: 0.2,
            ..c
        };
        assert!(size_control_summary(&[d]).contains("WARNING"));
        assert!(size_control_json(&[c, d]).contains("\"identical\": false"));
    }

    #[test]
    fn the_size_curve_summarises_across_seeds() {
        let row = |edge, churn| SizeRow {
            edge,
            churn,
            occupancy: 0.2,
            resolved_share: 0.25,
            work: 1,
        };
        let runs = vec![
            (1u64, vec![row(16, 0.01), row(32, 0.02)]),
            (2u64, vec![row(16, 0.03), row(32, 0.04)]),
        ];
        let s = size_curve_summary(&runs);
        assert!(s.contains("2 seeds"));
        assert!(s.contains("0.02000 ± 0.01414"));
        assert_eq!(size_curve_to_csv(&runs).lines().count(), 5);
    }
}
