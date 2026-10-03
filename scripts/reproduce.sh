#!/usr/bin/env sh
# Recreate every published table from source.
#
# Runs each documented command with its shipped config and writes the
# artifacts under out/, each directory with a metadata.json naming the commit,
# toolchain, target, config and seeds that produced it. The README's numbers
# are read from these files and nowhere else; analysis/ledger.py checks the
# claims ledger against them.
#
# Reproducible counters (work, divergences, bits, counts) are identical on any
# machine. Wall-clock columns are not and are never asserted on.
#
#   scripts/reproduce.sh          # everything; about half an hour on four cores
#   scripts/reproduce.sh quick    # the cheap commands only, for a smoke test
#
# The measure command is the slow one (every band law and a life-like sample
# at twenty seeds, then the band family in six more universes at three).

set -eu
cd "$(dirname "$0")/.."

echo "source: $(git rev-parse HEAD 2>/dev/null || echo unknown)$(git diff --quiet 2>/dev/null || echo ' (dirty)')"
echo "toolchain: $(rustc --version)"
echo

cargo build --release

run() {
  echo "== $* =="
  cargo run --release -q -- "$@"
  echo
}

run run    --config configs/default.toml
run nest   --config configs/nesting.toml
run pipe   --config configs/pipe.toml
run detect --config configs/detect.toml
run boot   --config configs/boot.toml
run boot   --config configs/boot-permissive.toml

if [ "${1:-}" = "quick" ]; then
  echo "quick: skipping sweep, edge and measure"
  exit 0
fi

run sweep  --config configs/sweep.toml --steps 21
run edge   --config configs/edge.toml
run measure --config configs/measure.toml

echo "done. check the ledger against the artifacts with: python3 analysis/ledger.py"
