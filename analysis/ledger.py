#!/usr/bin/env python3
"""Check the claims ledger against itself and against the artifacts.

`analysis/claims.toml` is the machine-readable list of every claim this
repository makes. This script checks:

1. **Structure.** Every claim has an id, text, category, status, and the
   fields its category requires. Categories are `definition`, `consequence`,
   `computational_finding` and `physical_hypothesis`. A computational finding
   must name its experiment, metric, seeds and controls; a physical hypothesis
   must have status `untested` and no experiment, because nothing here tests
   one.
2. **Cross-references.** Every claim id appears in README.md, so no claim is
   made that the README does not show, and the README's "claims" table lists
   nothing the ledger does not hold.
3. **Provenance.** Every computational finding's `last_verified_commit` matches
   the commit recorded in the `metadata.json` of its experiment's output
   directory, and the experiment design version it was verified under matches
   the one the artifact records. A finding verified under an older design is
   reported as stale.
4. **Checks.** A finding may carry a `check` table naming an artifact file,
   a JSON path or CSV column, and an expected range. The script reads the
   artifact and confirms the value lies in the range. Ranges are deliberately
   loose where the README says a number is an example; they exist to catch
   drift, not to pin a decimal.

Usage:

    python3 analysis/ledger.py             # check everything under out/
    python3 analysis/ledger.py --out DIR   # artifacts elsewhere
    python3 analysis/ledger.py --no-artifacts   # structure and README only
    python3 analysis/ledger.py --ignore-commit  # CI: fresh artifacts at HEAD, check the ranges

Exit status is non-zero on any failure. Only the standard library is used, so
the check can run in CI without a virtual environment.
"""

from __future__ import annotations

import argparse
import csv
import json
import re
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CATEGORIES = {"definition", "consequence", "computational_finding", "physical_hypothesis"}
STATUSES = {"verified", "stale", "exploratory", "untested", "withdrawn"}


class Check:
    def __init__(self) -> None:
        self.failures: list[str] = []
        self.warnings: list[str] = []
        self.passed = 0

    def fail(self, msg: str) -> None:
        self.failures.append(msg)

    def warn(self, msg: str) -> None:
        self.warnings.append(msg)

    def ok(self) -> None:
        self.passed += 1

    def finish(self) -> int:
        for w in self.warnings:
            print(f"warning: {w}")
        if self.failures:
            print(f"\n{len(self.failures)} failure(s):")
            for f in self.failures:
                print(f"  {f}")
            return 1
        print(f"\n{self.passed} checks passed; the ledger is consistent with the README"
              + (" and the artifacts." if self.passed else "."))
        return 0


def load_ledger() -> list[dict]:
    with open(ROOT / "analysis" / "claims.toml", "rb") as f:
        data = tomllib.load(f)
    return data.get("claim", [])


def check_structure(claims: list[dict], chk: Check) -> None:
    seen: set[str] = set()
    for c in claims:
        cid = c.get("id", "<missing id>")
        if cid in seen:
            chk.fail(f"{cid}: duplicate id")
        seen.add(cid)
        for field in ("id", "text", "category", "status", "module"):
            if field not in c:
                chk.fail(f"{cid}: missing field `{field}`")
        cat = c.get("category")
        if cat not in CATEGORIES:
            chk.fail(f"{cid}: category `{cat}` is not one of {sorted(CATEGORIES)}")
        if c.get("status") not in STATUSES:
            chk.fail(f"{cid}: status `{c.get('status')}` is not one of {sorted(STATUSES)}")
        if cat == "computational_finding":
            for field in ("experiment", "metric", "seeds", "controls", "last_verified_commit"):
                if field not in c:
                    chk.fail(f"{cid}: a computational finding needs `{field}`")
            if c.get("status") not in {"verified", "stale", "exploratory", "withdrawn"}:
                chk.fail(f"{cid}: a computational finding cannot be `{c.get('status')}`")
        if cat == "physical_hypothesis":
            if c.get("status") != "untested":
                chk.fail(f"{cid}: a physical hypothesis must be `untested`; nothing here tests one")
            if "experiment" in c:
                chk.fail(f"{cid}: a physical hypothesis must not name an experiment")
        if cat in {"definition", "consequence"} and "derivation" not in c and "source" not in c:
            chk.warn(f"{cid}: a {cat} should point at its derivation or source")
        chk.ok()


