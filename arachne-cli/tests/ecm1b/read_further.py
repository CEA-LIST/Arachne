"""EC.M1h, R's judgment over refusal.py's refusal.csv (spec: cea-cdrt-knowledge/spec/items/EC.M1h.md).

    python3 -I arachne-cli/tests/ecm1b/read_further.py <r-dir>

A row both builds refuse whose first line differs fails R, except when a5f3111's line is exactly its
forward-package refusal (FORWARD): describe now reads that package and refuses later, on the file's
own cause, so the row is counted as read further. Exit 0 no other difference, 1 at least one, 2 the
instrument did not run.
"""
import csv
import os
import sys

FORWARD = ("[ERROR arachne] Error: Failed to parse Ecore metamodel: "
           "failed to define previously forward-referenced package: not empty")


def main(r_dir):
    path = os.path.join(r_dir, "refusal.csv")
    if not os.path.isfile(path):
        print(f"INSTRUMENT: {path} missing")
        return 2
    moved = [r for r in csv.DictReader(open(path)) if r["same"] == "0"]
    further = [r["id"] for r in moved if r["ref_line"] == FORWARD]
    bad = [r["id"] for r in moved if r["ref_line"] != FORWARD]
    open(os.path.join(r_dir, "read-further.txt"), "w").write("".join(i + "\n" for i in further))
    open(os.path.join(r_dir, "refusal-failures-judged.txt"), "w").write("".join(i + "\n" for i in bad))
    print(f"R: first line differs on {len(moved)}: read further past a5f3111's forward-package refusal "
          f"{len(further)} (counted), other {len(bad)} (judged)")
    return 1 if bad else 0


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    sys.exit(main(sys.argv[1]))
