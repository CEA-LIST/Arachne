"""EC.M1n, D's judgment (spec: cea-cdrt-knowledge/spec/items/EC.M1n.md).

    python3 -I arachne-cli/tests/ecm1n/census.py <run.sh out-dir> <base files.csv>

The base is ecm1b/run.sh's candidate run at 643ff52 (its cand/files.csv, pinned by sha256 in
run.sh). Judged:
  M1 moved.tsv (the inputs the base describes with an uncontained containment component beside its
     first-pass roots): the candidate describes the input, its `rootClasses` and its class keys are
     the registered lists;
  M3 every other input: describe status, stdout sha256, identity line and stderr detail equal to
     the base's.
Writes census.csv and census-failures.txt in <out-dir>/d. Exit 0 all hold, 1 one fails, 2 the
instrument did not run.
"""
import csv
import json
import os
import resource
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
MOVED = 62
csv.field_size_limit(10**9)


def cap():
    resource.setrlimit(resource.RLIMIT_AS, (3 << 30, 3 << 30))
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))


def described(binary, path):
    p = subprocess.run([binary, "describe", path], capture_output=True, timeout=60, preexec_fn=cap)
    if p.returncode != 0:
        return None
    try:
        d = json.loads(p.stdout)
        return d["rootClasses"], sorted(d["classes"])
    except (ValueError, KeyError):
        return None


def main(out, base_csv):
    cand_bin = os.path.join(out, "arachne-candidate")
    moved = {}
    for line in open(os.path.join(HERE, "moved.tsv")).read().splitlines():
        if line:
            i, roots, classes = line.split("\t")
            moved[i] = (json.loads(roots), json.loads(classes))
    if len(moved) != MOVED:
        print(f"INSTRUMENT: moved.tsv does not have the registered {MOVED} inputs")
        return 2
    base = {r["id"]: r for r in csv.DictReader(open(base_csv))}
    cand = {r["id"]: r for r in csv.DictReader(open(os.path.join(out, "cand", "files.csv")))}
    if set(base) != set(cand) or not set(moved) <= set(base):
        print("INSTRUMENT: the base and candidate censuses do not hold the same inputs, or a registered id is missing")
        return 2
    result = {}
    for i in sorted(cand):
        b, c = base[i], cand[i]
        if i in moved:
            got = described(cand_bin, c["path"])
            ok = b["describe_status"] == "ok" and c["describe_status"] == "ok" and got == moved[i]
            cls = "moved: as registered" if ok else "registered roots or classes not met"
        else:
            ok = all(b[k] == c[k] for k in ("describe_status", "describe_sha", "describe_id", "describe_detail"))
            cls = "identical" if ok else "differs from 643ff52"
        result[i] = (ok, cls)
    rows = [(i, cls) for i, (_, cls) in sorted(result.items())]
    fails = [f"{i}\t{cls}\n" for i, (ok, cls) in sorted(result.items()) if not ok]
    d = os.path.join(out, "d")
    os.makedirs(d, exist_ok=True)
    with open(os.path.join(d, "census.csv"), "w", newline="") as f:
        csv.writer(f).writerows([("id", "class")] + rows)
    open(os.path.join(d, "census-failures.txt"), "w").write("".join(fails))
    counts = {}
    for _, cls in rows:
        counts[cls] = counts.get(cls, 0) + 1
    print("D census: " + json.dumps(dict(sorted(counts.items()))) + f"; failures {len(fails)}")
    return 1 if fails else 0


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    try:
        sys.exit(main(*sys.argv[1:]))
    except (OSError, KeyError, ValueError, subprocess.TimeoutExpired) as e:
        print(f"INSTRUMENT: {e!r}")
        sys.exit(2)
