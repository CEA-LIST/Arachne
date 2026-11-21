"""EC.M1b, O5, the tree (spec: cea-cdrt-knowledge/spec/items/EC.M1b.md).

    python3 -I arachne-cli/tests/ecm1b/tree.py

Reads the worktree (tracked and untracked files, ignored ones aside) against git's a5f3111 (the base)
and ebc3141 (Léo's frozen tip). "Holds his lines" means ebc3141's lines are a subsequence of the
file's: lines were only added, none of his removed or changed.
  T1 his generator: every path of arachne-codegen/src at ebc3141 exists with ebc3141's bytes, except
     lib.rs, codegen/mod.rs and error.rs, which hold his lines;
  T2 his generator replaces ours: none of the paths a5f3111 has there for its generator alone
     (GONE below) exists;
  T3 his CLI and manifests: arachne-cli/src/main.rs, arachne-cli/Cargo.toml and
     arachne-codegen/Cargo.toml hold his lines;
  T4 inline tests: in every .rs file under arachne-codegen/src and arachne-cli/src, the text from the
     first `#[cfg(test)]` line to the end is ebc3141's for a path ebc3141 has, a5f3111's for a path
     only a5f3111 has (semantics.rs: with semantics-tests.patch applied), and absent for a new path;
  T5 examples/: a5f3111's paths and bytes, except README.md (sha256 README_SHA: a5f3111's with his
     run-command hunk) and pub_crawl.ecore (ebc3141's), and no other path;
  T6 every path outside arachne-codegen/, arachne-cli/, examples/, Cargo.toml and Cargo.lock is
     a5f3111's;
  T7 `cargo metadata --offline --locked` exits 0.
Exit 0 all hold, 1 one fails.
"""
import hashlib
import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
BASE, HIS = "a5f3111", "ebc3141"
GROWN = {"arachne-codegen/src/lib.rs", "arachne-codegen/src/codegen/mod.rs", "arachne-codegen/src/error.rs"}
HELD = ["arachne-cli/src/main.rs", "arachne-cli/Cargo.toml", "arachne-codegen/Cargo.toml"]
GONE = ["arachne-codegen/src/codegen/read_as_ecore.rs", "arachne-codegen/src/codegen/reference/containment.rs",
        "arachne-codegen/src/codegen/value.rs", "arachne-codegen/src/deployment.rs",
        "arachne-codegen/src/deployment/conformance.rs"]
SEMANTICS = "arachne-codegen/src/codegen/semantics.rs"
README_SHA = "d59d4f0672e541cb415a12828c008659418363d512f0752c2b5a673fa1d74523"
SCOPE = ("arachne-codegen/", "arachne-cli/", "examples/")


def git(*args):
    return subprocess.run(["git", "-C", REPO, *args], capture_output=True, check=True).stdout


def at(rev, prefix=""):
    """{path: bytes} of every file under prefix at rev."""
    names = git("ls-tree", "-r", "-z", "--name-only", rev, "--", prefix or ".").decode().split("\0")
    return {n: git("show", f"{rev}:{n}") for n in names if n}


def worktree(prefix=""):
    names = git("ls-files", "-z", "-c", "-o", "--exclude-standard", "--", prefix or ".").decode().split("\0")
    out = {}
    for n in names:
        p = os.path.join(REPO, n)
        if n and os.path.isfile(p) and not os.path.islink(p):
            out[n] = open(p, "rb").read()
    return out


def holds(his, ours):
    """His lines are a subsequence of ours (greedy is exact for a subsequence test)."""
    it = iter(ours.splitlines())
    return all(any(line == o for o in it) for line in his.splitlines())


def tests_block(text):
    lines = text.splitlines(keepends=True)
    for i, line in enumerate(lines):
        if line.rstrip(b"\r\n") == b"#[cfg(test)]":
            return b"".join(lines[i:])
    return b""


def patched_semantics(base_text):
    with tempfile.TemporaryDirectory() as tmp:
        path = os.path.join(tmp, "semantics.rs")
        open(path, "wb").write(base_text)
        subprocess.run(["patch", "-s", path, os.path.join(HERE, "semantics-tests.patch")], check=True)
        return open(path, "rb").read()


def main():
    fails = []
    his, base, cand = at(HIS), at(BASE), worktree()

    def check(tag, ok, what):
        if not ok:
            fails.append(f"{tag} {what}")

    # T1, T2
    for p, b in his.items():
        if p.startswith("arachne-codegen/src/"):
            c = cand.get(p)
            if p in GROWN:
                check("T1", c is not None and holds(b, c), f"{p} does not hold ebc3141's lines")
            else:
                check("T1", c == b, f"{p} is not ebc3141's bytes")
    for p in GONE:
        check("T2", p not in cand, f"{p} still exists")
    # T3
    for p in HELD:
        check("T3", p in cand and holds(his[p], cand[p]), f"{p} does not hold ebc3141's lines")
    # T4
    for p, c in cand.items():
        if p.startswith(("arachne-codegen/src/", "arachne-cli/src/")) and p.endswith(".rs"):
            if p in his:
                want, whose = tests_block(his[p]), "ebc3141's"
            elif p == SEMANTICS:
                want, whose = tests_block(patched_semantics(base[p])), "a5f3111's with semantics-tests.patch"
            elif p in base:
                want, whose = tests_block(base[p]), "a5f3111's"
            else:
                want, whose = b"", "none (a new file)"
            check("T4", tests_block(c) == want, f"{p}: inline tests are not {whose}")
    # T5
    ex = {p: b for p, b in cand.items() if p.startswith("examples/")}
    want = {p: b for p, b in base.items() if p.startswith("examples/")}
    want["examples/pub_crawl.ecore"] = his["examples/pub_crawl.ecore"]
    readme = ex.pop("examples/README.md", b"")
    want.pop("examples/README.md")
    check("T5", hashlib.sha256(readme).hexdigest() == README_SHA, "examples/README.md is not the registered bytes")
    for p in sorted(set(ex) | set(want)):
        check("T5", ex.get(p) == want.get(p), f"{p} is not as registered (missing, extra or other bytes)")
    # T6
    outside = lambda d: {p: b for p, b in d.items()
                         if not p.startswith(SCOPE) and p not in ("Cargo.toml", "Cargo.lock")}
    o_c, o_b = outside(cand), outside(base)
    for p in sorted(set(o_c) | set(o_b)):
        check("T6", o_c.get(p) == o_b.get(p), f"{p} differs from a5f3111")
    # T7
    meta = subprocess.run(["cargo", "metadata", "--offline", "--locked", "--format-version", "1"],
                          cwd=REPO, capture_output=True)
    check("T7", meta.returncode == 0, "cargo metadata --offline --locked: " +
          meta.stderr.decode("utf-8", "replace").strip()[-300:])

    for tag in ("T1", "T2", "T3", "T4", "T5", "T6", "T7"):
        n = sum(1 for f in fails if f.startswith(tag + " "))
        print(f"{tag} {'ok' if n == 0 else f'FAIL ({n})'}")
    for f in fails[:60]:
        print("  " + f)
    print("PASS" if not fails else f"FAIL: {len(fails)} findings")
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
