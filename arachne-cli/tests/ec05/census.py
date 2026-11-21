"""EC.05, census N (spec: cea-cdrt-knowledge/spec/items/EC.05.md).

    python3 -I arachne-cli/tests/ec05/census.py <40f4043 arachne> <candidate arachne> <new-out-dir>

Runs `describe <file>` with both binaries on EC.01's 5,517 census inputs (EC.04's runner: cap.sh
caps, 30 s, 4 at a time) and judges each input where neither run hit the deadline:
  R  the base refuses: the candidate's exit, stdout and stderr are byte-identical to the base's;
  S  the base describes: the candidate exits 0 and its stdout with EC.05's keys removed
     (model.strip) renders to the base's stdout bytes, `dependsOn` aside, whose nsURIs are the
     base's (its digests are X's);
  D  its digest differs from the base's;
  E  its stderr is the base's without the warning lines of carried facts (model.carried_line),
     with the identity line naming the candidate descriptor's digest;
  X  its stdout is model.extend_file(base stdout, file): every added key as the XML gives it, each
     `dependsOn` digest its dependency's (40f4043's descriptor of that file, extended). An input
     the model cannot map is `model-skip`, judged on T instead: every added key present with its
     type, `superTypeOrder` a permutation of `superTypes`, `enumLiterals` naming `enums`' literals.
Writes rows.csv, failures.txt, counts.json and pairs/<n>.{base,cand}.json (the described inputs,
for ec05_describe_carries's census table test) in the out-dir. Exit 0 all hold, 1 one fails, 2 the
instrument did not run.
"""
import collections
import concurrent.futures
import csv
import importlib.util
import json
import os
import sys
import xml.etree.ElementTree as ET
from types import SimpleNamespace

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import model  # noqa: E402

_spec = importlib.util.spec_from_file_location("ec04", os.path.join(HERE, "..", "ec04", "census.py"))
ec04 = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(ec04)


def identity(d):
    return json.dumps({"digest": model.digest(d), "nsURI": d["nsURI"]}, separators=(",", ":"))


def typed(d):
    """T: the added keys' presence and types, for an input the model cannot map."""
    if not isinstance(d.get("nsPrefix"), (str, type(None))):
        return "nsPrefix"
    lits = d.get("enumLiterals")
    if not isinstance(lits, dict) or sorted(lits) != sorted(d["enums"]):
        return "enumLiterals keys"
    for key, names in d["enums"].items():
        if [x.get("name") for x in lits[key]] != names or any(
                set(x) != {"name", "value", "literal"} or not isinstance(x["value"], (int, str, type(None)))
                or not isinstance(x["literal"], (str, type(None))) for x in lits[key]):
            return f"enumLiterals {key}"
    for key, c in d["classes"].items():
        if not isinstance(c.get("interface"), bool) or sorted(c.get("superTypeOrder") or []) != sorted(c["superTypes"]):
            return f"class {key}"
        for part in model.PARTS:
            for f in c[part]:
                if not (type(f.get("lowerBound")) is int and type(f.get("upperBound")) is int
                        and isinstance(f.get("default", 0), (str, type(None)))
                        and isinstance(f.get("changeable"), bool) and isinstance(f.get("unsettable"), bool)
                        and isinstance(f.get("eKeys"), list)):
                    return f"feature {key}.{f.get('name')}"
    return None


def facts(d):
    """Non-default carried facts of one descriptor, by key."""
    n = collections.Counter()
    n["nsPrefix"] += d["nsPrefix"] is not None
    for lits in d["enumLiterals"].values():
        for x in lits:
            n["literal.value"] += x["value"] is not None
            n["literal.literal"] += x["literal"] is not None
    for c in d["classes"].values():
        n["interface"] += c["interface"]
        n["superTypeOrder"] += c["superTypeOrder"] != c["superTypes"]
        for part in model.PARTS:
            for f in c[part]:
                n["lowerBound>=2"] += f["lowerBound"] >= 2
                n["upperBound not 1/-1"] += f["upperBound"] not in (1, -1)
                n["default"] += f["default"] is not None
                n["changeable false"] += not f["changeable"]
                n["unsettable"] += f["unsettable"]
                n["eKeys"] += bool(f["eKeys"])
    return n


def base_describer(base_bin):
    cache = {}

    def describe(path):
        if path not in cache:
            r = ec04.describe(base_bin, path)
            cache[path] = json.loads(r[1]) if r and r[0] == 0 else None
        return cache[path]
    return describe


def without_deps(d):
    return {k: v for k, v in d.items() if k != "dependsOn"}


