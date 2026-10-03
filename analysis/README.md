# analysis

Three readers of the artifacts the `cargo` commands write under `out/`. None of
them establishes anything: every claim this project makes is produced by a
command in the repository and recorded with the artifact it came from. These
check that the record is consistent, recompute its arithmetic in another
language, and draw it.

## `claims.toml` and `ledger.py`

`claims.toml` is the machine-readable list of every claim, each with an id, the
exact text, a category (`definition`, `consequence`, `computational_finding`,
`physical_hypothesis`), the module and experiment it rests on, the metric, the
seeds, the controls, a status, the commit and experiment design version it was
last verified at, and — for findings — the artifact file, path and range that
must hold.

`ledger.py` checks the ledger's structure, that the README names every claim
and claims nothing the ledger does not hold, that each finding's artifacts
carry the commit and design version the ledger records, and that every range
holds on the artifacts. It uses only the standard library, so CI runs it
without a virtual environment:

```sh
python3 analysis/ledger.py                 # everything, against out/
python3 analysis/ledger.py --no-artifacts  # structure and README only
python3 analysis/ledger.py --ignore-commit # CI: fresh artifacts at HEAD, check the ranges
python3 analysis/ledger.py --stamp         # after scripts/reproduce.sh at a clean commit:
                                           # record that commit in every claim
```

## `verify.py`

A second opinion on the statistics. Every summary the CLI prints — means,
standard deviations, t intervals, Wilson intervals, counts of seeds, the
detection rules' thresholds and outcomes — is recomputed with pandas and scipy
from the per-seed CSVs and compared to the JSON the CLI wrote. If the two
disagree, one of two implementations in two languages is wrong, and the
disagreement is the finding.

## `figures.py`

Draws the findings from the artifacts into `analysis/figures/`, computing
nothing the artifacts do not already hold.

## Setup and use

```sh
python3 -m venv analysis/.venv
analysis/.venv/bin/pip install -r analysis/requirements.txt

scripts/reproduce.sh                        # regenerate every artifact
python3 analysis/ledger.py                  # the ledger against the artifacts
analysis/.venv/bin/python analysis/verify.py
analysis/.venv/bin/python analysis/figures.py
```

All three take `--out DIR` if the artifacts are somewhere other than `out/`.

## What none of this checks

Wall time. It is a measurement rather than a counter, and the machine that
wrote the artifacts is not the machine reading them.
