"""EC.M1 piece 1, O3's refusal count (spec: cea-cdrt-knowledge/spec/items/EC.M1.md, Q3 (a):
counted, not judged; run.sh reads exit 1 as a count, exit 2 as the instrument not running).

    python3 -I arachne-cli/tests/ecm1a/refusal.py <ref-bin> <cand-bin> <ref-files.csv> <out-dir>

For every input the reference census refused to describe (`describe_status` not ok in the
reference run's files.csv), runs `describe <file> -o <tmp>` with both binaries (30 s, 3 GiB of
address space, no core files) and compares the first line of stderr as the model editor's
language eval reads it (agent/eval/language.eval.ts:241: ANSI colors removed, the whole text
trimmed). The candidate's line is judged whenever it does not describe the input: a refusal,
a timeout or a signal with another first line is a difference. Inputs the candidate describes
are counted (Q2), not judged. Writes refusal.csv and refusal-failures.txt.
Exit 0 no difference, 1 at least one, 2 the instrument did not run.
"""
import concurrent.futures as cf
import csv
import os
import re
import resource
import subprocess
import sys
import tempfile

ANSI = re.compile(r"\x1b\[[0-9;]*m")


def cap():
    resource.setrlimit(resource.RLIMIT_AS, (3 << 30, 3 << 30))
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))


def first_line(binary, path, tmp):
    try:
        p = subprocess.run([binary, "describe", path, "-o", os.path.join(tmp, "d.json")],
                           capture_output=True, timeout=30, preexec_fn=cap)
    except subprocess.TimeoutExpired:
        return "timeout", "<timeout>"
    if p.returncode == 0:
        return "ok", ""
    text = ANSI.sub("", p.stderr.decode("utf-8", "replace")).strip()
    return ("refused" if p.returncode > 0 else "signal"), (text.splitlines() or [f"<exit {p.returncode}>"])[0]


def main(ref, cand, files, out):
    rows = [r for r in csv.DictReader(open(files)) if r["describe_status"] != "ok"]
    if not rows or not all(os.access(b, os.X_OK) for b in (ref, cand)):
        print("INSTRUMENT: no refused inputs or a binary is missing")
        return 2
    os.makedirs(out, exist_ok=True)

    def one(r):
        with tempfile.TemporaryDirectory(dir=out) as tmp:
            return r["id"], first_line(ref, r["path"], tmp), first_line(cand, r["path"], tmp)

    with cf.ThreadPoolExecutor(4) as pool:
        res = sorted(pool.map(one, rows))
    if any(rs != "refused" for _, (rs, _), _ in res):
        print("INSTRUMENT: the reference did not refuse every input its census refused")
        return 2
    accepted = [i for i, _, (cs, _) in res if cs == "ok"]
    bad = [i for i, (_, rl), (cs, cl) in res if cs != "ok" and cl != rl]
    with open(os.path.join(out, "refusal.csv"), "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["id", "cand_status", "same", "ref_line", "cand_line"])
        for i, (_, rl), (cs, cl) in res:
            w.writerow([i, cs, int(cs == "ok" or cl == rl), rl, cl])
    open(os.path.join(out, "refusal-failures.txt"), "w").write("".join(i + "\n" for i in bad))
    print(f"refusal: {len(res)} inputs f90795e refuses; candidate describes {len(accepted)} (counted); "
          f"both refuse {len(res) - len(accepted)}, first error line differs on {len(bad)}")
    return 1 if bad else 0


if __name__ == "__main__":
    if len(sys.argv) != 5:
        sys.exit(__doc__)
    sys.exit(main(*sys.argv[1:]))
