"""EC.M1l-b ground truth (spec: cea-cdrt-knowledge/spec/items/EC.M1l-b.md).

    python3 -I arachne-cli/tests/ecm1l_b/twins.py <060d88f arachne binary> <out-dir>

For each registered enum-keyed fixture, which 060d88f refuses, two twins that 060d88f describes:
  A  the fixture without the `uw-map` annotation block (`entries` an ordinary containment);
  B  the fixture with the key attribute typed EString (`entries` a str-keyed map).
The expected descriptor is A's, with `Root.containments[entries]` replaced by B's entry whose
`merge.shape.key` is `{"kind": "enum", "class": <the key enum's name as A's `enums` keys it>}`;
the expected digest is sha256 of its compact sorted JSON, the formula checked here against 060d88f's
own identity line on every twin. Writes <out-dir>/<stem>.json and <out-dir>/identity.tsv.
Exit 0 written, 2 the instrument did not run.
"""
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
FIX = os.path.join(HERE, "..", "fixtures")
# id (relative to tests/fixtures), the key enum's descriptor name
REGISTERED = [
    ("ecm1k/enum_key.ecore", "Color"),
    ("ecm1k/empty_enum_key.ecore", "Color"),
    ("ecm1l_b/containment_value.ecore", "Color"),
    ("ecm1l_b/sub_key.ecore", "sub::Shade"),
    ("ecm1l_b/import/cross_key.ecore", "tones::Tone"),
]
ANNOT = re.compile(r'\s*<eAnnotations source="urn:arachne:semantics">.*?</eAnnotations>', re.S)
KEY = re.compile(r'(name="key" lowerBound="1" eType=")[^"]*(")')
ESTRING = "ecore:EDataType http://www.eclipse.org/emf/2002/Ecore#//EString"


def digest(d):
    return hashlib.sha256(json.dumps(d, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def describe(binary, path):
    p = subprocess.run([binary, "describe", path], capture_output=True, text=True, timeout=60)
    if p.returncode != 0:
        raise RuntimeError(f"060d88f does not describe the twin {path}")
    d = json.loads(p.stdout)
    ident = json.loads(re.sub(r"\x1b\[[0-9;]*m", "", p.stderr).strip().splitlines()[-1])
    if ident["digest"] != digest(d):
        raise RuntimeError(f"digest formula differs from 060d88f on {path}")
    return d


def entries(d):
    (e,) = [c for c in d["classes"]["Root"]["containments"] if c["name"] == "entries"]
    return e


def main(binary, out):
    os.makedirs(out)
    rows = []
    for rel, name in REGISTERED:
        src = os.path.join(FIX, rel)
        text = open(src).read()
        a, n_a = ANNOT.subn("", text)
        b, n_b = KEY.subn(r"\g<1>" + ESTRING + r"\g<2>", text)
        if n_a != 1 or n_b != 1:
            raise RuntimeError(f"{rel}: twin rewrite matched {n_a} and {n_b} times, not once")
        work = os.path.join(out, "work")
        shutil.copytree(os.path.dirname(src), work)
        stem = os.path.splitext(os.path.basename(rel))[0]
        open(os.path.join(work, "twin_a.ecore"), "w").write(a)
        open(os.path.join(work, "twin_b.ecore"), "w").write(b)
        da = describe(binary, os.path.join(work, "twin_a.ecore"))
        db = describe(binary, os.path.join(work, "twin_b.ecore"))
        shutil.rmtree(work)
        if name not in da["enums"] or entries(db)["merge"]["shape"] != {"kind": "keyed", "key": {"kind": "str"}}:
            raise RuntimeError(f"{rel}: twin A lists no enum {name}, or twin B's entries is no str-keyed map")
        e = entries(db)
        e["merge"]["shape"]["key"] = {"kind": "enum", "class": name}
        cs = da["classes"]["Root"]["containments"]
        cs[[c["name"] for c in cs].index("entries")] = e
        open(os.path.join(out, stem + ".json"), "w").write(json.dumps(da, indent=2, sort_keys=True) + "\n")
        rows.append(f"{rel}\t{name}\t{digest(da)}\t{da['nsURI']}\n")
    open(os.path.join(out, "identity.tsv"), "w").write("".join(rows))
    return 0


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    try:
        sys.exit(main(*sys.argv[1:]))
    except (OSError, KeyError, ValueError, RuntimeError, subprocess.TimeoutExpired) as e:
        print(f"INSTRUMENT: {e!r}")
        sys.exit(2)
