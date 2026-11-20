#!/bin/bash
# EC.M1b, O6, the corpus (spec: cea-cdrt-knowledge/spec/items/EC.M1b.md).
#
#   ECM1B_SCRATCH=<dir> arachne-cli/tests/ecm1b/run.sh <new-out-dir>
#
# Builds this worktree's `arachne` (release, a fresh target in the out-dir) and runs it with EC.01's
# driver over ModelSet and the EC.01 fixtures (30 s per file and command, 4 at a time, cap.sh's caps):
#   D  describe: every input the pinned a5f3111 build describes gives the identical stdout and identity
#      line (EC.01's pass conditions; inputs only the candidate describes are counted);
#   R  refusal: on every input a5f3111 refuses and the candidate refuses too, the first stderr line is
#      a5f3111's (ecm1a/refusal.py, judged here);
#   GA generate against EC.01's guide build e2ac279 (Léo's ebc3141 generator and CLI on our parser):
#      the same status and, when both are ok, the identical project tree, on every input; an input
#      where either side hit the deadline is counted, not judged;
#   GB generate against Léo's own ebc3141 build: EC.01's pass conditions (nothing ebc3141 generates
#      lost, no difference without a stated cause); stated-cause differences and inputs only the
#      candidate generates are counted.
# The three reference runs are cached under <scratch>/runs; delete one to rerun it.
# Exit 0 all hold, 1 one fails, 2 the instrument did not run (a pin, a build or a run failed).
set -uo pipefail
export LC_ALL=C CARGO_INCREMENTAL=0 CARGO_PROFILE_RELEASE_DEBUG=0
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../../.." && pwd)
S=${ECM1B_SCRATCH:?ECM1B_SCRATCH names the work folder}
OUT=${1:?usage: run.sh <new-out-dir>}
LOCK=/volatile/home/vp242103/.cache/cea-crdt-scratch/cargo.lock
CORPUS=/volatile/home/vp242103/datasets/modelset/unpacked/modelset/raw-data/repo-ecore-all/data
FIX=$REPO/arachne-cli/tests/fixtures/ec01
EC01="python3 -I $REPO/arachne-cli/tests/ec01/ec01.py"
W1=/volatile/home/vp242103/.cache/cea-crdt-scratch/EC.M1/bin
OURS=$S/bin/arachne-a5f3111;  OURS_SHA=fb51b053feea0c0e54e0fff408667bb7b546067160f69b7b3f28a36904bd835d
GUIDE=$W1/arachne-e2ac279;    GUIDE_SHA=f49cb61328fbffb6cd53600f363761ea285f1acb9cb66806ca68c47b6b395075
HIS=$W1/arachne-ebc3141;      HIS_SHA=7ac3685826cb148702a31a8c20cf29ee7a9e9a36c05b521e08026c9d3237a6a1
[ -e "$OUT" ] && { echo "INSTRUMENT: $OUT exists; give a new out-dir"; exit 2; }
mkdir -p "$OUT" "$S/runs"
for pin in "$OURS $OURS_SHA" "$GUIDE $GUIDE_SHA" "$HIS $HIS_SHA"; do
    set -- $pin
    [ "$(sha256sum <"$1" 2>/dev/null | cut -d' ' -f1)" = "$2" ] || { echo "INSTRUMENT: $1 missing or not sha256 $2"; exit 2; }
done
(cd "$REPO" && CARGO_TARGET_DIR=$OUT/target CARGO_BUILD_JOBS=2 flock -w 3000 "$LOCK" \
    nice -n 10 cargo build -j 2 --offline --release -p arachne-cli) >"$OUT/build.log" 2>&1 ||
    { echo "INSTRUMENT: candidate build failed, see $OUT/build.log"; exit 2; }
cp "$OUT/target/release/arachne" "$OUT/arachne-candidate" && rm -rf "$OUT/target"
sha256sum "$OURS" "$GUIDE" "$HIS" "$OUT/arachne-candidate" >"$OUT/bins.txt"

run() { # binary out-dir kind-flags...
    local bin=$1 out=$2; shift 2
    ECM1B_REAL=$bin $EC01 run --bin "$HERE/cap.sh" --out "$out" "$@" --corpus "$CORPUS" \
        --fixture-dir "$FIX/ecore" --fixture-dir "$FIX/generate" --jobs 4 --tmp-root "$S"
}
ref() { # binary name kind-flag
    local dir=$S/runs/$2
    [ -f "$dir/files.csv" ] && return 0
    run "$1" "$dir" "$3" >"$S/runs/$2.log" 2>&1 || { rm -rf "$dir"; echo "INSTRUMENT: reference run $2 failed"; exit 2; }
}
ref "$OURS" ref-a5f3111-describe --describe
ref "$GUIDE" ref-e2ac279-generate --generate
ref "$HIS" ref-ebc3141-generate --generate
run "$OUT/arachne-candidate" "$OUT/cand" --describe --generate >"$OUT/cand-run.log" 2>&1 ||
    { echo "INSTRUMENT: candidate run failed, see $OUT/cand-run.log"; exit 2; }

$EC01 compare --kind describe --left "$S/runs/ref-a5f3111-describe" --right "$OUT/cand" --out "$OUT/d" >/dev/null
d=$?
python3 -I "$REPO/arachne-cli/tests/ecm1a/refusal.py" "$OURS" "$OUT/arachne-candidate" \
    "$S/runs/ref-a5f3111-describe/files.csv" "$OUT/r" >"$OUT/r.log" 2>&1
r=$?
[ "$r" = 2 ] && { cat "$OUT/r.log"; echo "INSTRUMENT: refusal check did not run"; exit 2; }
$EC01 compare --kind generate --left "$S/runs/ref-e2ac279-generate" --right "$OUT/cand" --out "$OUT/ga" >/dev/null
$EC01 compare --kind generate --left "$S/runs/ref-ebc3141-generate" --right "$OUT/cand" --out "$OUT/gb" >/dev/null
gb=$?
python3 -I - "$OUT" <<'PY' || { echo "INSTRUMENT: a comparison did not run"; exit 2; }
import collections, csv, json, sys
out = sys.argv[1]
for part in ("d", "ga", "gb"):
    kind = "describe" if part == "d" else "generate"
    rows = list(csv.DictReader(open(f"{out}/{part}/compare-{kind}.csv")))
    by = collections.Counter((r["id"].split("/", 1)[0], r["class"].split("(")[0]) for r in rows)
    print(part, json.dumps({f"{s} {c}": n for (s, c), n in sorted(by.items())}))
rows = list(csv.DictReader(open(f"{out}/ga/compare-generate.csv")))
late = [r for r in rows if "timeout" in (r["left_status"], r["right_status"])]
bad = [r for r in rows if r not in late and r["class"] not in ("identical", "both-refused")]
open(f"{out}/ga-failures.txt", "w").write("".join(f"{r['id']} {r['class']}\n" for r in bad))
open(f"{out}/ga-deadline.txt", "w").write("".join(f"{r['id']} {r['class']}\n" for r in late))
print(f"GA: {len(bad)} inputs where the candidate's generate differs from e2ac279's; {len(late)} at the deadline (counted)")
PY
ga=$(wc -l <"$OUT/ga-failures.txt")
tail -n 1 "$OUT/r.log"
echo "D exit $d, R exit $r, GA failures $ga, GB exit $gb"
[ "$d" = 0 ] && [ "$r" = 0 ] && [ "$ga" = 0 ] && [ "$gb" = 0 ] && { echo PASS; exit 0; }
echo FAIL; exit 1
