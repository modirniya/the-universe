# analysis

A second opinion on the numbers, and pictures of them.

Everything here reads artifacts that `cargo` commands wrote. Nothing here
establishes anything. The project's rule is that every claim is reproducible by
one command in the repo, and these are not those commands — if this layer and
the CLI ever disagree, the CLI is right and this layer has a bug.

That is why it is kept out of CI. Adding it as a gate would quietly make a
Python script load-bearing for claims that are supposed to rest on `cargo`
alone.

## What it is for

`verify.py` reads the same files CI reads and checks the README against them
with different code in a different language. The README quotes numbers; the
artifacts contain them; drift between the two is invisible until something
compares them. It also re-checks the structural invariants — budgets strictly
decreasing, no layer outspending its host, no correlation reported from too few
events — so a second reader confirms them independently.

`figures.py` draws the findings. The fine-tuning sweep in particular is easier
to read as an image than as the ASCII map the CLI prints, though the CLI keeps
printing the ASCII because a terminal is where the claim is made.

## Setup

```sh
python3 -m venv analysis/.venv
analysis/.venv/bin/pip install -r analysis/requirements.txt
```

## Use

Generate the artifacts, then read them:

```sh
cargo run --release -- run    --config configs/default.toml
cargo run --release -- nest   --config configs/nesting.toml
cargo run --release -- pipe   --config configs/pipe.toml
cargo run --release -- detect --config configs/detect.toml
cargo run --release -- sweep  --config configs/sweep.toml --steps 21
cargo run --release -- boot   --config configs/boot.toml

analysis/.venv/bin/python analysis/verify.py
analysis/.venv/bin/python analysis/figures.py
```

`verify.py` exits non-zero if the README and the artifacts disagree. Both take
`--out DIR` if your artifacts are somewhere other than `out/`.

## What it does not check

Wall time. It is a measurement rather than a counter, and the machine that wrote
the README is not the machine reading it. The README says so too; this layer
just declines to pretend otherwise.
