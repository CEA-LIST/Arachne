#!/bin/bash
# EC.M1 piece 1, O3, the corpus census (spec: cea-cdrt-knowledge/spec/items/EC.M1.md).
#
#   ECM1_SCRATCH=<dir> arachne-cli/tests/ecm1a/run.sh <out-dir>
#
# Builds this worktree's `arachne` (release) as it stands, then runs it and the pinned f90795e
# build over ModelSet and the EC.01 fixtures with EC.01's driver, 30 s per file and command:
#   describe: every input f90795e describes gives the identical stdout and identity line;
#   generate: every input f90795e generates gives the identical project tree (any difference
#             fails, EC.01's stated causes included: our generator is meant to be unchanged),
#             after recheck.py's reruns, since f90795e's generator varies run to run on some inputs.
# Inputs only the candidate accepts are reported. Exit 0 both hold, 1 one fails, 2 the
# instrument could not run (reference missing, build failed): not a pass.
set -uo pipefail
export LC_ALL=C CARGO_INCREMENTAL=0 CARGO_PROFILE_RELEASE_DEBUG=0
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../../.." && pwd)
S=${ECM1_SCRATCH:?ECM1_SCRATCH names the work folder}
OUT=${1:?usage: run.sh <out-dir>}
TARGET=${ECM1_TARGET:-/volatile/home/vp242103/.cache/cea-crdt-scratch/EC.M2/target}
LOCK=/volatile/home/vp242103/.cache/cea-crdt-scratch/cargo.lock
CORPUS=${CORPUS:-/volatile/home/vp242103/datasets/modelset/unpacked/modelset/raw-data/repo-ecore-all/data}
REF=$S/bin/arachne-f90795e
REF_SHA=7575d47f1d652cae6ee12eba22a43e210cfb1a86d2a6f594bd8a81639c7fe83c
FIX=$REPO/arachne-cli/tests/fixtures/ec01
PY="python3 -I $REPO/arachne-cli/tests/ec01/ec01.py"
mkdir -p "$OUT" "$S/runs" "$S/moirai-stub"
export ECM1A_STUB=$S/moirai-stub

[ "$(sha256sum <"$REF" 2>/dev/null | cut -d' ' -f1)" = "$REF_SHA" ] ||
    { echo "INSTRUMENT: $REF missing or not sha256 $REF_SHA (EC.01's f90795e build)"; exit 2; }
(cd "$REPO" && CARGO_TARGET_DIR=$TARGET CARGO_BUILD_JOBS=2 flock -w 3000 "$LOCK" \
    nice -n 10 cargo build -j 2 --offline --release -p arachne-cli) >"$OUT/build.log" 2>&1 ||
    { echo "INSTRUMENT: candidate build failed, see $OUT/build.log"; exit 2; }
cp "$TARGET/release/arachne" "$OUT/arachne-candidate"
sha256sum "$REF" "$OUT/arachne-candidate" >"$OUT/bins.txt"

run() { # real-binary out-dir
    ECM1A_REAL=$1 $PY run --bin "$HERE/as-ec01.sh" --out "$2" --describe --generate \
        --corpus "$CORPUS" --fixture-dir "$FIX/ecore" --fixture-dir "$FIX/generate" --jobs 4 --tmp-root "$S"
}
[ -f "$S/runs/ref-f90795e/files.csv" ] || run "$REF" "$S/runs/ref-f90795e" >"$OUT/ref-run.log" 2>&1 ||
    { rm -rf "$S/runs/ref-f90795e"; echo "INSTRUMENT: reference run failed"; exit 2; }
run "$OUT/arachne-candidate" "$OUT/cand" >"$OUT/cand-run.log" 2>&1 ||
    { echo "INSTRUMENT: candidate run failed, see $OUT/cand-run.log"; exit 2; }

$PY compare --kind describe --left "$S/runs/ref-f90795e" --right "$OUT/cand" --out "$OUT" >/dev/null
d=$?
$PY compare --kind generate --left "$S/runs/ref-f90795e" --right "$OUT/cand" --out "$OUT" >/dev/null
python3 -I - "$OUT" <<'EOF'
import collections, csv, json, sys
out = sys.argv[1]
for kind in ("describe", "generate"):
    rows = list(csv.DictReader(open(f"{out}/compare-{kind}.csv")))
    by = collections.Counter((r["id"].split("/", 1)[0], r["class"].split("(")[0]) for r in rows)
    print(kind, json.dumps({f"{s} {c}": n for (s, c), n in sorted(by.items())}))
    bad = [r for r in rows if r["class"].startswith(("differ", "left-only", "missing"))]
    open(f"{out}/{kind}-failures.txt", "w").write("".join(f"{r['id']} {r['class']}\n" for r in bad))
    print(f"{kind}: {len(bad)} inputs f90795e accepts that differ or are lost")
    if kind == "describe":  # reported, not judged: the refusal sentence the model editor's eval reads
        moved = [r["id"] for r in rows if r["class"] == "both-refused" and r["left_detail"] != r["right_detail"]]
        open(f"{out}/describe-refusal-text-changed.txt", "w").write("".join(i + "\n" for i in moved))
        print(f"describe: {len(moved)} inputs both refuse with another first error line (reported)")
EOF
# A generate difference counts only when f90795e is stable on that input and M never gives its tree.
python3 -I "$HERE/recheck.py" "$OUT" "$REF" "$OUT/arachne-candidate" "$S/runs/ref-f90795e" ||
    { echo "INSTRUMENT: recheck failed"; exit 2; }
g=$(wc -l <"$OUT/generate-failures.txt")
echo "describe parity exit $d (EC.01's pass conditions), generate failures $g"
[ "$d" = 0 ] && [ "$g" = 0 ] && { echo PASS; exit 0; }
echo FAIL; exit 1
