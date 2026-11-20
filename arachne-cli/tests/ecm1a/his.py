"""EC.M1 piece 1 round 2, O4: the merged build's refusals against Léo's own ebc3141 build
(spec: cea-cdrt-knowledge/spec/items/EC.M1.md, section 11). his.sh builds the binaries and calls:

    python3 -I arachne-cli/tests/ecm1a/his.py <his-bin> <cand-bin> <files.csv> <out-dir> [<rule>=<bin> ...]

Every input of files.csv (ModelSet and the EC.01 fixtures), 30 s, 3 GiB, no core files, 4 at a time.
A line is the first stderr line as the model editor's language eval reads it
(agent/eval/language.eval.ts:241: ANSI removed, the file's path replaced by `metamodel.ecore`, trimmed).
  his build reads the file:  `parse -q FILE` exits 0 (his CLI has no describe);
  his line, when it does not: `generate -o TMP FILE` with RUST_LOG=error (his CLI through his
    arachne-codegen boundary, as our describe goes through ours);
  the candidate: `describe FILE -o TMP`; its parser alone: `parse -q FILE`.
Judged:
  J1 both refuse: the candidate's line equals his byte for byte, or one <rule>=<bin> build (the
     candidate's tree with that EC.01 rule off, rules-off/<rule>.patch) gives his line exactly:
     that rule read past his first error (allowed, listed by rule); failing any single rule, a
     <rule>+<rule> build (two rules off) that gives it (allowed, listed by pair);
  J2 his parser reads the file and the candidate's parser refuses it;
  J3 every input of subpackage-order.txt (eSubpackages after eClassifiers, 39) is described, but
     the 4 of subpackage-order-no-eclass.txt (the ASMLogo.ecore copies; EC.M1k round 5, registered
     in ecm1k/no-eclass.txt: their root package declares no EClass, which Léo's guard refuses too;
     our earlier description of them read datatypes as classes), which describe refuses with the
     root-class line first and a later line saying `declares no EClass`: 35 of 39 described.
     EC.M1j (spec EC.M1j.md): describe takes roots from the whole package tree, and the 4 ASMLogo
     copies declare their EClasses in a subpackage, so all 39 are described.
Counted: inputs only the candidate reads, by the rule whose off build gives his line (else by the
rules whose off build refuses, else `unattributed`); inputs both parsers read that our describe
refuses after the parse. Writes his.csv and his-failures.txt.
Exit 0 all hold, 1 one fails, 2 the instrument did not run.
"""
import collections
import concurrent.futures as cf
import csv
import os
import re
import resource
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ANSI = re.compile(r"\x1b\[[0-9;]*m")
ENV = dict(os.environ, RUST_LOG="error")


def cap():
    resource.setrlimit(resource.RLIMIT_AS, (3 << 30, 3 << 30))
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))


def run(argv, path):
    """(status, first line): ok, refused, signal or timeout."""
    try:
        p = subprocess.run(argv, capture_output=True, timeout=30, preexec_fn=cap, env=ENV)
    except subprocess.TimeoutExpired:
        return "timeout", "<timeout>"
    if p.returncode == 0:
        return "ok", ""
    text = ANSI.sub("", p.stderr.decode("utf-8", "replace")).replace(path, "metamodel.ecore").strip()
    return ("refused" if p.returncode > 0 else "signal"), (text.splitlines() or [f"<exit {p.returncode}>"])[0]


def main(his, cand, files, out, *offs):
    rows = list(csv.DictReader(open(files)))
    subpkg = set(open(os.path.join(HERE, "subpackage-order.txt")).read().split())
    noecl = set(open(os.path.join(HERE, "subpackage-order-no-eclass.txt")).read().split())
    off = dict(o.split("=", 1) for o in offs)
    bins = [his, cand, *off.values()]
    if not rows or not all(os.access(b, os.X_OK) for b in bins) or len(subpkg) != 39 \
            or len(noecl) != 4 or not noecl <= subpkg or not subpkg <= {r["id"] for r in rows}:
        print("INSTRUMENT: no inputs, a binary missing, or subpackage-order.txt not 39 ids of files.csv")
        return 2
    os.makedirs(out, exist_ok=True)

    def one(r):
        p = r["path"]
        with tempfile.TemporaryDirectory(dir=out) as tmp:
            cs, cl = run([cand, "describe", p, "-o", os.path.join(tmp, "d.json")], p)
            hp, _ = run([his, "parse", "-q", p], p)
            hs, hl = ("ok", "") if hp == "ok" else run([his, "generate", "-o", os.path.join(tmp, "g"), p], p)
            cp = run([cand, "parse", "-q", p], p)[0] if cs != "ok" and hp == "ok" else ""
            offl = {}
            if hp != "ok" and cl != hl:  # J1 not equal, or only the candidate reads: try each rule off
                for rule, b in off.items():
                    offl[rule] = run([b, "describe", p, "-o", os.path.join(tmp, f"{rule}.json")], p)
        return r["id"], cs, cl, hp, hs, hl, cp, offl

    with cf.ThreadPoolExecutor(4) as pool:
        res = sorted(pool.map(one, rows))
    if any(hp != "ok" and hs == "ok" for _, _, _, hp, hs, *_ in res):
        print("INSTRUMENT: his generate read an input his parse refused")
        return 2
    table, fails, counts = [], [], collections.Counter()
    for i, cs, cl, hp, hs, hl, cp, offl in res:
        # one rule off first; a pair only when no single rule gives his line
        explains = [r for r in sorted(offl, key=lambda r: (r.count("+"), r))
                    if offl[r][0] != "ok" and offl[r][1] == hl]
        explains = [r for r in explains if r.count("+") == explains[0].count("+")] if explains else []
        if cs != "ok" and hp != "ok":
            cls = "J1 same" if cl == hl else ("J1 allowed " + "+".join(explains) if explains else "J1 differ")
        elif cs != "ok":
            cls = "J2 candidate parser refuses" if cp != "ok" else "counted: refused by our describe after the parse"
        elif hp != "ok":
            refusing = sorted(r for r, (s, _) in offl.items() if s != "ok" and "+" not in r)
            cls = "counted: only the candidate reads, " + (
                "+".join(explains) if explains else
                ("refused with another line by " + "+".join(refusing) if refusing else "unattributed"))
        else:
            cls = "both read"
        if i in subpkg and cs != "ok":
            cls = "J3 subpackage-order not described; " + cls
        bad = cls.startswith(("J1 differ", "J2", "J3"))
        counts[cls.split(";")[0]] += 1
        if bad:
            fails.append(f"{i}\t{cls}\n")
        table.append([i, cls, cs, hp, cl, hl] + [offl.get(r, ("", ""))[1] for r in off])
    with open(os.path.join(out, "his.csv"), "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["id", "class", "cand_status", "his_parse", "cand_line", "his_line"] + [f"off_{r}" for r in off])
        w.writerows(table)
    open(os.path.join(out, "his-failures.txt"), "w").write("".join(fails))
    for k, n in sorted(counts.items()):
        print(f"{n:6d} {k}")
    described = sum(1 for t in table if t[0] in subpkg and t[2] == "ok")
    print(f"his: {len(res)} inputs, {len(fails)} failures (J1 differ, J2, J3); subpackage-order described {described} of 39")
    return 1 if fails else 0


if __name__ == "__main__":
    if len(sys.argv) < 5:
        sys.exit(__doc__)
    sys.exit(main(*sys.argv[1:]))
