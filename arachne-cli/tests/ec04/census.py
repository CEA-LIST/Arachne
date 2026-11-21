"""EC.04, census E (spec: cea-cdrt-knowledge/spec/items/EC.04.md).

    python3 -I arachne-cli/tests/ec04/census.py <base arachne> <candidate arachne> <new-out-dir>

Runs `describe <file>` with both binaries on the 5,517 census inputs (EC.01's list: ModelSet and the
EC.01 fixtures, through ecm1b/cap.sh's caps, 30 s per run, 4 at a time) and judges each input where
neither run hit the deadline:
  S  exit status and stdout bytes equal to the base's;
  R  a run the base refuses: stderr byte-identical to the base's;
  F  a run the base describes: every candidate stderr line that is not a warning line, in order,
     equals the base's stderr; the warning lines form one block right before the last line, sorted
     (byte order), unique, each `warning: dropped <kind> at <location>` with a registered kind and a
     location whose classifier part is a key of stdout's `classes` or `enums`, the package, or an
     EDataType of the file;
  X  a run the base describes, with an EPackage root named as the descriptor's `package`: the warning
     lines that name the file's own classifiers or its package, superTypeOrder aside, equal the set the
     XML gives by the spec's rules. An input whose XML does not match the descriptor's listing (a
     listed class missing a feature it declares, an enum with other literals) is counted `model-skip`.
Writes rows.csv, failures.txt and counts.json in the out-dir. Exit 0 all hold, 1 one fails, 2 the
instrument did not run.
"""
import collections
import concurrent.futures
import csv
import hashlib
import json
import os
import re
import subprocess
import sys
import xml.etree.ElementTree as ET
from types import SimpleNamespace

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "ec01"))
import ec01  # noqa: E402  EC.01's input list, unchanged

CORPUS = "/volatile/home/vp242103/datasets/modelset/unpacked/modelset/raw-data/repo-ecore-all/data"
FIX = os.path.join(HERE, "..", "fixtures", "ec01")
CAP = os.path.join(HERE, "..", "ecm1b", "cap.sh")
KINDS = {"bounds", "changeable", "defaultValueLiteral", "derived", "eAnnotation", "eDataType", "eKeys",
         "eOperation", "instanceClassName", "interface", "literalValue", "nsPrefix", "resolveProxies",
         "superTypeOrder", "transient", "unsettable", "volatile"}
LINE = re.compile(r"warning: dropped (\S+) at (\S+)")
READ_SOURCES = {"urn:arachne:semantics", "urn:arachne:representation", "subsets", "redefines"}
XSI_TYPE = "{http://www.w3.org/2001/XMLSchema-instance}type"
# Flags whose value differs from Ecore's default: (attribute, value that is dropped).
FLAGS = [("changeable", False), ("volatile", True), ("transient", True), ("derived", True),
         ("unsettable", True), ("resolveProxies", False)]


def describe(binary, path):
    env = dict(os.environ, ECM1B_REAL=binary)
    try:
        p = subprocess.run([CAP, "describe", path], capture_output=True, timeout=30, env=env)
    except subprocess.TimeoutExpired:
        return None
    return p.returncode, p.stdout, p.stderr


def local(tag):
    return tag.rsplit("}", 1)[-1]


def kids(el, name):
    return [c for c in el if local(c.tag) == name]


def boolean(el, name):
    v = el.get(name)
    return None if v is None else v in ("true", "True", "1")


def foreign(el):
    return any(a.get("source") not in READ_SOURCES for a in kids(el, "eAnnotations"))


def expected(path, d):
    """The warning set the XML gives, the file's own classifiers and package only; or a skip cause."""
    root = ET.parse(path).getroot()
    if local(root.tag) != "EPackage" or root.get("name") != d.get("package"):
        return "other-root", None, set(), set()
    want, own, datatypes = set(), {root.get("name")}, set()
    pkg = root.get("name")
    if root.get("nsPrefix") is not None:
        want.add(("nsPrefix", pkg))
    if foreign(root):
        want.add(("eAnnotation", pkg))

    def walk(pack, prefix):
        for c in kids(pack, "eClassifiers"):
            key, typ = prefix + (c.get("name") or ""), (c.get(XSI_TYPE) or "").rsplit(":", 1)[-1]
            own.add(key)
            if typ == "EDataType":
                datatypes.add(key)
                want.add(("eDataType", key))
            elif typ == "EEnum" and key in d["enums"]:
                lits = kids(c, "eLiterals")
                if [lit.get("name") for lit in lits] != d["enums"][key]:
                    raise LookupError(f"enum {key}")
                if foreign(c):
                    want.add(("eAnnotation", key))
                for lit in lits:
                    at = f"{key}.{lit.get('name')}"
                    if lit.get("value") is not None or lit.get("literal") is not None:
                        want.add(("literalValue", at))
                    if foreign(lit):
                        want.add(("eAnnotation", at))
            elif typ == "EClass" and key in d["classes"]:
                entry = d["classes"][key]
                listed = {f["name"] for part in ("attributes", "containments", "references")
                          for f in entry[part]}
                if boolean(c, "interface"):
                    want.add(("interface", key))
                if c.get("instanceClassName") is not None or c.get("instanceTypeName") is not None:
                    want.add(("instanceClassName", key))
                if foreign(c):
                    want.add(("eAnnotation", key))
                for op in kids(c, "eOperations"):
                    want.add(("eOperation", f"{key}.{op.get('name')}"))
                for f in kids(c, "eStructuralFeatures"):
                    if f.get("name") not in listed:
                        raise LookupError(f"feature {key}.{f.get('name')}")
                    at = f"{key}.{f.get('name')}"
                    for flag, dropped in FLAGS:
                        if boolean(f, flag) == dropped:
                            want.add((flag, at))
                    if f.get("defaultValueLiteral") is not None or f.get("defaultValue") is not None:
                        want.add(("defaultValueLiteral", at))
                    if (f.get("eKeys") or "").split():
                        want.add(("eKeys", at))
                    lb, ub = int(f.get("lowerBound") or 0), int(f.get("upperBound") or 1)
                    if lb >= 2 or ub not in (1, -1):
                        want.add(("bounds", at))
                    if foreign(f):
                        want.add(("eAnnotation", at))
        for sub in kids(pack, "eSubpackages"):
            walk(sub, prefix + sub.get("name") + "::")

    try:
        walk(root, "")
    except LookupError as e:
        return f"model-skip ({e})", None, own, datatypes
    return None, want, own, datatypes


