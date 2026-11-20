"""EC.07, census N (spec: cea-cdrt-knowledge/spec/items/EC.07.md).

    python3 -I arachne-cli/tests/ec07/census.py <base arachne> <candidate arachne> <new-out-dir>

Runs `describe <file>` with both binaries on the 5,517 census inputs (EC.01's list, cap.sh's caps,
30 s, 4 at a time). The twin of an input is its bytes with every construct EC.07 ignores replaced by
spaces (same length, same lines and columns), written by `blank` below from the XML, never by the
code under test. Judged per input where no run hit the deadline:
  S  the base describes it: the candidate's exit, stdout and stderr are byte-identical to the base's;
  R  the base refuses it and no W5 message is in its first line or a `caused by:` line: the same;
  M  the base refuses it on a W5 message ("moved"): the base's run on the twin gives the expected
     result. Refused there: the candidate's exit, stdout and stderr equal the twin's. Described
     there: the candidate's exit and stdout equal the twin's, and its stderr is the twin's with the
     W5 lines `blank` gives merged into the warning block right before the identity line (sorted,
     unique). An input whose twin cannot stand alone (a relative `.ecore#//` reference) or whose XML
     expat refuses is `moved-cross`: the candidate's stderr must hold no W5 message, and when it
     describes, every W5 line `blank` gives must be in its stderr.
  I  instrument: the twin of a moved input must not itself be refused on a W5 message.
Writes rows.csv, failures.txt and counts.json. Exit 0 all hold, 1 one fails, 2 the instrument did
not run.
"""
import collections
import concurrent.futures
import csv
import hashlib
import json
import os
import re
import sys
import importlib.util
import xml.parsers.expat as expat
from types import SimpleNamespace

HERE = os.path.dirname(os.path.abspath(__file__))
_spec = importlib.util.spec_from_file_location("ec04", os.path.join(HERE, "..", "ec04", "census.py"))
ec04 = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(ec04)  # EC.04's describe runner, input list and constants, unchanged

ECORE = "http://www.eclipse.org/emf/2002/Ecore"
XMI = "http://www.omg.org/XMI"
# The base's refusal messages for the constructs EC.07 ignores, by family.
W5 = {
    "xmi:id": r"Unsupported namespaced attribute `id` in `(EPackage|eSubpackages)`",
    "eFactoryInstance": r"Unsupported attribute `eFactoryInstance` in `(EPackage|eSubpackages)`",
    "eExceptions": r"Unsupported attribute `eExceptions` in `eOperations`",
    "annotationDetail": r"Missing attribute `value` in `details`",
    "annotationContents": r"Unsupported annotation element `contents`",
    "annotationReferences": r"Unsupported annotation element `references`",
}
CROSS = re.compile(rb"""["'\s][^"'\s#:]*\.ecore#//""")


def family(stderr):
    """The W5 family of a refusal, from its first line or a `caused by:` line; or None."""
    lines = stderr.decode("utf-8", "replace").split("\n")
    looked = [lines[0]] + [line for line in lines if line.lstrip().startswith("caused by:")]
    for name, rx in W5.items():
        if any(re.search(rx, line) for line in looked):
            return name
    return None


def blank(data):
    """(twin bytes, {(kind, location)}): every construct EC.07 ignores, replaced by spaces."""
    out = bytearray(data)
    found = set()
    stack = []  # one dict per open element
    p = expat.ParserCreate(namespace_separator=" ")
    p.namespace_prefixes = True

    def tag_end(begin):
        """(offset after the start tag at `begin`, whether it is an empty-element tag)."""
        i, quote = begin, None
        while True:
            c = data[i:i + 1]
            if quote:
                quote = None if c == quote else quote
            elif c in (b'"', b"'"):
                quote = c
            elif c == b">":
                return i + 1, data[i - 1:i] == b"/"
            i += 1

    def spaces(begin, end):
        out[begin:end] = bytes(c if c in b"\r\n" else 32 for c in data[begin:end])

    def blank_attr(begin, qname):
        end, _ = tag_end(begin)
        m = re.search(rb"(?<=\s)" + re.escape(qname.encode()) + rb"""\s*=\s*("[^"]*"|'[^']*')""",
                      data[begin:end])
        spaces(begin + m.start(), begin + m.end())

    def start(name, attrs):
        parts = name.split(" ")
        ns, local = (parts[0], parts[1]) if len(parts) > 1 else (None, parts[0])
        parent = stack[-1] if stack else None
        if parent and (parent["skip"] or parent["kind"]):
            parent["child"] = True
            stack.append({"skip": True, "kind": None})
            return
        ours = ns in (None, ECORE)
        f = {"local": local, "loc": parent["loc"] if parent else None, "sub": None, "skip": False,
             "kind": None, "begin": p.CurrentByteIndex, "child": False,
             "annotation": ours and local == "eAnnotations"}
        if parent is None:
            if (ns, local) == (ECORE, "EPackage"):
                f["loc"], f["sub"] = attrs.get("name"), ""
        elif not ours:
            pass
        elif parent["sub"] is not None and local in ("EPackage", "eSubpackages"):
            f["sub"] = (parent["sub"] + "::" if parent["sub"] else "") + (attrs.get("name") or "")
            f["loc"] = f["sub"]
        elif parent["sub"] is not None and local == "eClassifiers":
            f["loc"] = (parent["sub"] + "::" if parent["sub"] else "") + (attrs.get("name") or "")
        elif parent["local"] in ("eClassifiers", "eOperations") and local in (
                "eStructuralFeatures", "eOperations", "eLiterals", "eParameters"):
            f["loc"] = f"{parent['loc']}.{attrs.get('name')}"
        elif parent["annotation"]:
            if local == "details" and "value" not in attrs:
                f["kind"] = "annotationDetail"
            elif local in ("contents", "references"):
                f["kind"] = "annotation" + local.capitalize()
        if f["sub"] is not None:
            for qname in attrs:
                q = qname.split(" ")
                if len(q) == 3 and q[0] == XMI and q[1] == "id":
                    blank_attr(f["begin"], f"{q[2]}:id")
                    found.add(("xmi:id", f["loc"]))
                elif qname == "eFactoryInstance":
                    blank_attr(f["begin"], "eFactoryInstance")
                    found.add(("eFactoryInstance", f["loc"]))
        if ours and local == "eOperations" and "eExceptions" in attrs:
            blank_attr(f["begin"], "eExceptions")
            found.add(("eExceptions", f["loc"]))
        stack.append(f)

    def finish(name):
        f = stack.pop()
        if not f["kind"] or (f["kind"] == "annotationDetail" and f["child"]):
            return  # a details element with child elements is not ignored
        end, empty = tag_end(f["begin"])
        if not empty:
            end = data.index(b">", p.CurrentByteIndex) + 1
        spaces(f["begin"], end)
        found.add((f["kind"], stack[-1]["loc"]))

    p.StartElementHandler = start
    p.EndElementHandler = finish
    p.Parse(data, True)
    return bytes(out), found


