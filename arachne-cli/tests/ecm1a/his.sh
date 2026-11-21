#!/bin/bash
# EC.M1 piece 1 round 2, O4 (spec: cea-cdrt-knowledge/spec/items/EC.M1.md, section 11).
#
#   ECM1_SCRATCH=<dir> arachne-cli/tests/ecm1a/his.sh <out-dir>
#
# Checks Léo's pinned ebc3141 build, builds this worktree's `arachne` (release) in its own target, and
# one release build per rules-off/<rule>.patch and per pair of them, from `git archive HEAD` of this
# worktree with the patches applied (EC.01 rules off, the parser otherwise the candidate's), then
# runs his.py over the census list. Exit as his.py: 0 all hold, 1 one fails, 2 the instrument did not run.
set -uo pipefail
export LC_ALL=C CARGO_INCREMENTAL=0 CARGO_PROFILE_RELEASE_DEBUG=0
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../../.." && pwd)
S=${ECM1_SCRATCH:?ECM1_SCRATCH names the work folder}
OUT=${1:?usage: his.sh <out-dir>}
# Fresh targets inside OUT for every run: the shared EC.M2 target gave the candidate build a stale
# ecore_rs (2026-11-20), and cargo can take another clone's artifacts from a shared target.
TARGET=$OUT/target
LOCK=/volatile/home/vp242103/.cache/cea-crdt-scratch/cargo.lock
HIS=$S/bin/arachne-ebc3141
HIS_SHA=7ac3685826cb148702a31a8c20cf29ee7a9e9a36c05b521e08026c9d3237a6a1
FILES=$S/runs/ref-f90795e/files.csv
[ -e "$OUT" ] && { echo "INSTRUMENT: $OUT exists; give a new out-dir"; exit 2; }
mkdir -p "$OUT"

[ "$(sha256sum <"$HIS" 2>/dev/null | cut -d' ' -f1)" = "$HIS_SHA" ] ||
    { echo "INSTRUMENT: $HIS missing or not sha256 $HIS_SHA (ebc3141's release build)"; exit 2; }
[ -f "$FILES" ] || { echo "INSTRUMENT: $FILES missing (run.sh's reference census list)"; exit 2; }
build() { # source-dir target-dir log
    (cd "$1" && CARGO_TARGET_DIR=$2 CARGO_BUILD_JOBS=2 flock -w 3000 "$LOCK" \
        nice -n 10 cargo build -j 2 --offline --release -p arachne-cli) >"$3" 2>&1
}
build "$REPO" "$TARGET" "$OUT/build.log" || { echo "INSTRUMENT: candidate build failed, see $OUT/build.log"; exit 2; }
cp "$TARGET/release/arachne" "$OUT/arachne-candidate"

# One source copy and one target for the rule-off builds; tar -m gives fresh mtimes, so cargo
# rebuilds what each patch and each restore touches.
OFF=$OUT/rules-off
rm -rf "$OFF/arachne-model-plane"; mkdir -p "$OFF/arachne-model-plane"
ln -sfn "$(readlink -f "$REPO/../moirai-model-plane")" "$OFF/moirai-model-plane"
git -C "$REPO" archive HEAD | tar -x -m -C "$OFF/arachne-model-plane"
# Each rule alone, then each pair (a difference two rules make together, section 11).
rules=$(cd "$HERE/rules-off" && ls *.patch | sed 's/\.patch$//')
sets=$(for a in $rules; do echo "$a"; done; for a in $rules; do for b in $rules; do [[ $a < $b ]] && echo "$a+$b"; done; done)
args=()
for set in $sets; do
    git -C "$REPO" archive HEAD arachne-parser/src | tar -x -m -C "$OFF/arachne-model-plane"
    for rule in ${set//+/ }; do
        patch -s -p1 -d "$OFF/arachne-model-plane" <"$HERE/rules-off/$rule.patch" ||
            { echo "INSTRUMENT: $rule.patch does not apply to HEAD's parser (set $set)"; exit 2; }
    done
    build "$OFF/arachne-model-plane" "$OUT/rules-off-target" "$OUT/build-$set.log" ||
        { echo "INSTRUMENT: $set build failed, see $OUT/build-$set.log"; exit 2; }
    cp "$OUT/rules-off-target/release/arachne" "$OUT/arachne-off-$set"
    args+=("$set=$OUT/arachne-off-$set")
done
sha256sum "$HIS" "$OUT"/arachne-* >"$OUT/bins.txt"
python3 -I "$HERE/his.py" "$HIS" "$OUT/arachne-candidate" "$FILES" "$OUT" "${args[@]}"
