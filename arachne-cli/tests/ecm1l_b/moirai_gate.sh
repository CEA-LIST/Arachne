#!/bin/bash
# EC.M1l-b outside gate (spec: cea-cdrt-knowledge/spec/items/EC.M1l-b.md).
#
#   ECM1LB_TARGET=<cargo target> arachne-cli/tests/ecm1l_b/moirai_gate.sh <arachne binary> <new-out-dir>
#
# Describes the five registered enum-keyed fixtures (fixtures/ecm1l_b/expected/identity.tsv) with
# <arachne binary>, then runs moirai_gate.rs as a moirai-semantics test in a scratch copy (git
# archive) of the Moirai lane at b1e254c, which is never committed to. Exit 0 every input described
# and the test passes, 1 otherwise, 2 the instrument did not run.
set -uo pipefail
export LC_ALL=C CARGO_INCREMENTAL=0
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../../.." && pwd)
BIN=${1:?usage: moirai_gate.sh <arachne binary> <new-out-dir>}
OUT=${2:?usage: moirai_gate.sh <arachne binary> <new-out-dir>}
TARGET=${ECM1LB_TARGET:?ECM1LB_TARGET names the cargo target}
MOIRAI=$(readlink -f "$REPO/../moirai-model-plane")
MOIRAI_AT=b1e254c
TSV=$REPO/arachne-cli/tests/fixtures/ecm1l_b/expected/identity.tsv
LOCK=/volatile/home/vp242103/.cache/cea-crdt-scratch/cargo.lock
[ -e "$OUT" ] && { echo "INSTRUMENT: $OUT exists; give a new out-dir"; exit 2; }
mkdir -p "$OUT/descriptors" "$OUT/moirai"
git -C "$MOIRAI" archive "$MOIRAI_AT" | tar -x -C "$OUT/moirai" || { echo "INSTRUMENT: no Moirai $MOIRAI_AT"; exit 2; }
cp "$HERE/moirai_gate.rs" "$OUT/moirai/moirai-semantics/tests/ecm1l_b_enum_key_descriptors.rs"

refused=0 n=0
while IFS=$'\t' read -r id _; do
    n=$((n + 1)); stem=$(basename "$id" .ecore)
    "$BIN" describe "$REPO/arachne-cli/tests/fixtures/$id" >"$OUT/descriptors/$stem.json" 2>/dev/null ||
        { rm -f "$OUT/descriptors/$stem.json"; refused=$((refused + 1)); echo "not described: $id"; }
done <"$TSV"
[ "$n" = 5 ] || { echo "INSTRUMENT: $n registered inputs, not 5"; exit 2; }

(cd "$OUT/moirai" && ECM1LB_DESCRIPTORS=$OUT/descriptors ECM1LB_IDENTITY=$TSV CARGO_TARGET_DIR=$TARGET \
    flock -w 3000 "$LOCK" nice -n 10 cargo test -j 2 --offline -p moirai-semantics \
    --test ecm1l_b_enum_key_descriptors -- --nocapture) >"$OUT/test.log" 2>&1
t=$?
tail -n 5 "$OUT/test.log"
echo "moirai gate: $refused of 5 not described, moirai-semantics test exit $t"
[ "$refused" = 0 ] && [ "$t" = 0 ] && { echo PASS; exit 0; }
echo FAIL; exit 1
