#!/bin/bash
# EC.M1j outside gate (spec: cea-cdrt-knowledge/spec/items/EC.M1j.md).
#
#   ECM1J_TARGET=<cargo target> arachne-cli/tests/ecm1j/moirai_gate.sh <arachne binary> <new-out-dir>
#
# Describes the three EC.M1j golden fixtures and the 141 inputs of changed.txt and described.txt
# with <arachne binary>, then runs moirai_gate.rs as a moirai-semantics test in a scratch copy
# (git archive) of the Moirai lane at aeed0bd, which is never committed to. Exit 0 every input
# described and the test passes, 1 otherwise, 2 the instrument did not run.
set -uo pipefail
export LC_ALL=C CARGO_INCREMENTAL=0
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../../.." && pwd)
BIN=${1:?usage: moirai_gate.sh <arachne binary> <new-out-dir>}
OUT=${2:?usage: moirai_gate.sh <arachne binary> <new-out-dir>}
TARGET=${ECM1J_TARGET:?ECM1J_TARGET names the cargo target}
MOIRAI=$(readlink -f "$REPO/../moirai-model-plane")
MOIRAI_AT=aeed0bd
CORPUS=/volatile/home/vp242103/datasets/modelset/unpacked/modelset/raw-data/repo-ecore-all/data
LOCK=/volatile/home/vp242103/.cache/cea-crdt-scratch/cargo.lock
[ -e "$OUT" ] && { echo "INSTRUMENT: $OUT exists; give a new out-dir"; exit 2; }
mkdir -p "$OUT/descriptors" "$OUT/moirai"
git -C "$MOIRAI" archive "$MOIRAI_AT" | tar -x -C "$OUT/moirai" || { echo "INSTRUMENT: no Moirai $MOIRAI_AT"; exit 2; }
cp "$HERE/moirai_gate.rs" "$OUT/moirai/moirai-semantics/tests/ecm1j_subpackage_descriptors.rs"

refused=0
one() { # name path
    if "$BIN" describe "$2" >"$OUT/descriptors/$1.json" 2>"$OUT/err.txt"; then
        tail -n 1 "$OUT/err.txt" | sed 's/\x1b\[[0-9;]*m//g' >"$OUT/descriptors/$1.id"
    else
        rm -f "$OUT/descriptors/$1.json"; refused=$((refused + 1)); echo "not described: $2"
    fi
}
F=$REPO/arachne-cli/tests/fixtures/ecm1j
one tree "$F/tree.ecore"; one classless_root "$F/classless_root.ecore"; one deptree "$F/import/deptree.ecore"
n=0
while read -r id; do
    n=$((n + 1)); one "c$n" "$CORPUS/${id#modelset/}"
done < <(cat "$HERE/changed.txt" "$HERE/described.txt")
[ "$n" = 141 ] || { echo "INSTRUMENT: $n registered inputs, not 141"; exit 2; }

(cd "$OUT/moirai" && ECM1J_DESCRIPTORS=$OUT/descriptors CARGO_TARGET_DIR=$TARGET flock -w 3000 "$LOCK" \
    nice -n 10 cargo test -j 2 --offline -p moirai-semantics --test ecm1j_subpackage_descriptors -- --nocapture) \
    >"$OUT/test.log" 2>&1
t=$?
tail -n 5 "$OUT/test.log"
echo "moirai gate: $refused of 144 not described, moirai-semantics test exit $t"
[ "$refused" = 0 ] && [ "$t" = 0 ] && { echo PASS; exit 0; }
echo FAIL; exit 1