def merged(twin_err, lines):
    """The twin's stderr with `lines` merged into its warning block before the identity line."""
    text = twin_err.decode("utf-8", "replace").split("\n")
    ident = max(i for i, line in enumerate(text) if line)
    first = ident
    while first > 0 and text[first - 1].startswith("warning: dropped "):
        first -= 1
    block = sorted(set(text[first:ident]) | set(lines))
    return "\n".join(text[:first] + block + text[ident:]).encode()


def judge(n, path, base, cand, base_bin, out):
    """(class, family, detail)."""
    if base is None or cand is None:
        return "deadline", "", ""
    if base[0] == 0:
        return ("identical", "", "") if base == cand else ("fail S", "", "differs from the base")
    fam = family(base[2])
    if fam is None:
        return ("refused", "", "") if base == cand else ("fail R", "", "refusal differs from the base")
    if family(cand[2]) is not None and cand[0] != 0:
        return "fail M", fam, "the candidate still refuses on a W5 message"
    data = open(path, "rb").read()
    try:
        twin, found = blank(data)
    except (expat.ExpatError, AttributeError, ValueError) as e:
        twin, found = None, set()
        why = f"xml {type(e).__name__}"
    lines = sorted(f"warning: dropped {k} at {loc}" for k, loc in found)
    if twin is None or CROSS.search(data):
        cerr = cand[2].decode("utf-8", "replace")
        if cand[0] == 0 and any(line not in cerr.split("\n") for line in lines):
            return "fail M", fam, "a W5 line `blank` gives is missing"
        return "moved-cross", fam, why if twin is None else "relative reference"
    d = os.path.join(out, "twins", str(n))
    os.makedirs(d)
    tpath = os.path.join(d, os.path.basename(path))
    open(tpath, "wb").write(twin)
    tw = ec04.describe(base_bin, tpath)
    if tw is None:
        return "deadline", fam, "twin"
    if family(tw[2]) is not None and tw[0] != 0:
        return "fail I", fam, "the twin is refused on a W5 message"
    want_err = tw[2] if tw[0] != 0 else merged(tw[2], lines)
    if (cand[0], cand[1], cand[2]) != (tw[0], tw[1], want_err):
        return "fail M", fam, f"exit {cand[0]} want {tw[0]}, stdout same {cand[1] == tw[1]}, stderr same {cand[2] == want_err}"
    return ("moved-read" if tw[0] == 0 else "moved-refused"), fam, ""


def main(base_bin, cand_bin, out):
    if os.path.exists(out):
        print(f"INSTRUMENT: {out} exists; give a new out-dir")
        return 2
    os.makedirs(out)
    items = ec04.ec01.list_inputs(SimpleNamespace(
        corpus=[ec04.CORPUS], fixture_dir=[os.path.join(ec04.FIX, "ecore"), os.path.join(ec04.FIX, "generate")]))
    if len(items) != 5517:
        print(f"INSTRUMENT: {len(items)} census inputs, expected 5517")
        return 2
    sha = {b: hashlib.sha256(open(b, "rb").read()).hexdigest() for b in (base_bin, cand_bin)}

    def one(arg):
        n, (fid, path) = arg
        return judge(n, path, ec04.describe(base_bin, path), ec04.describe(cand_bin, path), base_bin, out)

    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
        results = list(pool.map(one, enumerate(items)))
    classes, families = collections.Counter(), collections.Counter()
    rows, fails = [], []
    for (fid, _), (cls, fam, detail) in zip(items, results):
        classes[cls] += 1
        if fam:
            families[f"{fam} {cls}"] += 1
        rows.append((fid, cls, fam, detail))
        if cls.startswith("fail"):
            fails.append(f"{fid}\t{cls}\t{fam}\t{detail}\n")
    with open(os.path.join(out, "rows.csv"), "w", newline="") as f:
        csv.writer(f).writerows([("id", "class", "family", "detail")] + rows)
    open(os.path.join(out, "failures.txt"), "w").write("".join(fails))
    counts = {"bins": sha, "classes": dict(sorted(classes.items())),
              "moved_by_family": dict(sorted(families.items()))}
    json.dump(counts, open(os.path.join(out, "counts.json"), "w"), indent=2)
    print("N census: " + json.dumps(counts["classes"]) + f"; failures {len(fails)}")
    print("N moved by family: " + json.dumps(counts["moved_by_family"]))
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
