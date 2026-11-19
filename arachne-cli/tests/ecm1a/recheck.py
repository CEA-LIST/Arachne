#!/usr/bin/env python3
"""EC.M1 piece 1, O3's generate recheck (spec: cea-cdrt-knowledge/spec/items/EC.M1.md, section 3).

   recheck.py <out-dir> <reference-binary> <candidate-binary> <reference-run-dir>

Every generate row of <out-dir>/compare-generate.csv that would fail (differ*, left-only*,
missing*) is run again, one process at a time, RERUNS times with each binary through
as-ec01.sh. f90795e's generator is not deterministic on some inputs (where it boxes a
recursive field), so a census difference alone does not show a change:
  reference-unstable  the reference's reruns do not all give `ok` with one tree, equal to its
                      census tree: reported, not judged (this depends on the reference alone);
  confirmed           the reference is stable and no candidate rerun gives `ok` with that same
                      tree: a failure;
  noise               the reference is stable and at least one candidate rerun gives its tree
                      (the candidate can produce it; the census difference was run to run).
Writes generate-failures.txt (confirmed only) and generate-recheck.csv; exit 0.
"""

import csv
import os
import shutil
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "ec01"))
from ec01 import run_bin, tree_hash  # noqa: E402

RERUNS = 5
WRAP = os.path.join(HERE, "as-ec01.sh")


def gen_once(real, path, tmp_root):
    tmp = tempfile.mkdtemp(prefix="recheck-", dir=tmp_root)
    try:
        os.environ["ECM1A_REAL"] = real
        status, _, _ = run_bin(["sh", WRAP, "generate", "-o", os.path.join(tmp, "p"),
                                "-p", "ec01probe", path], 30)
        return (status, tree_hash(os.path.join(tmp, "p")) if status == "ok" else "")
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


def main():
    out, ref, cand, ref_run = sys.argv[1:5]
    runs = [{r["id"]: r for r in csv.DictReader(open(os.path.join(d, "files.csv")))}
            for d in (ref_run, os.path.join(out, "cand"))]
    rows = [r for r in csv.DictReader(open(os.path.join(out, "compare-generate.csv")))
            if r["class"].startswith(("differ", "left-only", "missing"))]
    result = []
    for r in rows:
        fid = r["id"]
        path = (runs[0].get(fid) or runs[1].get(fid))["path"]
        census_ref = (runs[0].get(fid) or {}).get("generate_tree", "")
        refs = [gen_once(ref, path, out) for _ in range(RERUNS)]
        cands = [gen_once(cand, path, out) for _ in range(RERUNS)]
        ref_trees = {t for s, t in refs if s == "ok"}
        stable = (all(s == "ok" for s, _ in refs) and len(ref_trees) == 1
                  and (not census_ref or census_ref in ref_trees))
        if not stable:
            verdict = "reference-unstable"
        elif refs[0] in cands:
            verdict = "noise"
        else:
            verdict = "confirmed"
        result.append({"id": fid, "census_class": r["class"], "verdict": verdict,
                       "reference_reruns": " ".join(s + ":" + t[:12] for s, t in refs),
                       "candidate_reruns": " ".join(s + ":" + t[:12] for s, t in cands)})
    with open(os.path.join(out, "generate-recheck.csv"), "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=["id", "census_class", "verdict",
                                          "reference_reruns", "candidate_reruns"])
        w.writeheader()
        w.writerows(result)
    with open(os.path.join(out, "generate-failures.txt"), "w") as f:
        f.write("".join(f"{x['id']} {x['census_class']}\n" for x in result
                        if x["verdict"] == "confirmed"))
    for v in ("confirmed", "noise", "reference-unstable"):
        print(f"generate recheck {v}: {sum(1 for x in result if x['verdict'] == v)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