def judge(fid, path, base, cand):
    """(class, detail, warnings) for one input."""
    if base is None or cand is None:
        return "deadline", "", []
    if base[0] != cand[0] or base[1] != cand[1]:
        return "fail S", f"exit {base[0]}->{cand[0]}, stdout same {base[1] == cand[1]}", []
    if base[0] != 0:
        return ("refused" if base[2] == cand[2] else "fail R"), "", []
    blines = base[2].decode("utf-8", "replace").split("\n")
    clines = cand[2].decode("utf-8", "replace").split("\n")
    warn = [i for i, line in enumerate(clines) if line.startswith("warning: dropped ")]
    lines = [clines[i] for i in warn]
    rest = [line for i, line in enumerate(clines) if i not in set(warn)]
    if rest != blines:
        return "fail F", "stderr differs from the base's beyond warning lines", lines
    ident = max(i for i, line in enumerate(clines) if line)  # the identity line
    if warn and (warn != list(range(warn[0], warn[0] + len(warn))) or warn[-1] != ident - 1):
        return "fail F", "warning lines are not one block right before the identity line", lines
    if lines != sorted(set(lines)):
        return "fail F", "warning lines not sorted or not unique", lines
    d = json.loads(base[1])
    pairs = []
    for line in lines:
        m = LINE.fullmatch(line)
        if not m or m.group(1) not in KINDS:
            return "fail F", f"malformed line {line!r}", lines
        pairs.append((m.group(1), m.group(2)))
    try:
        skip, want, own, datatypes = expected(path, d)
    except (ET.ParseError, ValueError) as e:
        skip, want, own, datatypes = f"model-skip ({type(e).__name__})", None, set(), set()
    for kind, at in pairs:
        head = at.split(".", 1)[0]
        known = set(d["classes"]) | set(d["enums"]) | {d.get("package")} | datatypes
        if head not in known and not (kind == "eDataType" and "dependsOn" in d):
            return "fail F", f"location {at!r} names nothing stdout's descriptor or the file holds", lines
    if skip:
        return skip.split(" ", 1)[0], skip, lines
    got = {(k, at) for k, at in pairs if k != "superTypeOrder" and at.split(".", 1)[0] in own}
    if got != want:
        missing = sorted(f"{k} at {a}" for k, a in want - got)[:5]
        extra = sorted(f"{k} at {a}" for k, a in got - want)[:5]
        return "fail X", f"missing {missing} extra {extra}", lines
    return "identical+warnings", "", lines


def main(base_bin, cand_bin, out):
    if os.path.exists(out):
        print(f"INSTRUMENT: {out} exists; give a new out-dir")
        return 2
    os.makedirs(out)
    items = ec01.list_inputs(SimpleNamespace(
        corpus=[CORPUS], fixture_dir=[os.path.join(FIX, "ecore"), os.path.join(FIX, "generate")]))
    if len(items) != 5517:
        print(f"INSTRUMENT: {len(items)} census inputs, expected 5517")
        return 2
    sha = {b: hashlib.sha256(open(b, "rb").read()).hexdigest() for b in (base_bin, cand_bin)}

    def one(item):
        fid, path = item
        return judge(fid, path, describe(base_bin, path), describe(cand_bin, path))

    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
        results = list(pool.map(one, items))
    rows, fails = [], []
    classes, kinds, inputs = collections.Counter(), collections.Counter(), collections.Counter()
    for (fid, _), (cls, detail, lines) in zip(items, results):
        classes[cls] += 1
        rows.append((fid, cls, len(lines), detail))
        if cls.startswith("fail"):
            fails.append(f"{fid}\t{cls}\t{detail}\n")
        seen = set()
        for line in lines:
            m = LINE.fullmatch(line)
            if m:
                kinds[m.group(1)] += 1
                seen.add(m.group(1))
        inputs.update(seen)
    with open(os.path.join(out, "rows.csv"), "w", newline="") as f:
        csv.writer(f).writerows([("id", "class", "warnings", "detail")] + rows)
    open(os.path.join(out, "failures.txt"), "w").write("".join(fails))
    counts = {"bins": sha, "classes": dict(sorted(classes.items())),
              "warning_lines_by_kind": dict(sorted(kinds.items())),
              "inputs_by_kind": dict(sorted(inputs.items()))}
    json.dump(counts, open(os.path.join(out, "counts.json"), "w"), indent=2)
    print("E census: " + json.dumps(counts["classes"]) + f"; failures {len(fails)}")
    print("E warnings by kind: " + json.dumps(counts["warning_lines_by_kind"]))
    return 1 if fails else 0


if __name__ == "__main__":
    if len(sys.argv) != 4:
        sys.exit(__doc__)
    os.environ["LC_ALL"] = "C"
    try:
        sys.exit(main(*sys.argv[1:]))
    except (OSError, KeyError) as e:
        print(f"INSTRUMENT: {e!r}")
        sys.exit(2)
