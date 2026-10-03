#!/usr/bin/env python3
"""A second opinion on the ensemble summaries.

Every summary the CLI prints — a mean, a standard deviation, a 95% interval,
a Wilson interval, a count of seeds — is computed in Rust and written to JSON.
The per-seed values it was computed from are written to CSV beside it. This
script recomputes the summaries from the per-seed CSVs with pandas and compares
them to the JSON, so that a mistake in the Rust statistics would show up as a
disagreement between two implementations in two languages.

It establishes nothing on its own. If it disagrees with the CLI, one of the two
is wrong and the disagreement is the finding. The claims ledger is checked by
`analysis/ledger.py`; this checks the arithmetic beneath it.

Usage:

    python3 analysis/verify.py            # cross-check whatever is in out/
    python3 analysis/verify.py --out DIR  # look somewhere else

Regenerate the artifacts first with `scripts/reproduce.sh`.
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path

import pandas as pd
from scipy import stats as sps

ROOT = Path(__file__).resolve().parent.parent
TOL = 1e-4


class Report:
    def __init__(self) -> None:
        self.checked = 0
        self.drift: list[str] = []
        self.skipped: list[str] = []

    def agree(self, label: str, ours: float, theirs: float, tol: float = TOL) -> None:
        self.checked += 1
        if (math.isnan(ours) and math.isnan(theirs)):
            return
        if abs(ours - theirs) > tol * max(1.0, abs(theirs)):
            self.drift.append(f"{label}: pandas says {ours:.6g}, the CLI wrote {theirs:.6g}")

    def skip(self, what: str, why: str) -> None:
        self.skipped.append(f"{what} — {why}")

    def finish(self) -> int:
        print()
        for s in self.skipped:
            print(f"not checked: {s}")
        if self.drift:
            print(f"\nDRIFT — {len(self.drift)} of {self.checked} summaries disagree:")
            for d in self.drift:
                print(f"  {d}")
            return 1
        print(f"{self.checked} summaries recomputed; pandas agrees with the CLI.")
        print("this is a second opinion, not a source of truth. the cargo commands are.")
        return 0


def t_half(n: int, sd: float) -> float:
    if n < 2:
        return float("nan")
    return float(sps.t.ppf(0.975, n - 1)) * sd / math.sqrt(n)


def summary(values) -> dict:
    v = pd.Series(values, dtype=float).dropna()
    v = v[~v.isin([float("inf"), float("-inf")])]
    n = len(v)
    if n == 0:
        return {"n": 0}
    sd = float(v.std(ddof=1)) if n > 1 else float("nan")
    return {
        "n": n,
        "mean": float(v.mean()),
        "median": float(v.median()),
        "sd": sd,
        "min": float(v.min()),
        "max": float(v.max()),
        "ci_lo": float(v.mean()) - t_half(n, sd),
        "ci_hi": float(v.mean()) + t_half(n, sd),
    }


def compare_summary(rep: Report, label: str, ours: dict, theirs: dict) -> None:
    if ours["n"] != theirs.get("n"):
        rep.drift.append(f"{label}: n {ours['n']} vs {theirs.get('n')}")
        return
    for k in ("mean", "sd", "min", "max", "ci_lo", "ci_hi"):
        if theirs.get(k) is None:
            continue
        # The Rust t table is interpolated between tabulated rows; allow for it.
        tol = 0.02 if k.startswith("ci") else TOL
        rep.agree(f"{label}.{k}", ours[k], float(theirs[k]), tol)


def wilson(k: int, n: int) -> tuple[float, float]:
    if n == 0:
        return float("nan"), float("nan")
    z = 1.96
    p = k / n
    denom = 1 + z * z / n
    centre = (p + z * z / (2 * n)) / denom
    half = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / denom
    return max(0.0, centre - half), min(1.0, centre + half)


# --- Theory 1 ---------------------------------------------------------------

def verify_limits(out: Path, rep: Report) -> None:
    csv, js = out / "ensemble.csv", out / "ensemble.json"
    if not csv.exists() or not js.exists():
        rep.skip("Theory 1", "out/ensemble.csv or ensemble.json missing")
        return
    df = pd.read_csv(csv)
    d = json.loads(js.read_text())
    by_label = {s["label"]: s for s in d["settings"]}
    for label, grp in df.groupby("label"):
        if label not in by_label:
            continue
        s = by_label[label]
        compare_summary(rep, f"run/{label}/div_half", summary(grp.div_half), s["div_half"])
        compare_summary(rep, f"run/{label}/distance", summary(grp.distance), s["distance"])
        compare_summary(rep, f"run/{label}/work_ratio", summary(grp.work_ratio), s["work_ratio"])
        seed_rows = df[(df.label == "seed")].set_index("seed").div_half
        ratio = grp.set_index("seed").div_half / seed_rows.reindex(grp.seed).values
        compare_summary(rep, f"run/{label}/ratio_to_reseed", summary(ratio), s["ratio_to_reseed"])
        above = int((ratio > 1.0).sum())
        rep.agree(f"run/{label}/above_reseed", above, s["above_reseed"], 0)
    # The pairwise null's n is n(n-1)/2.
    n = len(d["seeds"])
    rep.agree("run/pairwise_null.n", n * (n - 1) // 2, d["pairwise_null"]["n"], 0)


# --- Detection --------------------------------------------------------------

def verify_detect(out: Path, rep: Report) -> None:
    ev, js = out / "detect" / "evidence.csv", out / "detect" / "detection.json"
    if not ev.exists() or not js.exists():
        rep.skip("detection", "evidence.csv or detection.json missing")
        return
    df = pd.read_csv(ev)
    d = json.loads(js.read_text())
    n_cal = d["n_cal"]
    seeds = sorted(df.seed.unique(), key=lambda s: list(df.seed.unique()).index(s))
    cal, ev_seeds = set(seeds[:n_cal]), set(seeds[n_cal:])
    stats_cols = {"influence_speed": "influence_speed", "anisotropy": "anisotropy",
                  "smoothness": "smoothness", "edge_excess": "edge_excess"}
    for t in d["tests"]:
        h0 = f"without:{t['limit']}"
        col = stats_cols[t["statistic"]]
        rows = df[df.gaze == t["gaze"]]
        pairs = rows.pivot_table(index="seed", columns="condition", values=col, aggfunc="first")
        if h0 not in pairs or "all_on" not in pairs:
            continue
        c = pairs.loc[[s for s in pairs.index if s in cal], [h0, "all_on"]].dropna()
        if len(c) == 0:
            continue
        direction = "above" if c["all_on"].mean() > c[h0].mean() else ("below" if c["all_on"].mean() < c[h0].mean() else "none")
        rep.agree(f"detect/{t['gaze']}/{t['limit']}/{t['statistic']}/direction",
                  0 if direction == t["direction"] else 1, 0, 0)
        if direction == "none":
            continue
        thr = c[h0].max() if direction == "above" else c[h0].min()
        rep.agree(f"detect/{t['gaze']}/{t['limit']}/{t['statistic']}/threshold", thr, t["threshold"])
        e = pairs.loc[[s for s in pairs.index if s in ev_seeds], [h0, "all_on"]]
        flag = (lambda v: v > thr) if direction == "above" else (lambda v: v < thr)
        fp = int(flag(e[h0].dropna()).sum())
        tp = int(flag(e["all_on"].dropna()).sum())
        rep.agree(f"detect/{t['gaze']}/{t['limit']}/{t['statistic']}/false_positives", fp, t["false_positives"], 0)
        rep.agree(f"detect/{t['gaze']}/{t['limit']}/{t['statistic']}/true_positives", tp, t["true_positives"], 0)


# --- Pipe -------------------------------------------------------------------

def verify_information(out: Path, rep: Report) -> None:
    csv = out / "pipe" / "information_ensemble.csv"
    if not csv.exists():
        rep.skip("pipe information", "information_ensemble.csv missing")
        return
    df = pd.read_csv(csv)
    # mi_corrected is mi - mi_null, row by row.
    rep.agree("pipe/mi_corrected", float((df.mi - df.mi_null - df.mi_corrected).abs().max()), 0.0, 1e-5)
    # Entropies are bounded by log2 of the level count.
    enc = df[df.kind == "encoding"]
    over = enc[enc.h_symbol > enc.bits.clip(upper=6) + 1e-9]
    rep.agree("pipe/h_symbol <= min(bits,6)", len(over), 0, 0)


# --- Nesting ----------------------------------------------------------------

def verify_nesting(out: Path, rep: Report) -> None:
    ctl = out / "nesting" / "size_control.json"
    if ctl.exists():
        for row in json.loads(ctl.read_text()):
            rep.agree(f"nest/size_control/depth{row['depth']}", 0 if row["identical"] else 1, 0, 0)
    terms = out / "nesting" / "terminations.csv"
    if terms.exists():
        df = pd.read_csv(terms)
        rep.agree("nest/terminations/built<=closed_form", int((df.built_depth > df.closed_form_depth).sum()), 0, 0)
    else:
        rep.skip("nesting terminations", "terminations.csv missing")


# --- Bootloaders ------------------------------------------------------------

def verify_boot(out: Path, rep: Report) -> None:
    csv = out / "boot" / "shuffle_control.csv"
    if not csv.exists():
        rep.skip("boot shuffle control", "shuffle_control.csv missing")
        return
    df = pd.read_csv(csv)
    rep.agree("boot/shuffle/per_kilocell", float((df.real_bootloaders * 1000 / df.cells - df.real_per_kilocell).abs().max()), 0.0, 1e-5)
    area = out / "boot" / "area_control.csv"
    if area.exists():
        a = pd.read_csv(area)
        rep.agree("boot/area/identical", int((~a.identical).sum()), 0, 0)


# --- Measure ----------------------------------------------------------------

def verify_measure(out: Path, rep: Report) -> None:
    csv, js = out / "measure" / "measure.csv", out / "measure" / "measure.json"
    if not csv.exists() or not js.exists():
        rep.skip("measure", "measure.csv or measure.json missing")
        return
    df = pd.read_csv(csv)
    d = json.loads(js.read_text())
    for s in d["shares"]:
        grp = df[(df.prior == s["prior"]) & (df.criterion == s["criterion"])]
        compare_summary(rep, f"measure/{s['prior']}/{s['criterion']}", summary(grp.fraction), s["share"])
    # Wilson intervals, recomputed.
    for _, r in df.iterrows():
        lo, hi = wilson(int(r.admitted), int(r.n))
        rep.agree(f"measure/wilson_lo/{r.prior}/{r.criterion}/{r.seed}", lo, r.wilson_lo, 1e-3)
        rep.agree(f"measure/wilson_hi/{r.prior}/{r.criterion}/{r.seed}", hi, r.wilson_hi, 1e-3)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--out", default=str(ROOT / "out"))
    args = ap.parse_args()
    out = Path(args.out)
    rep = Report()
    verify_limits(out, rep)
    verify_detect(out, rep)
    verify_information(out, rep)
    verify_nesting(out, rep)
    verify_boot(out, rep)
    verify_measure(out, rep)
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
