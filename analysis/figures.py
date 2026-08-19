#!/usr/bin/env python3
"""Draw the findings from the artifacts the runs produced.

Figures, not claims. Everything plotted here comes from a CSV or JSON a cargo
command wrote, and nothing is computed that the CLI did not already report. If a
picture and the CLI disagree, the CLI is right.

Usage:

    python3 analysis/figures.py                  # write into analysis/figures/
    python3 analysis/figures.py --out DIR --to D
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import matplotlib
import pandas as pd

matplotlib.use("Agg")  # never needs a display
import matplotlib.pyplot as plt  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent

# The project's palette, so a figure sits beside the README and the viewer
# without looking like it came from somewhere else.
INK = "#14181D"
FAINT = "#78848F"
RULE = "#D0D9E0"
ACCENT = "#A8620A"
CONFIRMED = "#0E6B62"
CORRECTED = "#9C3524"


def style(ax, title: str, subtitle: str = "") -> None:
    ax.set_title(title, loc="left", color=INK, fontsize=12, pad=14 if subtitle else 8)
    if subtitle:
        ax.text(
            0, 1.02, subtitle, transform=ax.transAxes, color=FAINT, fontsize=8.5,
            va="bottom",
        )
    for side in ("top", "right"):
        ax.spines[side].set_visible(False)
    for side in ("left", "bottom"):
        ax.spines[side].set_color(RULE)
    ax.tick_params(colors=FAINT, labelsize=8.5)
    ax.grid(axis="y", color=RULE, linewidth=0.6, alpha=0.7)
    ax.set_axisbelow(True)


def save(fig, to: Path, name: str) -> None:
    to.mkdir(parents=True, exist_ok=True)
    path = to / name
    fig.savefig(path, dpi=160, bbox_inches="tight", facecolor="white")
    plt.close(fig)
    print(f"  wrote {path.relative_to(ROOT)}")


def fig_limits(out: Path, to: Path) -> bool:
    csv = out / "runs.csv"
    if not csv.exists():
        return False
    df = pd.read_csv(csv).set_index("label")
    order = [k for k in ["space", "time", "speed", "lazy", "all_on"] if k in df.index]
    ref = df.loc["all_off"] if "all_off" in df.index else None

    fig, (a, b) = plt.subplots(1, 2, figsize=(11, 4))

    x = range(len(order))
    a.bar([i - 0.2 for i in x], [df.loc[k].work_ratio for k in order], 0.4,
          color=ACCENT, label="work")
    a.bar([i + 0.2 for i in x], [df.loc[k].memory_ratio for k in order], 0.4,
          color=CONFIRMED, label="memory")
    a.set_xticks(list(x))
    a.set_xticklabels(order)
    a.axhline(1.0, color=FAINT, linewidth=0.8, linestyle="--")
    a.legend(frameon=False, fontsize=8.5, labelcolor=FAINT)
    style(a, "What each limit costs",
          "fraction of the unconstrained universe; lower is cheaper")

    # Divergence read against the chaos floor, which is the only way it means
    # anything: this world decorrelates under any perturbation.
    floor = None
    js = out / "report.json"
    if js.exists():
        floor = json.loads(js.read_text()).get("chaos_floor")
    divs = [df.loc[k].mean_divergence for k in order]
    colors = [
        CONFIRMED if (floor and d <= floor) else CORRECTED for d in divs
    ]
    b.bar(list(x), divs, 0.55, color=colors)
    if floor:
        b.axhline(floor, color=ACCENT, linewidth=1.2)
        b.text(-0.42, floor, "chaos floor", color=ACCENT, fontsize=8,
               va="bottom", ha="left")
    b.set_xticks(list(x))
    b.set_xticklabels(order)
    style(b, "What each limit changed",
          "macro divergence; below the floor is indistinguishable from a reseed")

    save(fig, to, "v01-limits.png")
    return True


def fig_chain(out: Path, to: Path) -> bool:
    csv = out / "nesting" / "chain.csv"
    if not csv.exists():
        return False
    df = pd.read_csv(csv)

    fig, (a, b) = plt.subplots(1, 2, figsize=(11, 4))
    a.bar(df.depth, df.budget_work, 0.5, color=ACCENT)
    a.set_yscale("log")
    a.set_xticks(df.depth)
    a.set_xlabel("depth", color=FAINT, fontsize=8.5)
    style(a, "Each layer is poorer than its host", "budget in neighbour visits, log scale")

    b.plot(df.depth, df.churn, marker="o", color=CONFIRMED, linewidth=1.6)
    b.set_yscale("log")
    b.set_xticks(df.depth)
    b.set_xlabel("depth", color=FAINT, fontsize=8.5)
    style(b, "And produces less", "churn: mean tick-to-tick change, log scale")

    save(fig, to, "v02-chain.png")
    return True


def fig_pipe(out: Path, to: Path) -> bool:
    csv = out / "pipe" / "pipe.csv"
    if not csv.exists():
        return False
    df = pd.read_csv(csv)
    d = json.loads((out / "pipe" / "pipe.json").read_text())

    fig, ax = plt.subplots(figsize=(7.5, 4))
    ax.plot(df.threshold, df.visible_fraction * 100, marker="o", color=ACCENT,
            linewidth=1.6, label="of the child's history that registers")

    thin = df[df.samples < 5]
    if not thin.empty:
        ax.scatter(thin.threshold, thin.visible_fraction * 100, s=80,
                   facecolors="none", edgecolors=CORRECTED, linewidths=1.4,
                   label="too few events to report a correlation", zorder=5)

    ax.set_xlabel("logging threshold", color=FAINT, fontsize=8.5)
    ax.set_ylabel("% registering", color=FAINT, fontsize=8.5)
    ax.legend(frameon=False, fontsize=8.5, labelcolor=FAINT)
    style(ax, "A parent watching at the wrong resolution",
          f"content avalanche {d['content_avalanche']:.3f} — the arrangement did not survive; "
          f"magnitude correlation {d['magnitude_correlation']:.3f} — the amount did")

    save(fig, to, "v03-pipe.png")
    return True


def fig_sweep(out: Path, to: Path) -> bool:
    js = out / "sweep" / "sweep.json"
    if not js.exists():
        return False
    d = json.loads(js.read_text())
    steps = d["steps"]
    grid = d["grid"]

    # Same map the CLI prints as ASCII, as an image.
    complexity = [[0.0] * steps for _ in range(steps)]
    for i, o in enumerate(grid):
        complexity[i // steps][i % steps] = 1.0 if o["complex"] else (
            0.5 if o["activity"] > d["bar"]["max_activity"] else 0.0
        )

    fig, ax = plt.subplots(figsize=(6.5, 5.5))
    cmap = matplotlib.colors.ListedColormap(["#EDF0F3", CORRECTED, CONFIRMED])
    ax.imshow(complexity, cmap=cmap, origin="lower", vmin=0, vmax=1,
              extent=[d["min"], d["max"], d["min"], d["max"]], aspect="auto")
    ax.set_xlabel("birth centre", color=FAINT, fontsize=8.5)
    ax.set_ylabel("survive centre", color=FAINT, fontsize=8.5)
    ax.grid(False)
    style(ax, "The band that produces anything",
          f"{d['distinct_complex']} of {d['distinct_rules']} distinct laws productive "
          f"({d['productive_rule_fraction'] * 100:.1f}%); red is chaotic, green is complex")

    save(fig, to, "v05-sweep.png")
    return True


def fig_boot(out: Path, to: Path) -> bool:
    csv = out / "boot" / "boot.csv"
    if not csv.exists():
        return False
    df = pd.read_csv(csv)

    fig, ax = plt.subplots(figsize=(7.5, 4))
    colors = [CONFIRMED if b else CORRECTED for b in df.booted_child]
    ax.bar(df.depth, df.bootloaders, 0.5, color=colors)
    for _, r in df.iterrows():
        ax.text(r.depth, r.bootloaders, f" {int(r.width)}x{int(r.height)}",
                ha="center", va="bottom", color=FAINT, fontsize=8)
    ax.set_xticks(df.depth)
    ax.set_xlabel("depth", color=FAINT, fontsize=8.5)
    style(ax, "Poorer layers produce less life",
          "structures that persist, stay localized and travel; red booted nothing")

    save(fig, to, "v06-boot.png")
    return True


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--out", default=str(ROOT / "out"), help="artifact directory")
    ap.add_argument("--to", default=str(ROOT / "analysis" / "figures"), help="where to write")
    args = ap.parse_args()

    out, to = Path(args.out), Path(args.to)
    if not out.exists():
        print(f"no artifacts at {out}. run the cargo commands first.")
        return 2

    print(f"reading artifacts from {out}")
    drawn = sum(
        [
            fig_limits(out, to),
            fig_chain(out, to),
            fig_pipe(out, to),
            fig_sweep(out, to),
            fig_boot(out, to),
        ]
    )
    if drawn == 0:
        print("nothing to draw — no recognised artifacts found.")
        return 2
    print(f"\n{drawn} figures drawn from artifacts the CLI wrote.")
    print("pictures, not claims. where one disagrees with the CLI, the CLI is right.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
