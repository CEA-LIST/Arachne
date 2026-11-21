"""EC.M1m, D's judgment (spec: cea-cdrt-knowledge/spec/items/EC.M1m.md).

    python3 -I arachne-cli/tests/ecm1m/census.py <run.sh out-dir> <base files.csv>

The base is ecm1b/run.sh's candidate run at 91555a2 (its cand/files.csv, pinned by sha256 in
run.sh). Judged:
  M1 roots.tsv (37 inputs the base refuses for want of a root; their concrete classes contain each
     other in cycles): the candidate describes the input and its `rootClasses` is the registered list;
  M2 no-concrete.txt (1 input whose package declares only abstract classes and interfaces): the
     candidate refuses it, nothing on stdout, the first stderr line the base's, and exactly one later
     line holding both `no concrete` and `EClass`;
  M3 every other input: describe status, stdout sha256, identity line and stderr detail equal to
     the base's.
Writes census.csv and census-failures.txt in <out-dir>/d. Exit 0 all hold, 1 one fails, 2 the
instrument did not run.
"""
import csv
import json
import os
import re
import resource
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ANSI = re.compile(r"\x1b\[[0-9;]*m")
SIZES = {"roots": 37, "no-concrete": 1}
csv.field_size_limit(10**9)


def cap():
    resource.setrlimit(resource.RLIMIT_AS, (3 << 30, 3 << 30))
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))


def describe(binary, path):
    p = subprocess.run([binary, "describe", path], capture_output=True, timeout=60, preexec_fn=cap)
    return p.returncode, p.stdout, ANSI.sub("", p.stderr.decode("utf-8", "replace")).splitlines()


def main(out, base_csv):
    cand_bin = os.path.join(out, "arachne-candidate")
    roots = dict(l.split("\t") for l in open(os.path.join(HERE, "roots.tsv")).read().splitlines() if l)
    roots = {i: json.loads(r) for i, r in roots.items()}
    none = {l for l in open(os.path.join(HERE, "no-concrete.txt")).read().splitlines() if l}
    if (len(roots), len(none)) != (SIZES["roots"], SIZES["no-concrete"]) or none & set(roots):
        print(f"INSTRUMENT: the registered lists do not have the registered sizes {SIZES} or overlap")
        return 2
    base = {r["id"]: r for r in csv.DictReader(open(base_csv))}
    cand = {r["id"]: r for r in csv.DictReader(open(os.path.join(out, "cand", "files.csv")))}
    if set(base) != set(cand) or not (set(roots) | none) <= set(base):
        print("INSTRUMENT: the base and candidate censuses do not hold the same inputs, or a registered id is missing")
        return 2
    result = {}
    for i in sorted(cand):
        b, c = base[i], cand[i]
        if i in roots:
            rc, so, _ = describe(cand_bin, c["path"])
            try:
                got = json.loads(so)["rootClasses"] if rc == 0 else None
            except (ValueError, KeyError):
                got = None
            ok = b["describe_status"] != "ok" and c["describe_status"] == "ok" and got == roots[i]
            cls = "roots: as registered" if ok else "registered roots not met"
        elif i in none:
            rc, so, se = describe(cand_bin, c["path"])
            first = b["describe_detail"].splitlines()[:1]
            later = [l for l in se[1:] if "no concrete" in l and "EClass" in l]
            ok = rc > 0 and not so and se[:1] == first and len(later) == 1
            cls = "no concrete: refused with its cause" if ok else "registered no-concrete refusal not met"
        else:
            ok = all(b[k] == c[k] for k in ("describe_status", "describe_sha", "describe_id", "describe_detail"))
            cls = "identical" if ok else "differs from 91555a2"
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
