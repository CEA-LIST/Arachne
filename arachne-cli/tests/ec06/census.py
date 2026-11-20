"""EC.06, census N (spec: cea-cdrt-knowledge/spec/items/EC.06.md).

    python3 -I arachne-cli/tests/ec06/census.py <9908485 arachne> <candidate arachne> <new-out-dir>

Runs `describe <file>` with both binaries on EC.01's 5,517 census inputs (EC.04's runner: cap.sh
caps, 30 s, 4 at a time) and judges each input where neither run hit the deadline:
  R  the base refuses: the candidate's exit, stdout and stderr are byte-identical to the base's;
  I  the base describes and the model adds nothing (no datatype, no moved dependency): the
     candidate's exit, stdout and stderr are byte-identical to the base's (class `identical`);
  S  otherwise the candidate exits 0, its stdout is its descriptor rendered as serde's `{:#}`, and
     that descriptor without `datatypes` renders to the base's stdout bytes, `dependsOn` aside
     (same nsURIs; its digests are X's);
  E  its stderr is the base's without the `eDataType` lines of plain own datatypes
     (model.plain_keys), with the identity line naming the candidate descriptor's digest;
  X  its descriptor is model.extend_file(base descriptor, file) (class `extended`).
An input the model cannot map is `model-skip`, judged on T instead: `datatypes` present, every entry
`{instanceClassName: string|null, serializable: bool}`, and E with the lines removed a subset of the
`eDataType` lines at its keys. Writes rows.csv, failures.txt, counts.json and
pairs/<n>.{base,cand}.json (the described inputs, for ec05_describe_carries's census table test,
EC05_PAIRS=<out-dir>/pairs) in the out-dir. Exit 0 all hold, 1 one fails, 2 the instrument did not run.
"""
import collections
import concurrent.futures
import csv
import importlib.util
import json
import os
import re
import sys
import xml.etree.ElementTree as ET
from types import SimpleNamespace

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import model  # noqa: E402

_spec = importlib.util.spec_from_file_location("ec04", os.path.join(HERE, "..", "ec04", "census.py"))
ec04 = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(ec04)

DT_LINE = "warning: dropped eDataType at "


def identity(d):
    return json.dumps({"digest": model.digest(d), "nsURI": d["nsURI"]}, separators=(",", ":"))


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


def expected_err(base_err, cand, keep):
    lines = base_err.decode("utf-8", "replace").split("\n")
    last = max(i for i, line in enumerate(lines) if line)
    return [line for line in lines[:last] if keep(line)] + [identity(cand)] + lines[last + 1:]


def typed(c, base_err, cand_err):
    dts = c.get("datatypes")
    if not isinstance(dts, dict) or not dts:
        return "datatypes"
    for k, v in dts.items():
        if not (isinstance(v, dict) and set(v) == {"instanceClassName", "serializable"}
                and isinstance(v["instanceClassName"], (str, type(None))) and isinstance(v["serializable"], bool)):
            return f"datatype {k}"
    gone = [line for line in base_err.decode("utf-8", "replace").split("\n")
            if line.startswith(DT_LINE) and line not in cand_err.decode("utf-8", "replace").split("\n")]
    if any(line[len(DT_LINE):] not in dts for line in gone):
        return "a line left that names no datatype"
    if cand_err.decode("utf-8", "replace").split("\n") != expected_err(base_err, c, lambda line: line not in gone):
        return "stderr"
    return None


def judge(path, base, cand, describe):
    """(class, detail, base descriptor, candidate descriptor, the model's datatypes or None)."""
    if base is None or cand is None:
        return "deadline", "", None, None, None
    if base[0] != 0:
        same = base == cand
        return ("refused" if same else "fail R"), "" if same else f"exit {base[0]}->{cand[0]}", None, None, None
    if cand[0] != 0:
        return "fail S", f"exit 0->{cand[0]}", None, None, None
    b = json.loads(base[1])
    try:
        c = json.loads(cand[1])
    except ValueError:
        return "fail S", "stdout is not JSON", b, None, None
    try:
        expected = model.extend_file(b, path, describe)
        keys = model.plain_keys(b, path)
    except (LookupError, ET.ParseError, ValueError, OSError) as e:
        expected, keys, skip = None, None, str(e)
    dts = expected.get("datatypes", {}) if expected else None
    if expected == b:
        if cand != base:
            return "fail I", "the model adds nothing, the candidate's output is not the base's", b, c, dts
        return "identical", "", b, c, dts
    if cand[1] != model.pretty(c):
        return "fail S", "stdout is not its descriptor rendered", b, c, dts
    # Bytes, but for a dependency's digest, which X judges.
    want = base[1] if "dependsOn" not in b else model.pretty(without_deps(b))
    if model.pretty(without_deps(model.strip(c))) != want or sorted(
            x["nsURI"] for x in c.get("dependsOn", [])) != sorted(x["nsURI"] for x in b.get("dependsOn", [])):
        return "fail S", "stdout without datatypes is not the base's", b, c, dts
    if expected is None:
        if cand == base:
            declares = re.search(r"""xsi:type\s*=\s*["']ecore:EDataType["']""", open(path, encoding="utf-8", errors="replace").read())
            return ("fail T", "no datatypes, the file declares one", b, c, dts) if declares else ("model-skip", skip, b, c, dts)
        bad = typed(c, base[2], cand[2])
        return ("fail T", bad, b, c, dts) if bad else ("model-skip", skip, b, c, dts)
    if cand[2].decode("utf-8", "replace").split("\n") != expected_err(base[2], c, lambda line: not model.carried_line(line, keys)):
        return "fail E", "stderr is not the base's without the plain eDataType lines and with the new identity", b, c, dts
    if c != expected:
        top = [k for k in ("datatypes", "dependsOn") if c.get(k) != expected.get(k)]
        return "fail X", f"differs from the model at {top}", b, c, dts
    return "extended", "", b, c, dts


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
        cls, detail, b, c, dts = judge(path, ec04.describe(base_bin, path), ec04.describe(cand_bin, path), describer)
        if b is not None and c is not None:
            for side, d in (("base", b), ("cand", c)):
                with open(os.path.join(out, "pairs", f"{n:04d}.{side}.json"), "w") as fh:
                    json.dump(d, fh)
        return cls, detail, dts

    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
        results = list(pool.map(one, enumerate(items)))
    rows, fails = [], []
    classes, facts = collections.Counter(), collections.Counter()
    for (fid, _), (cls, detail, dts) in zip(items, results):
        classes[cls] += 1
        rows.append((fid, cls, detail))
        if cls.startswith("fail"):
            fails.append(f"{fid}\t{cls}\t{detail}\n")
        if dts:
            facts["inputs with datatypes"] += 1
            facts["datatypes"] += len(dts)
            facts["instanceClassName null"] += sum(v["instanceClassName"] is None for v in dts.values())
            facts["serializable false"] += sum(not v["serializable"] for v in dts.values())
    with open(os.path.join(out, "rows.csv"), "w", newline="") as fh:
        csv.writer(fh).writerows([("id", "class", "detail")] + rows)
    open(os.path.join(out, "failures.txt"), "w").write("".join(fails))
    counts = {"classes": dict(sorted(classes.items())), "facts": dict(sorted(facts.items()))}
    json.dump(counts, open(os.path.join(out, "counts.json"), "w"), indent=2)
    print("N census: " + json.dumps(counts["classes"]) + f"; failures {len(fails)}")
    print("N facts (the model's): " + json.dumps(counts["facts"]))
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
