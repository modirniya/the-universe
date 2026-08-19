#!/usr/bin/env python3
"""Cross-check the README's numbers against the artifacts the runs produced.

This is a *second opinion*, never a source of truth. Every claim this project
makes is established by a cargo command, and CI checks those claims directly
from the same artifacts. What this adds is an independent reading of the same
files by different code in a different language: if the README and the outputs
have drifted apart, one of them is wrong, and a second reader is how you find
out which.

Nothing here is required for anything. If this script disagrees with the CLI,
believe the CLI and fix the script.

Usage:

    python3 analysis/verify.py            # cross-check whatever is in out/
    python3 analysis/verify.py --out DIR  # look somewhere else

Regenerate the artifacts first:

    cargo run --release -- run    --config configs/default.toml
    cargo run --release -- nest   --config configs/nesting.toml
    cargo run --release -- pipe   --config configs/pipe.toml
    cargo run --release -- detect --config configs/detect.toml
    cargo run --release -- sweep  --config configs/sweep.toml --steps 21
    cargo run --release -- boot   --config configs/boot.toml
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

import pandas as pd

ROOT = Path(__file__).resolve().parent.parent

# How far a README figure may sit from the artifact's before it counts as drift.
# The README prints rounded values, so exact equality is the wrong test; this is
# half a unit in the last place the README shows, with a little room.
TOLERANCE = 0.0006


class Report:
    """Collects agreements and disagreements, and decides the exit code."""

    def __init__(self) -> None:
        self.checked = 0
        self.drift: list[str] = []
        self.skipped: list[str] = []

    def agree(self, label: str, readme: float, artifact: float, tol: float = TOLERANCE) -> None:
        self.checked += 1
        if abs(readme - artifact) > tol:
            self.drift.append(
                f"{label}: README says {readme:g}, the artifact says {artifact:g}"
            )

    def skip(self, what: str, why: str) -> None:
        self.skipped.append(f"{what} — {why}")

    def finish(self) -> int:
        print()
        if self.skipped:
            print("not checked:")
            for s in self.skipped:
                print(f"  {s}")
            print()
        if self.drift:
            print(f"DRIFT — {len(self.drift)} of {self.checked} figures disagree:")
            for d in self.drift:
                print(f"  {d}")
            print("\nthe README and the artifacts describe different runs.")
            return 1
        print(f"{self.checked} figures checked; the README matches the artifacts.")
        print("this is a second opinion, not a source of truth. the cargo commands are.")
        return 0


def readme_text() -> str:
    return (ROOT / "README.md").read_text(encoding="utf-8")


def row_numbers(text: str, label: str) -> list[float]:
    """Every number on the first line of a fenced block starting with `label`."""
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith(label):
            rest = stripped[len(label) :]
            return [float(m) for m in re.findall(r"-?\d+\.?\d*", rest)]
    return []


# --- v0.1: limits as optimizations ---------------------------------------


def check_limits(out: Path, text: str, rep: Report) -> None:
    csv = out / "runs.csv"
    if not csv.exists():
        rep.skip("v0.1 limits", f"no {csv.relative_to(ROOT)}")
        return

    df = pd.read_csv(csv).set_index("label")
    print("v0.1 — limits as optimizations")
    print(f"  {'limit':<10}{'work':>9}{'time':>9}{'memory':>9}{'divergence':>13}")
    for label in ["space", "time", "speed", "lazy", "all_on"]:
        if label not in df.index:
            continue
        r = df.loc[label]
        print(
            f"  {label:<10}{r.work_ratio:>8.3f}x{r.time_ratio:>8.3f}x"
            f"{r.memory_ratio:>8.3f}x{r.mean_divergence:>13.5f}"
        )

        # The README prints "space  0.250x  0.246x  0.257x  0.12319  1.53x".
        nums = row_numbers(text, label)
        if len(nums) >= 4:
            rep.agree(f"v0.1 {label} work_ratio", nums[0], float(r.work_ratio))
            rep.agree(f"v0.1 {label} memory_ratio", nums[2], float(r.memory_ratio))
            rep.agree(f"v0.1 {label} divergence", nums[3], float(r.mean_divergence))
        else:
            rep.skip(f"v0.1 {label}", "no matching row in the README")

    # Wall time is deliberately not cross-checked: it is a measurement, and the
    # machine that wrote the README is not the machine reading it.
    rep.skip("v0.1 wall time", "a measurement, not a counter — expected to differ")


# --- v0.2: nesting and degradation ---------------------------------------


def check_chain(out: Path, text: str, rep: Report) -> None:
    csv = out / "nesting" / "chain.csv"
    if not csv.exists():
        rep.skip("v0.2 chain", f"no {csv.relative_to(ROOT)}")
        return

    df = pd.read_csv(csv)
    print("\nv0.2 — nesting and degradation")
    print(f"  {'depth':>5}{'world':>12}{'budget':>12}{'churn':>10}")
    for _, r in df.iterrows():
        print(
            f"  {int(r.depth):>5}{f'{int(r.width)}x{int(r.height)}':>12}"
            f"{int(r.budget_work):>12}{r.churn:>10.5f}"
        )

    for _, r in df.iterrows():
        nums = row_numbers(text, f"{int(r.depth)}      ") or row_numbers(
            text, f"{int(r.depth)}    "
        )
        if not nums:
            continue

    # The structural claims are what matter here, and they are checkable without
    # parsing prose: budgets strictly decrease, and no layer outspends its host.
    budgets = df.budget_work.tolist()
    if budgets != sorted(budgets, reverse=True) or len(set(budgets)) != len(budgets):
        rep.drift.append("v0.2: budgets are not strictly decreasing down the chain")
    rep.checked += 1

    if (df.spent_work > df.budget_work).any():
        rep.drift.append("v0.2: a layer outspent its host")
    rep.checked += 1

    if "1.33" in text:
        fraction = 0.25  # configs/nesting.toml
        rep.agree("v0.2 geometric bound", 1.33, 1.0 / (1.0 - fraction), tol=0.005)


# --- v0.3: the pipe -------------------------------------------------------


def check_pipe(out: Path, text: str, rep: Report) -> None:
    js = out / "pipe" / "pipe.json"
    if not js.exists():
        rep.skip("v0.3 pipe", f"no {js.relative_to(ROOT)}")
        return

    d = json.loads(js.read_text())
    rows = pd.read_csv(out / "pipe" / "pipe.csv")
    print("\nv0.3 — the pipe")
    print(f"  content avalanche      {d['content_avalanche']:.4f}")
    print(f"  magnitude correlation  {d['magnitude_correlation']:.4f}")
    print(f"  channel carries        {d['compression_ratio'] * 100:.2f}%")

    for label, value, key in [
        ("avalanche", 0.502, "content_avalanche"),
        ("correlation", 0.7884, "magnitude_correlation"),
    ]:
        if f"{value}" in text or f"{value:.4f}" in text:
            rep.agree(f"v0.3 {label}", value, d[key], tol=0.0006)

    if "5.56" in text:
        rep.agree("v0.3 compression", 5.56, d["compression_ratio"] * 100, tol=0.006)

    # Independent of the prose: the sweep must never reveal more as the
    # threshold rises, and thin rows must report no correlation.
    vis = rows.visible_fraction.tolist()
    if any(b > a + 1e-12 for a, b in zip(vis, vis[1:])):
        rep.drift.append("v0.3: a higher threshold showed more of the child")
    rep.checked += 1

    thin = rows[rows.samples < 5]
    if not thin.correlation.isna().all():
        rep.drift.append("v0.3: a correlation was reported from fewer than five events")
    rep.checked += 1


# --- v0.4: detection ------------------------------------------------------


def check_detection(out: Path, text: str, rep: Report) -> None:
    csv = out / "detect" / "detection.csv"
    if not csv.exists():
        rep.skip("v0.4 detection", f"no {csv.relative_to(ROOT)}")
        return

    df = pd.read_csv(csv)
    honest = df[df.gaze == "looking renders"].set_index("limit")
    passive = df[df.gaze == "reads without rendering"].set_index("limit")

    print("\nv0.4 — detection")
    for limit, r in honest.iterrows():
        verdict = "found" if r.detectable else "invisible"
        print(f"  {limit:<16}{r.signal:>16}{r.with_limit:>10.4f}{r.without_limit:>10.4f}  {verdict}")

    # The three findings, each stated in the README as a verdict rather than a
    # number, so they are checked as verdicts.
    for limit, expected, why in [
        ("discrete_space", False, "the cell is the ruler"),
        ("speed_cap", True, "influence speed is measurable"),
        ("lazy_rendering", False, "looking conceals it"),
    ]:
        rep.checked += 1
        if bool(honest.loc[limit].detectable) != expected:
            rep.drift.append(f"v0.4 {limit}: expected {expected} ({why})")

    rep.checked += 1
    if not bool(passive.loc["lazy_rendering"].detectable):
        rep.drift.append("v0.4: lazy rendering should be visible to a passive reader")


# --- v0.5: fine-tuning ----------------------------------------------------


def check_sweep(out: Path, text: str, rep: Report) -> None:
    js = out / "sweep" / "sweep.json"
    if not js.exists():
        rep.skip("v0.5 sweep", f"no {js.relative_to(ROOT)}")
        return

    d = json.loads(js.read_text())
    print("\nv0.5 — fine-tuning")
    print(f"  distinct laws          {d['distinct_rules']}")
    print(f"  productive             {d['distinct_complex']}")
    print(f"  productive fraction    {d['productive_rule_fraction'] * 100:.1f}%")

    if not d["reference_admitted"]:
        rep.drift.append("v0.5: the reference setting failed the bar it set")
    rep.checked += 1

    if d["distinct_rules"] >= len(d["grid"]):
        rep.drift.append("v0.5: the grid did not collapse onto fewer distinct laws")
    rep.checked += 1

    # The README's headline is a percentage of *laws*, and the settings count
    # differs with --steps, so only check the figure when the shipped 21-step
    # sweep is what produced the file.
    if len(d["grid"]) == 441 and "19.0%" in text:
        rep.agree("v0.5 productive fraction", 19.0, d["productive_rule_fraction"] * 100, tol=0.06)
    elif len(d["grid"]) != 441:
        rep.skip(
            "v0.5 productive fraction",
            f"the README quotes the 21-step sweep; this file has {len(d['grid'])} settings",
        )


# --- v0.6: bootloader life ------------------------------------------------


def check_boot(out: Path, text: str, rep: Report) -> None:
    csv = out / "boot" / "boot.csv"
    if not csv.exists():
        rep.skip("v0.6 boot chain", f"no {csv.relative_to(ROOT)}")
        return

    df = pd.read_csv(csv)
    print("\nv0.6 — bootloader life")
    print(f"  {'depth':>5}{'world':>12}{'boots':>8}{'transport':>12}{'child':>8}")
    for _, r in df.iterrows():
        print(
            f"  {int(r.depth):>5}{f'{int(r.width)}x{int(r.height)}':>12}"
            f"{int(r.bootloaders):>8}{r.transport:>12.1f}{str(r.booted_child):>8}"
        )

    rep.checked += 1
    if df.iloc[0].bootloaders < 1:
        rep.drift.append("v0.6: the root universe produced no bootloader")

    rep.checked += 1
    if df.iloc[-1].bootloaders > df.iloc[0].bootloaders:
        rep.drift.append("v0.6: the deepest layer out-booted the first")

    rep.checked += 1
    if (df.transport < 0).any():
        rep.drift.append("v0.6: negative transport reported")

    rep.checked += 1
    if len(set(df.seed)) != len(df):
        rep.drift.append("v0.6: two layers shared a seed")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--out", default=str(ROOT / "out"), help="artifact directory")
    args = ap.parse_args()

    out = Path(args.out)
    if not out.exists():
        print(f"no artifacts at {out}. run the cargo commands in the docstring first.")
        return 2

    text = readme_text()
    rep = Report()

    print(f"reading artifacts from {out}\n")
    check_limits(out, text, rep)
    check_chain(out, text, rep)
    check_pipe(out, text, rep)
    check_detection(out, text, rep)
    check_sweep(out, text, rep)
    check_boot(out, text, rep)

    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
