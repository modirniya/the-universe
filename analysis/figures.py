#!/usr/bin/env python3
"""Draw the findings from the artifacts under out/.

Each figure reads the CSV and JSON the CLI wrote and draws them; nothing is
computed here that the artifacts do not already hold. Figures land in
analysis/figures/. Missing artifacts skip their figure rather than failing.

    python3 analysis/figures.py            # draw everything from out/
    python3 analysis/figures.py --out DIR  # read artifacts elsewhere
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
import pandas as pd  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent

ACCENT = "#a8620a"
CONFIRMED = "#0e6b62"
CORRECTED = "#9c3524"
FAINT = "#6b6b6b"
INK = "#14181d"


def style(ax, title: str, note: str = "") -> None:
    ax.set_title(title, loc="left", fontsize=11, color=INK)
    if note:
        ax.text(0, 1.02, note, transform=ax.transAxes, fontsize=8, color=FAINT, va="bottom")
    for side in ("top", "right"):
        ax.spines[side].set_visible(False)
    ax.tick_params(colors=FAINT, labelsize=8.5)
    for side in ("left", "bottom"):
        ax.spines[side].set_color(FAINT)


def save(fig, to: Path, name: str) -> None:
    to.mkdir(parents=True, exist_ok=True)
    path = to / name
    fig.tight_layout()
    fig.savefig(path, dpi=160)
    plt.close(fig)
    print(f"  wrote {path.relative_to(ROOT)}")


def fig_cost_fidelity(out: Path, to: Path) -> bool:
    js = out / "ensemble.json"
    if not js.exists():
        return False
    d = json.loads(js.read_text())
    rows = [s for s in d["settings"] if s["label"] not in ("shuffle",)]
    fig, ax = plt.subplots(figsize=(7.5, 5))
    for s in rows:
        w, dist = s["work_ratio"]["mean"], s["distance"]["mean"]
        if w is None or dist is None:
            continue
        kind = "null" if s["label"] in ("seed", "perturb", "density") else ("ablation" if "@" in s["label"] else "limit")
        color = {"null": FAINT, "ablation": CORRECTED, "limit": ACCENT}[kind]
        marker = {"null": "x", "ablation": "s", "limit": "o"}[kind]
        ax.scatter(w, max(dist, 0.05), color=color, marker=marker, s=28, zorder=3)
        ax.annotate(s["label"], (w, max(dist, 0.05)), fontsize=6.5, color=color,
                    xytext=(3, 3), textcoords="offset points")
    ax.set_xscale("log")
    ax.set_yscale("log")
    ax.set_xlabel("work, as a share of the unconstrained universe", color=FAINT, fontsize=8.5)
    ax.set_ylabel("D: distance from the reference, in seed-to-seed sd units", color=FAINT, fontsize=8.5)
    style(ax, "Cost against fidelity, every setting of the four limits",
          "circles: settings; crosses: nulls; squares: lazy settings under other closures")
    save(fig, to, "v10-cost-fidelity.png")
    return True


def fig_divergence_traces(out: Path, to: Path) -> bool:
    csv = out / "divergence_trace.csv"
    if not csv.exists():
        return False
    df = pd.read_csv(csv)
    fig, ax = plt.subplots(figsize=(8, 4.2))
    for col in df.columns[1:]:
        if col in ("seed", "perturb", "density", "shuffle"):
            ax.plot(df.tick, df[col], color=FAINT, linewidth=1.4, label=f"null: {col}")
        elif col in ("space", "time", "speed", "lazy", "all_on"):
            ax.plot(df.tick, df[col], linewidth=1.1, label=col)
    ax.legend(frameon=False, fontsize=7.5, ncol=3, labelcolor=INK)
    ax.set_xlabel("tick", color=FAINT, fontsize=8.5)
    style(ax, "Divergence from the reference over time, pinned seed",
          "same-seed settings start correlated; the reseed null does not. read the second half.")
    save(fig, to, "v10-divergence-traces.png")
    return True


def fig_terminations(out: Path, to: Path) -> bool:
    csv = out / "nesting" / "terminations.csv"
    if not csv.exists():
        return False
    df = pd.read_csv(csv)
    code = {"budget": 0, "space": 1, "quantisation": 2}
    roots = sorted(df.root_edge.unique())
    blocks = sorted(df.block_size.unique())
    fig, axes = plt.subplots(len(roots), len(blocks), figsize=(2.6 * len(blocks), 2.3 * len(roots)), squeeze=False)
    cmap = matplotlib.colors.ListedColormap([ACCENT, CONFIRMED, CORRECTED])
    for i, r in enumerate(roots):
        for j, b in enumerate(blocks):
            ax = axes[i][j]
            cell = df[(df.root_edge == r) & (df.block_size == b)]
            piv = cell.pivot_table(index="viable_edge", columns="fraction", values="ending", aggfunc="first")
            grid = piv.map(lambda v: code.get(v, 0)).values
            ax.imshow(grid, cmap=cmap, vmin=0, vmax=2, aspect="auto")
            ax.set_xticks(range(len(piv.columns)))
            ax.set_xticklabels([f"{c:.2f}" for c in piv.columns], fontsize=6)
            ax.set_yticks(range(len(piv.index)))
            ax.set_yticklabels(list(piv.index), fontsize=6)
            ax.set_title(f"root {r}, block {b}", fontsize=8, loc="left", color=INK)
            for (y, x), v in pd.DataFrame(piv.values).stack().items():
                depth = cell[(cell.viable_edge == piv.index[y]) & (cell.fraction == piv.columns[x])].built_depth.iloc[0]
                ax.text(x, y, str(depth), ha="center", va="center", fontsize=6, color="white")
    fig.suptitle("Why a plain chain ends (orange budget, teal space, red block quantisation); numbers are depth",
                 fontsize=9, color=INK, x=0.01, ha="left")
    save(fig, to, "v10-terminations.png")
    return True


def fig_size_curve(out: Path, to: Path) -> bool:
    csv = out / "nesting" / "size_curve.csv"
    if not csv.exists():
        return False
    df = pd.read_csv(csv)
    g = df.groupby("edge")
    fig, ax = plt.subplots(figsize=(7, 4))
    m, s = g.churn.mean(), g.churn.std()
    ax.errorbar(m.index, m.values, yerr=s.values, fmt="o-", color=ACCENT, capsize=3, markersize=4)
    share = g.resolved_share.first()
    for e, v in m.items():
        ax.annotate(f"{share[e]:.2f}", (e, v), fontsize=7, color=FAINT, xytext=(0, 6), textcoords="offset points", ha="center")
    ax.set_xlabel("world edge, cells", color=FAINT, fontsize=8.5)
    ax.set_ylabel("churn, mean ± sd over seeds", color=FAINT, fontsize=8.5)
    style(ax, "Churn against world size for standalone universes",
          "labels: share of blocks the rescaled probe resolves at that size")
    save(fig, to, "v10-size-curve.png")
    return True


def fig_detection(out: Path, to: Path) -> bool:
    js = out / "detect" / "detection.json"
    if not js.exists():
        return False
    d = json.loads(js.read_text())
    tests = [t for t in d["tests"] if t["gaze"] == "looking renders"]
    limits = ["discrete_space", "discrete_time", "speed_cap", "lazy_rendering"]
    statistics = ["influence_speed", "anisotropy", "smoothness", "edge_excess"]
    fig, ax = plt.subplots(figsize=(8, 4))
    width = 0.2
    for k, st in enumerate(statistics):
        xs, ys, cs = [], [], []
        for i, lim in enumerate(limits):
            t = next((t for t in tests if t["limit"] == lim and t["statistic"] == st), None)
            if t is None or t["tpr"] is None:
                continue
            xs.append(i + (k - 1.5) * width)
            ys.append(t["tpr"])
            cs.append({"consequence": FAINT, "finding": ACCENT, "exploratory": CORRECTED}[t["standing"]])
        ax.bar(xs, ys, width, color=cs, label=st)
    ceiling = d.get("control_ceiling") or 0
    ax.axhline(ceiling, color=CONFIRMED, linewidth=1, linestyle="--")
    ax.text(-0.45, ceiling, "negative control", color=CONFIRMED, fontsize=7.5, va="bottom")
    ax.set_xticks(range(len(limits)))
    ax.set_xticklabels(limits, fontsize=8)
    ax.set_ylim(0, 1.05)
    ax.set_ylabel("power on held-out seeds", color=FAINT, fontsize=8.5)
    style(ax, "Which limits an inhabitant whose looking renders can find",
          "bars per statistic, left to right: influence_speed, anisotropy, smoothness, edge_excess; grey = consequence, red = exploratory")
    save(fig, to, "v10-detection.png")
    return True


def fig_information(out: Path, to: Path) -> bool:
    csv = out / "pipe" / "information_ensemble.csv"
    if not csv.exists():
        return False
    df = pd.read_csv(csv)
    enc = df[df.kind == "encoding"]
    piv = enc.groupby(["encoding", "task"]).mi_corrected.mean().unstack("task")
    order = ["uniform:1", "uniform:2", "uniform:3", "uniform:4", "uniform:6", "uniform:128",
             "adaptive:2", "adaptive:4", "hash:4", "hash:6", "projection:4", "projection:6", "quadrants:4"]
    piv = piv.reindex([o for o in order if o in piv.index])
    fig, ax = plt.subplots(figsize=(7.5, 5))
    im = ax.imshow(piv.values, cmap="Oranges", vmin=0, aspect="auto")
    ax.set_xticks(range(len(piv.columns)))
    ax.set_xticklabels(piv.columns, fontsize=7.5, rotation=20)
    ax.set_yticks(range(len(piv.index)))
    ax.set_yticklabels(piv.index, fontsize=7.5)
    for (y, x), v in pd.DataFrame(piv.values).stack().items():
        ax.text(x, y, f"{v:.2f}", ha="center", va="center", fontsize=7, color=INK if v < 1.5 else "white")
    fig.colorbar(im, ax=ax, label="corrected mutual information, bits per tick")
    style(ax, "What crosses the pipe, per encoding and task", "mean over seeds; quadrants on quadrant_pattern is the encoding reading itself")
    save(fig, to, "v10-information.png")
    return True


def fig_measure(out: Path, to: Path) -> bool:
    js = out / "measure" / "measure.json"
    if not js.exists():
        return False
    d = json.loads(js.read_text())
    rows = pd.DataFrame([{"prior": s["prior"], "criterion": s["criterion"], "share": s["share"]["mean"],
                          "lo": s["share"]["ci_lo"], "hi": s["share"]["ci_hi"]} for s in d["shares"]])
    piv = rows.pivot(index="prior", columns="criterion", values="share")
    fig, ax = plt.subplots(figsize=(7.5, 4))
    x = range(len(piv.index))
    w = 0.25
    for k, c in enumerate(piv.columns):
        ax.bar([i + (k - 1) * w for i in x], piv[c].values, w, label=c)
    ax.axhline(0.5, color=CORRECTED, linewidth=0.8, linestyle="--")
    ax.set_xticks(list(x))
    ax.set_xticklabels(piv.index, fontsize=8)
    ax.set_ylabel("share of laws admitted", color=FAINT, fontsize=8.5)
    ax.legend(frameon=False, fontsize=8, labelcolor=INK)
    style(ax, "The productive share under five priors and three criteria", "means over seeds; the dashed line is a majority")
    save(fig, to, "v10-measure.png")
    return True


def fig_boot(out: Path, to: Path) -> bool:
    csv = out / "boot" / "area_control.csv"
    sh = out / "boot" / "shuffle_control.csv"
    if not csv.exists() or not sh.exists():
        return False
    df = pd.read_csv(csv)
    g = df.groupby("depth")
    fig, ax = plt.subplots(figsize=(7, 4))
    m, s = g.layer_per_kilocell.mean(), g.layer_per_kilocell.std()
    ax.errorbar(m.index - 0.08, m.values, yerr=s.values, fmt="o", color=ACCENT, capsize=3, label="layer of the chain")
    m2, s2 = g.root_seed_per_kilocell.mean(), g.root_seed_per_kilocell.std()
    ax.errorbar(m2.index + 0.08, m2.values, yerr=s2.values, fmt="s", color=CONFIRMED, capsize=3, label="standalone, same size, root seed")
    floor = pd.read_csv(sh).shuffled_per_kilocell.mean()
    ax.axhline(floor, color=CORRECTED, linewidth=1, linestyle="--")
    ax.text(0.6, floor, "tracker's false-positive floor", color=CORRECTED, fontsize=7.5, va="bottom")
    ax.set_xlabel("depth", color=FAINT, fontsize=8.5)
    ax.set_ylabel("bootloaders per thousand cells", color=FAINT, fontsize=8.5)
    ax.legend(frameon=False, fontsize=8, labelcolor=INK)
    style(ax, "Bootloader density down the chain, against size alone", "mean ± sd over seeds")
    save(fig, to, "v10-boot.png")
    return True


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--out", default=str(ROOT / "out"))
    args = ap.parse_args()
    out = Path(args.out)
    to = ROOT / "analysis" / "figures"
    drawn = 0
    for f in (fig_cost_fidelity, fig_divergence_traces, fig_terminations, fig_size_curve,
              fig_detection, fig_information, fig_measure, fig_boot):
        if f(out, to):
            drawn += 1
        else:
            print(f"  skipped {f.__name__}: artifacts missing")
    print(f"{drawn} figures drawn")
    return 0


if __name__ == "__main__":
    sys.exit(main())
