"""EC.M1i, D's judgment (spec: cea-cdrt-knowledge/spec/items/EC.M1i.md).

    python3 -I arachne-cli/tests/ecm1i/closure.py <run.sh out-dir> <base files.csv> <base bin>

The base is the orchestrator's run of ecm1b/run.sh at b93a76d (its cand/files.csv and its
arachne-candidate, both pinned by sha256 in run.sh). Judged per input of the census:
  D1 every id of refused.txt: the candidate refuses it, prints nothing on stdout, and one stderr line
     says `descriptor not closed` (counted 'refused: descriptor not closed');
  D2 every id of changed.txt: both describe it, the bytes differ, and the candidate's descriptor is
     closed (every supertype, target, enum and merge-level name it writes is a class or enum it
     writes), keeps every class, enum, root and other key the base wrote unchanged, and only adds
     classes, among them every name the base wrote without defining; an added `ecore::EObject` is
     the featureless concrete class (counted 'changed: closure');
  D3 every other input: status, stdout sha256 and identity line equal to the base's.
Writes closure.csv and closure-failures.txt in <out-dir>/d. Exit 0 all hold, 1 one fails, 2 the
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
EOBJECT = {"abstract": False, "superTypes": [], "attributes": [], "containments": [], "references": []}


def cap():
    resource.setrlimit(resource.RLIMIT_AS, (3 << 30, 3 << 30))
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))


def describe(binary, path):
    p = subprocess.run([binary, "describe", path], capture_output=True, timeout=60, preexec_fn=cap)
    return p.returncode, p.stdout, ANSI.sub("", p.stderr.decode("utf-8", "replace"))


def undefined(d):
    classes, enums = d["classes"], d["enums"]
    bad = []
    for cn, c in classes.items():
        bad += [s for s in c["superTypes"] if s not in classes]
        for f in c["containments"] + c["references"]:
            bad += [n for n in (f["target"], f["merge"].get("target")) if n is not None and n not in classes]
        for a in c["attributes"]:
            if "enum" in a and a["enum"] not in enums:
                bad.append(a["enum"])
            leaf = a["merge"].get("leaf") or {}
            if "class" in leaf and leaf["class"] not in classes and leaf["class"] not in enums:
                bad.append(leaf["class"])
    return set(bad)


def lines(name):
    return [l for l in open(os.path.join(HERE, name)).read().splitlines() if l]


def main(out, base_csv, base_bin):
    cand_bin = os.path.join(out, "arachne-candidate")
    refused, changed = set(lines("refused.txt")), set(lines("changed.txt"))
    if len(refused) != 42 or len(changed) != 68 or refused & changed:
        print("INSTRUMENT: refused.txt must hold 42 ids and changed.txt 68, disjoint")
        return 2
    base = {r["id"]: r for r in csv.DictReader(open(base_csv))}
    cand = {r["id"]: r for r in csv.DictReader(open(os.path.join(out, "cand", "files.csv")))}
    if set(base) != set(cand) or not (refused | changed) <= set(base):
        print("INSTRUMENT: the base and candidate censuses do not hold the same inputs, or a registered id is missing")
        return 2
    rows, fails = [], []
    for i in sorted(cand):
        b, c = base[i], cand[i]
        if i in refused:
            rc, so, se = describe(cand_bin, c["path"])
            ok = c["describe_status"] == "refused" and rc > 0 and not so and "descriptor not closed" in se
            cls = "refused: descriptor not closed" if ok else "registered refusal not met"
        elif i in changed:
            ok = b["describe_status"] == c["describe_status"] == "ok" and b["describe_sha"] != c["describe_sha"]
            if ok:
                rb, ob, _ = describe(base_bin, c["path"])
                rc, oc, _ = describe(cand_bin, c["path"])
                ok = rb == 0 and rc == 0
            if ok:
                db, dc = json.loads(ob), json.loads(oc)
                keep = all(dc.get(k) == v for k, v in db.items() if k not in ("classes", "dependsOn"))
                # a written target may name another file: dependsOn may grow, never lose an entry
                keep = keep and all(x in dc.get("dependsOn", []) for x in db.get("dependsOn", []))
                kept = all(dc["classes"].get(k) == v for k, v in db["classes"].items())
                added = set(dc["classes"]) - set(db["classes"])
                eobject = "ecore::EObject" not in added or dc["classes"]["ecore::EObject"] == EOBJECT
                ok = keep and kept and not undefined(dc) and undefined(db) <= added and eobject
            cls = "changed: closure" if ok else "registered change not met"
        else:
            ok = all(b[k] == c[k] for k in ("describe_status", "describe_sha", "describe_id"))
            cls = "identical" if ok else "differs from b93a76d"
        rows.append((i, cls))
        if not ok:
            fails.append(f"{i}\t{cls}\n")
    d = os.path.join(out, "d")
    os.makedirs(d, exist_ok=True)
    with open(os.path.join(d, "closure.csv"), "w", newline="") as f:
        csv.writer(f).writerows([("id", "class")] + rows)
    open(os.path.join(d, "closure-failures.txt"), "w").write("".join(fails))
    counts = {}
    for _, cls in rows:
        counts[cls] = counts.get(cls, 0) + 1
    print("D closure: " + json.dumps(dict(sorted(counts.items()))) + f"; failures {len(fails)}")
    return 1 if fails else 0


if __name__ == "__main__":
    if len(sys.argv) != 4:
        sys.exit(__doc__)
    try:
        sys.exit(main(*sys.argv[1:]))
    except (OSError, KeyError, ValueError, subprocess.TimeoutExpired) as e:
        print(f"INSTRUMENT: {e!r}")
        sys.exit(2)
