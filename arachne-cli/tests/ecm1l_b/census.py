"""EC.M1l-b, D's judgment (spec: cea-cdrt-knowledge/spec/items/EC.M1l-b.md).

    python3 -I arachne-cli/tests/ecm1l_b/census.py <run.sh out-dir> <base files.csv>

The base is EC.M1n's assessed run at 060d88f (its cand/files.csv, pinned by sha256 in run.sh). No
census input is refused for an enum-keyed map at 060d88f, so every input is judged the same way:
describe status, stdout sha256, identity line and stderr detail equal to the base's.
Writes census.csv and census-failures.txt in <out-dir>/d. Exit 0 all hold, 1 one fails, 2 the
instrument did not run.
"""
import csv
import json
import os
import sys

csv.field_size_limit(10**9)
KEYS = ("describe_status", "describe_sha", "describe_id", "describe_detail")


def main(out, base_csv):
    base = {r["id"]: r for r in csv.DictReader(open(base_csv))}
    cand = {r["id"]: r for r in csv.DictReader(open(os.path.join(out, "cand", "files.csv")))}
    if set(base) != set(cand) or len(base) != 5517:
        print("INSTRUMENT: the base and candidate censuses do not hold the same 5517 inputs")
        return 2
    rows = [(i, "identical" if all(base[i][k] == cand[i][k] for k in KEYS) else "differs from 060d88f")
            for i in sorted(cand)]
    fails = [f"{i}\t{cls}\n" for i, cls in rows if cls != "identical"]
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
    except (OSError, KeyError, ValueError) as e:
        print(f"INSTRUMENT: {e!r}")
        sys.exit(2)