def check_readme(claims: list[dict], chk: Check) -> None:
    readme = (ROOT / "README.md").read_text(encoding="utf-8")
    ids = {c["id"] for c in claims if "id" in c}
    for cid in sorted(ids):
        if cid not in readme:
            chk.fail(f"{cid}: not mentioned in README.md")
        else:
            chk.ok()
    # Anything that looks like a claim id in the README must be in the ledger.
    for m in sorted(set(re.findall(r"\b((?:T\d|DET|GEN)-(?:DEF|CON|FND|HYP)-\d{3})\b", readme))):
        if m not in ids:
            chk.fail(f"README.md mentions {m}, which the ledger does not hold")


def read_path(obj, path: str):
    """Walk a JSON value by dotted path.

    Each part is `key`, `key[sel]` or `[sel]` (a selector on the current value,
    for a list-rooted document). A selector is an index, or one or more
    `key=value` conditions separated by commas that pick the first element of a
    list matching all of them.
    """
    for part in path.split("."):
        m = re.match(r"^([^\[]*)(\[.*\])?$", part)
        if not m:
            raise KeyError(path)
        key, sel = m.group(1), m.group(2)
        if key:
            if not isinstance(obj, dict):
                raise KeyError(f"{key} in {path}")
            obj = obj[key]
        if sel:
            for s in re.findall(r"\[([^\]]+)\]", sel):
                if "=" in s:
                    conds = [c.split("=", 1) for c in s.split(",")]
                    matches = [o for o in obj if all(str(o.get(k)) == v for k, v in conds)]
                    if not matches:
                        raise KeyError(f"no element matching {s} in {path}")
                    obj = matches[0]
                else:
                    obj = obj[int(s)]
    return obj


def artifact_value(out: Path, check: dict):
    path = out / check["file"]
    if not path.exists():
        raise FileNotFoundError(path)
    if path.suffix == ".json":
        with open(path, encoding="utf-8") as f:
            data = json.load(f)
        return read_path(data, check["path"])
    if path.suffix == ".csv":
        with open(path, encoding="utf-8", newline="") as f:
            rows = list(csv.DictReader(f))
        where = check.get("where", {})
        rows = [r for r in rows if all(str(r.get(k)) == str(v) for k, v in where.items())]
        if not rows:
            raise KeyError(f"no row matching {where} in {path}")
        col = check["column"]
        agg = check.get("aggregate", "first")
        if agg == "count":
            return len(rows)
        if agg == "count_true":
            return sum(1 for r in rows if r[col] == "true")

        def parse(v: str):
            try:
                return float(v)
            except ValueError:
                return v

        vals = [parse(r[col]) for r in rows]
        if agg == "first":
            return vals[0]
        nums = [v for v in vals if isinstance(v, float)]
        if agg == "mean":
            return sum(nums) / len(nums)
        if agg == "min":
            return min(nums)
        if agg == "max":
            return max(nums)
        raise ValueError(f"unknown aggregate {agg}")
    raise ValueError(f"cannot read {path.suffix}")


def check_artifacts(claims: list[dict], out: Path, chk: Check, ignore_commit: bool) -> None:
    for c in claims:
        if c.get("category") != "computational_finding":
            continue
        cid = c["id"]
        if c.get("verified_by") == "tests":
            # Established by the test suite rather than an artifact (the
            # cross-target fingerprint); CI's `cargo test` and `wasm-pack test`
            # are its check.
            chk.ok()
            continue
        exp_dir = out / c.get("artifacts", "")
        meta_path = exp_dir / "metadata.json"
        if not meta_path.exists():
            chk.fail(f"{cid}: no metadata.json under {exp_dir}; run the experiment first")
            continue
        with open(meta_path, encoding="utf-8") as f:
            meta = json.load(f)
        if not ignore_commit and c.get("last_verified_commit") != meta.get("commit"):
            if c.get("status") == "verified":
                chk.fail(
                    f"{cid}: verified at {str(c.get('last_verified_commit'))[:12]} but the artifact is from "
                    f"{str(meta.get('commit'))[:12]}; re-run and update the ledger, or mark it stale"
                )
            else:
                chk.warn(f"{cid}: artifact commit differs from the ledger's ({c.get('status')})")
        if c.get("design_version") != meta.get("experiment_version"):
            chk.fail(
                f"{cid}: verified under design v{c.get('design_version')} but the artifact records "
                f"v{meta.get('experiment_version')}"
            )
        for check in c.get("check", []):
            try:
                value = artifact_value(out, check)
            except (FileNotFoundError, KeyError, ValueError) as e:
                chk.fail(f"{cid}: check could not be read: {e}")
                continue
            lo, hi = check.get("min"), check.get("max")
            expect = check.get("equals")
            contains = check.get("contains")
            if contains is not None:
                if not isinstance(value, list) or contains not in value:
                    chk.fail(f"{cid}: {check['file']} {check.get('path')} does not contain {contains!r}: {value}")
                    continue
                chk.ok()
                continue
            if expect is not None:
                if str(value) != str(expect) and not (
                    isinstance(value, float) and abs(value - float(expect)) < 1e-9
                ):
                    chk.fail(f"{cid}: {check['file']} {check.get('path', check.get('column'))} = {value}, expected {expect}")
                    continue
            else:
                if value is None or not isinstance(value, (int, float)):
                    chk.fail(f"{cid}: {check['file']} gave non-numeric {value!r}")
                    continue
                if lo is not None and value < lo:
                    chk.fail(f"{cid}: {check['file']} {check.get('path', check.get('column'))} = {value:.4g} below {lo}")
                    continue
                if hi is not None and value > hi:
                    chk.fail(f"{cid}: {check['file']} {check.get('path', check.get('column'))} = {value:.4g} above {hi}")
                    continue
            chk.ok()