def judge(path, base, cand, describe):
    """(class, detail, base descriptor, candidate descriptor, the model's facts or None)."""
    if base is None or cand is None:
        return "deadline", "", None, None, None
    if base[0] != 0:
        same = base == cand
        return ("refused" if same else "fail R"), "" if same else f"exit {base[0]}->{cand[0]}", None, None, None
    if cand[0] != 0:
        return "fail S", f"exit 0->{cand[0]}", None, None, None
    b, c = json.loads(base[1]), json.loads(cand[1])
    try:
        expected = model.extend_file(b, path, describe)
    except (LookupError, ET.ParseError, ValueError) as e:
        expected, skip = None, str(e)
    f = facts(expected) if expected else None
    if model.pretty(without_deps(model.strip(c))) != model.pretty(without_deps(b)) or sorted(
            x["nsURI"] for x in c.get("dependsOn", [])) != sorted(x["nsURI"] for x in b.get("dependsOn", [])):
        return "fail S", "stdout without the added keys is not the base's", b, c, f
    if model.digest(c) == model.digest(b):
        return "fail D", "digest did not move", b, c, f
    text = open(path, encoding="utf-8", errors="replace").read()
    blines = base[2].decode("utf-8", "replace").split("\n")
    last = max(i for i, line in enumerate(blines) if line)
    want = [line for line in blines[:last] if not model.carried_line(line, text)] + [identity(c)] + blines[last + 1:]
    if cand[2].decode("utf-8", "replace").split("\n") != want:
        return "fail E", "stderr is not the base's without carried lines and with the new identity", b, c, f
    if expected is None:
        bad = typed(c)
        return ("fail T", bad, b, c, f) if bad else ("model-skip", skip, b, c, f)
    if c != expected:
        diff = [k for k in sorted(set(c["classes"]) | set(expected["classes"]))
                if c["classes"].get(k) != expected["classes"].get(k)][:3]
        top = [k for k in model.TOP_KEYS if c.get(k) != expected.get(k)]
        return "fail X", f"differs from the model at {top + diff}", b, c, f
    return "identical+carried", "", b, c, f


def main(base_bin, cand_bin, out):
    if os.path.exists(out):
        print(f"INSTRUMENT: {out} exists; give a new out-dir")
        return 2
    os.makedirs(os.path.join(out, "pairs"))
    items = ec04.ec01.list_inputs(SimpleNamespace(
        corpus=[ec04.CORPUS], fixture_dir=[os.path.join(ec04.FIX, "ecore"), os.path.join(ec04.FIX, "generate")]))
    if len(items) != 5517:
        print(f"INSTRUMENT: {len(items)} census inputs, expected 5517")
        return 2

    describer = base_describer(base_bin)

    def one(numbered):
        n, (fid, path) = numbered
        cls, detail, b, c, f = judge(path, ec04.describe(base_bin, path), ec04.describe(cand_bin, path), describer)
        if b is not None and c is not None:
            for side, d in (("base", b), ("cand", c)):
                with open(os.path.join(out, "pairs", f"{n:04d}.{side}.json"), "w") as fh:
                    json.dump(d, fh)
        return cls, detail, f

    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
        results = list(pool.map(one, enumerate(items)))
    rows, fails = [], []
    classes, kinds, inputs = collections.Counter(), collections.Counter(), collections.Counter()
    mapped = collections.Counter()
    for (fid, _), (cls, detail, f) in zip(items, results):
        classes[cls] += 1
        mapped["model-mapped" if f is not None else "unmapped or refused"] += 1
        rows.append((fid, cls, "mapped" if f is not None else "-", detail))
        if cls.startswith("fail"):
            fails.append(f"{fid}\t{cls}\t{detail}\n")
        if f:
            kinds.update(f)
            inputs.update(k for k, v in f.items() if v)
    with open(os.path.join(out, "rows.csv"), "w", newline="") as fh:
        csv.writer(fh).writerows([("id", "class", "model", "detail")] + rows)
    open(os.path.join(out, "failures.txt"), "w").write("".join(fails))
    counts = {"classes": dict(sorted(classes.items())), "model": dict(sorted(mapped.items())), "facts": dict(sorted(kinds.items())),
              "inputs_with_fact": dict(sorted(inputs.items()))}
    json.dump(counts, open(os.path.join(out, "counts.json"), "w"), indent=2)
    print("N census: " + json.dumps(counts["classes"]) + f"; failures {len(fails)}")
    print("N model: " + json.dumps(counts["model"]))
    print("N facts (the model's, on mapped inputs): " + json.dumps(counts["facts"]))
    print("N inputs with a fact: " + json.dumps(counts["inputs_with_fact"]))
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
