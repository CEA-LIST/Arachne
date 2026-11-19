"""EC.M1 piece 1, O1, the merged tree (spec: cea-cdrt-knowledge/spec/items/EC.M1.md, section 4).

    python3 -I arachne-cli/tests/ecm1a/tree.py

Reads the working tree (tracked and untracked files, ignored ones left out), so it runs on the
uncommitted merge and on the merge commit alike. Checks:
  T1 no unmerged path;
  T2 arachne-parser: every path outside tests/ is 21dbe96's (ebc3141 plus EC.01's parser commit);
  T3 arachne-xmi: every path is ebc3141's;
  T4 arachne-codegen and arachne-cli: every path outside tests/ is f90795e's, except
     arachne-codegen/src/resources.rs and arachne-codegen/Cargo.toml (present, content free);
     none of the test files ebc3141 adds there exists (Q3 (a): no refusal mapping, so
     parser.rs and error.rs are f90795e's byte for byte);
  T5 every other path is f90795e's, examples/ included (our four examples/ tests pin its set),
     except: Cargo.toml (f90795e's plus the arachne-xmi member), Cargo.lock (T6), and
     ebc3141's README.md, REQUIREMENTS.md, modelset_coverage.py and
     modelset-results/MODELSET_COVERAGE_REPORT.md;
  T6 cargo metadata --offline --locked succeeds.
Exit 0 when all hold, 1 otherwise.
"""
import os
import subprocess
import sys

OURS, HIS, GUIDE = "f90795e", "ebc3141", "21dbe96"
REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "../../.."))
TESTS = ("arachne-parser/tests/", "arachne-codegen/tests/", "arachne-cli/tests/")
FREE = {"arachne-codegen/src/resources.rs", "arachne-codegen/Cargo.toml", "Cargo.lock"}
FROM_HIS = {"README.md", "REQUIREMENTS.md", "modelset_coverage.py",
            "modelset-results/MODELSET_COVERAGE_REPORT.md"}


def git(*args, check=True):
    return subprocess.run(["git", "-C", REPO, *args], capture_output=True, check=check).stdout


def tree(rev, *paths):
    """{path: blob sha} of rev, limited to paths."""
    out = git("ls-tree", "-r", "-z", rev, "--", *paths).split(b"\0")
    return {e.split(b"\t", 1)[1].decode(): e.split()[2].decode() for e in out if e}


def blob(data):
    return subprocess.run(["git", "hash-object", "--stdin"], input=data, capture_output=True,
                          check=True).stdout.decode().strip()


def main():
    status = 0

    def report(name, bad):
        nonlocal status
        if bad:
            status = 1
            print(f"{name} FAIL ({len(bad)} paths)")
            for line in sorted(bad)[:20]:
                print("  " + line)
        else:
            print(f"{name} ok")

    report("T1", sorted({l.split(b"\t", 1)[1].decode()
                         for l in git("ls-files", "-u", "-z").split(b"\0") if l}))

    on_disk = {p.decode() for p in git("ls-files", "-co", "--exclude-standard", "-z").split(b"\0") if p}
    on_disk = {p for p in on_disk if os.path.lexists(os.path.join(REPO, p))}
    actual = {}

    def same(path, sha):
        if path not in actual:
            with open(os.path.join(REPO, path), "rb") as f:
                actual[path] = blob(f.read())
        return actual[path] == sha

    def compare(name, expected, scope):
        """expected: {path: sha or None (present, content free)}; scope: the disk paths judged."""
        bad = [f"missing {p}" for p in expected if p not in on_disk]
        bad += [f"extra {p}" for p in scope if p not in expected]
        bad += [f"differs {p}" for p, sha in expected.items()
                if p in on_disk and sha is not None and not same(p, sha)]
        report(name, bad)

    def judged(prefixes):
        return {p for p in on_disk if p.startswith(prefixes) and not p.startswith(TESTS)}

    exp = {p: s for p, s in tree(GUIDE, "arachne-parser").items() if not p.startswith(TESTS)}
    compare("T2", exp, judged(("arachne-parser/",)))

    compare("T3", tree(HIS, "arachne-xmi"), judged(("arachne-xmi/",)))

    exp = {p: (None if p in FREE else s)
           for p, s in tree(OURS, "arachne-codegen", "arachne-cli").items() if not p.startswith(TESTS)}
    added = [p for p in git("diff", "--name-only", "--diff-filter=A", OURS, HIS, "--",
                            "arachne-codegen", "arachne-cli").decode().split()
             if p.startswith(TESTS) and p in on_disk]
    compare("T4", exp, judged(("arachne-codegen/", "arachne-cli/")) | set(added))

    crates = ("arachne-parser/", "arachne-xmi/", "arachne-codegen/", "arachne-cli/")
    exp = {p: s for p, s in tree(OURS).items() if not p.startswith(crates)}
    exp.update({p: s for p, s in tree(HIS, *FROM_HIS).items()})
    exp["Cargo.lock"] = None
    toml = git("show", f"{OURS}:Cargo.toml").replace(
        b'"arachne-parser"]', b'"arachne-parser", "arachne-xmi"]', 1)
    exp["Cargo.toml"] = blob(toml)
    compare("T5", exp, {p for p in on_disk if not p.startswith(crates)})

    meta = subprocess.run(["cargo", "metadata", "--offline", "--locked", "--format-version", "1"],
                          cwd=REPO, capture_output=True)
    report("T6", [] if meta.returncode == 0 else
           [l for l in meta.stderr.decode("utf-8", "replace").splitlines() if l.strip()][:5])

    print("PASS" if status == 0 else "FAIL")
    return status


if __name__ == "__main__":
    sys.exit(main())