def stamp(out: Path) -> int:
    """Record the commit and design version each claim's artifacts came from.

    Run after `scripts/reproduce.sh` at a clean commit. Every claim with an
    `artifacts` directory gets that directory's metadata commit and
    experiment version written into its `last_verified_commit` and
    `design_version` lines. Refuses if any artifact was built from a dirty tree
    or if the artifacts disagree about the commit: a ledger must point at one
    revision.
    """
    path = ROOT / "analysis" / "claims.toml"
    text = path.read_text(encoding="utf-8")
    claims = load_ledger()
    commits: set[str] = set()
    metas: dict[str, dict] = {}
    for c in claims:
        art = c.get("artifacts")
        if art is None or c.get("verified_by") == "tests":
            continue
        meta_path = out / art / "metadata.json"
        if not meta_path.exists():
            print(f"{c['id']}: no metadata.json under {out / art}")
            return 1
        meta = json.loads(meta_path.read_text(encoding="utf-8"))
        if meta.get("dirty") is not False:
            print(f"{c['id']}: {meta_path} was built from a dirty tree; commit first")
            return 1
        commits.add(meta["commit"])
        metas[c["id"]] = meta
    if len(commits) > 1:
        print(f"artifacts come from several commits: {sorted(commits)}; reproduce everything at one")
        return 1
    # Rewrite within each claim block, so claims keep their own values.
    blocks = re.split(r"(?=^\[\[claim\]\])", text, flags=re.M)
    out_blocks = []
    for b in blocks:
        m = re.search(r'^id\s*=\s*"([^"]+)"', b, flags=re.M)
        if m and m.group(1) in metas:
            meta = metas[m.group(1)]
            b = re.sub(r'^last_verified_commit\s*=\s*"[^"]*"', f'last_verified_commit = "{meta["commit"]}"', b, flags=re.M)
            b = re.sub(r"^design_version\s*=\s*\d+", f"design_version = {meta['experiment_version']}", b, flags=re.M)
        out_blocks.append(b)
    path.write_text("".join(out_blocks), encoding="utf-8")
    print(f"stamped {len(metas)} claims at {next(iter(commits))[:12] if commits else 'n/a'}")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--out", default=str(ROOT / "out"), help="artifact directory (default: out/)")
    ap.add_argument("--no-artifacts", action="store_true", help="check structure and README only")
    ap.add_argument(
        "--stamp",
        action="store_true",
        help="rewrite every claim's last_verified_commit and design_version from the artifacts' "
        "metadata.json, after a full reproduction at one commit",
    )
    ap.add_argument(
        "--ignore-commit",
        action="store_true",
        help="do not require the artifacts' commit to match the ledger's (for CI, which regenerates "
        "the artifacts at HEAD and checks the ranges)",
    )
    args = ap.parse_args()

    if args.stamp:
        return stamp(Path(args.out))

    claims = load_ledger()
    chk = Check()
    check_structure(claims, chk)
    check_readme(claims, chk)
    if not args.no_artifacts:
        check_artifacts(claims, Path(args.out), chk, args.ignore_commit)
    by_cat: dict[str, int] = {}
    for c in claims:
        by_cat[c.get("category", "?")] = by_cat.get(c.get("category", "?"), 0) + 1
    print(f"{len(claims)} claims: " + ", ".join(f"{k} {v}" for k, v in sorted(by_cat.items())))
    return chk.finish()


if __name__ == "__main__":
    sys.exit(main())
