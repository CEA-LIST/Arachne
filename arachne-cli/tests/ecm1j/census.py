"""EC.M1j, D's judgment (spec: cea-cdrt-knowledge/spec/items/EC.M1j.md).

    python3 -I arachne-cli/tests/ecm1j/census.py <run.sh out-dir> <base files.csv> <base bin>

The base is ecm1b/run.sh's candidate run at 18ccade (its cand/files.csv and its arachne-candidate,
pinned by sha256 in run.sh). For an input of changed.txt or described.txt the expected descriptor
is its flat twin's (flat.py): every subpackage classifier moved into the root package as
`a__b__Name`, described by the base binary, renamed back to `a::b::Name`, `packages` added. Judged:
  J1 changed.txt (the base describes the input): the candidate describes it, its descriptor equals
     the expected one and its identity line is {digest of the expected one, the root's nsURI};
  J2 described.txt (the base refuses the input): as J1;
  J3 refused.txt (every other input with a subpackage classifier): the candidate refuses it,
     nothing on stdout, the first stderr line equal to the base's;
  J4 every other input: status, stdout sha256 and identity line equal to the base's.
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
sys.path.insert(0, HERE)
import flat  # noqa: E402

ANSI = re.compile(r"\x1b\[[0-9;]*m")
SIZES = {"changed": 27, "described": 114, "refused": 261}


def cap():
    resource.setrlimit(resource.RLIMIT_AS, (3 << 30, 3 << 30))
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))


def describe(binary, path):
    p = subprocess.run([binary, "describe", path], capture_output=True, timeout=60, preexec_fn=cap)
    return p.returncode, p.stdout, ANSI.sub("", p.stderr.decode("utf-8", "replace")).splitlines()


def refs(path):
    """The files a .ecore names by a relative href, as written."""
    found = set()
    for e in flat.ET.parse(path).iter():
        for v in e.attrib.values():
            for tok in v.split():
                ref = tok.split("#", 1)[0]
                if "#" in tok and ref.endswith(".ecore") and ":" not in ref:
                    found.add(ref)
    return found


def expected(base_bin, path, work):
    """(the expected descriptor, the expected identity), from the flat twin in `work`."""
    tree = flat.Tree(path)
    # Deep enough that a `../` href from the twin stays inside `work`; dependencies are the originals.
    d = os.path.join(work, *["m"] * 8)
    os.makedirs(d)
    twin = os.path.join(d, os.path.basename(path))
    names = tree.flatten(twin)
    if tree.skip:
        raise ValueError(f"{path}: the flat twin does not model it: {tree.skip}")
    for ref in refs(path):
        src = os.path.normpath(os.path.join(os.path.dirname(path), ref))
        dst = os.path.normpath(os.path.join(d, ref))
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        if os.path.exists(src) and not os.path.lexists(dst):
            os.symlink(src, dst)
    rc, out, err = describe(base_bin, twin)
    if rc != 0:
        raise ValueError(f"{path}: the base refuses its flat twin: {err[:1]}")
    want, written = flat.expected(json.loads(out), names, tree.packages())
    if not written:
        raise ValueError(f"{path}: the flat twin writes no subpackage classifier")
    return want, {"digest": flat.digest(want), "nsURI": want["nsURI"]}


def lines(name):
    return [l for l in open(os.path.join(HERE, name + ".txt")).read().splitlines() if l]


def main(out, base_csv, base_bin):
    cand_bin = os.path.join(out, "arachne-candidate")
    sets = {n: set(lines(n)) for n in SIZES}
    if {n: len(s) for n, s in sets.items()} != SIZES or sum(SIZES.values()) != len(set().union(*sets.values())):
        print(f"INSTRUMENT: the registered lists do not have the registered sizes {SIZES} or overlap")
        return 2
    of = {i: n for n, s in sets.items() for i in s}
    base = {r["id"]: r for r in csv.DictReader(open(base_csv))}
    cand = {r["id"]: r for r in csv.DictReader(open(os.path.join(out, "cand", "files.csv")))}
    if set(base) != set(cand) or not set(of) <= set(base):
        print("INSTRUMENT: the base and candidate censuses do not hold the same inputs, or a registered id is missing")
        return 2
    work = os.path.join(out, "d", "twins")
    os.makedirs(work, exist_ok=True)
    result = {}
    for n, i in enumerate(sorted(cand)):
        b, c, kind = base[i], cand[i], of.get(i)
        if kind in ("changed", "described"):
            want, ident = expected(base_bin, c["path"], os.path.join(work, str(n)))
            rc, so, se = describe(cand_bin, c["path"])
            try:
                got = (json.loads(so), json.loads(se[-1])) if rc == 0 and se else None
            except ValueError:
                got = None
            ok = (b["describe_status"] == "ok") == (kind == "changed") and got == (want, ident)
            cls = f"{kind}: as registered" if ok else f"registered {kind} not met"
        elif kind == "refused":
            _, _, sb = describe(base_bin, c["path"])
            rc, so, se = describe(cand_bin, c["path"])
            ok = rc > 0 and not so and bool(se) and bool(sb) and se[0] == sb[0]
            cls = "refused: first line kept" if ok else "registered refusal not met"
        else:
            ok = all(b[k] == c[k] for k in ("describe_status", "describe_sha", "describe_id"))
            cls = "identical" if ok else "differs from 18ccade"
        result[i] = (ok, cls)
    rows = [(i, cls) for i, (_, cls) in sorted(result.items())]
    fails = [f"{i}\t{cls}\n" for i, (ok, cls) in sorted(result.items()) if not ok]
    d = os.path.join(out, "d")
    with open(os.path.join(d, "census.csv"), "w", newline="") as f:
        csv.writer(f).writerows([("id", "class")] + rows)
    open(os.path.join(d, "census-failures.txt"), "w").write("".join(fails))
    counts = {}
    for _, cls in rows:
        counts[cls] = counts.get(cls, 0) + 1
    print("D census: " + json.dumps(dict(sorted(counts.items()))) + f"; failures {len(fails)}")
    return 1 if fails else 0


if __name__ == "__main__":
    if len(sys.argv) != 4:
        sys.exit(__doc__)
    try:
        sys.exit(main(*sys.argv[1:]))
    except (OSError, KeyError, ValueError, subprocess.TimeoutExpired) as e:
        print(f"INSTRUMENT: {e!r}")
        sys.exit(2)
