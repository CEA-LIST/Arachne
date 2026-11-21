#!/usr/bin/env python3
"""EC.01 corpus oracle: describe parity, generate parity and the rule census.

Three subcommands, each writing into an output directory, none touching the repository:

  run      one `arachne` binary over a list of .ecore files: `describe` and/or `generate`, each
           under a per-file deadline; per file the status, the SHA-256 of describe's stdout, its
           identity line (the last stderr line), the SHA-256 of the generated tree, the generated
           classifiers.rs and the feature_omitted warnings (kept for the census).
  compare  two `run` outputs side by side: describe parity (reference f90795e against the
           candidate) and generate parity (base ebc3141 against the candidate); writes
           compare-<kind>.csv and summary-<kind>.json and exits 1 when a pass condition fails.
  census   for every feature of every file both describe sides accept and the candidate
           generates, the rule our descriptor writes against the field Léo's generator emits;
           writes census.csv and census-summary.json. A measurement: it never fails on a count.

Statuses: ok, refused (non-zero exit), timeout (deadline hit), crash (killed by a signal).
The spec is cea-cdrt-knowledge/spec/items/EC.01.md.
"""

import argparse
import collections
import concurrent.futures
import csv
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def file_id(path, root):
    rel = os.path.relpath(path, root)
    return rel


def list_inputs(args):
    """(id, path) pairs: every .ecore under each --corpus root (sorted, symlinks not followed,
    directories named `target` skipped, as the v0.8 census did) and every --fixture-dir file."""
    found = []
    for root in args.corpus or []:
        for dirpath, dirnames, filenames in os.walk(root):
            dirnames[:] = sorted(d for d in dirnames if d != "target")
            for name in sorted(filenames):
                if name.endswith(".ecore"):
                    path = os.path.join(dirpath, name)
                    found.append(("modelset/" + os.path.relpath(path, root), path))
    for root in args.fixture_dir or []:
        for name in sorted(os.listdir(root)):
            if name.endswith(".ecore"):
                found.append(("fixture/" + os.path.basename(os.path.normpath(root)) + "/" + name,
                              os.path.join(root, name)))
    return found


def run_bin(cmd, timeout):
    try:
        proc = subprocess.run(cmd, capture_output=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        return "timeout", b"", b""
    if proc.returncode < 0:
        return "crash", proc.stdout, proc.stderr
    if proc.returncode != 0:
        return "refused", proc.stdout, proc.stderr
    return "ok", proc.stdout, proc.stderr


def first_error(stderr):
    text = stderr.decode("utf-8", "replace").strip().splitlines()
    detail = " | ".join(line.strip() for line in text[-3:])
    return detail[:300]


def tree_hash(root):
    h = hashlib.sha256()
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames.sort()
        for name in sorted(filenames):
            path = os.path.join(dirpath, name)
            h.update(os.path.relpath(path, root).encode() + b"\0")
            with open(path, "rb") as f:
                h.update(sha256(f.read()).encode() + b"\n")
    return h.hexdigest()


def one_file(item, args):
    fid, path = item
    row = {"id": fid, "path": path}
    safe = hashlib.sha256(fid.encode()).hexdigest()[:20]
    if args.describe:
        status, out, err = run_bin([args.bin, "describe", path], args.timeout)
        row["describe_status"] = status
        row["describe_sha"] = sha256(out) if status == "ok" else ""
        lines = err.decode("utf-8", "replace").splitlines()
        row["describe_id"] = lines[-1] if status == "ok" and lines else ""
        row["describe_detail"] = "" if status == "ok" else first_error(err)
        if status == "ok" and args.keep_descriptors:
            with open(os.path.join(args.out, "descriptors", safe + ".json"), "wb") as f:
                f.write(out)
    if args.generate:
        tmp = tempfile.mkdtemp(prefix="gen-", dir=args.tmp)
        try:
            status, out, err = run_bin(
                [args.bin, "generate", "--diagnostics-json", "-o", os.path.join(tmp, "p"),
                 "-p", "ec01probe", path],
                args.timeout,
            )
            row["generate_status"] = status
            row["generate_tree"] = tree_hash(os.path.join(tmp, "p")) if status == "ok" else ""
            row["generate_detail"] = ""
            if status != "ok":
                msgs = [json.loads(l) for l in out.decode("utf-8", "replace").splitlines()
                        if l.startswith("{")]
                errs = [m.get("error") or m.get("message", "") for m in msgs
                        if m.get("reason") == "arachne-result" or m.get("level") == "error"]
                row["generate_detail"] = (str(errs[-1]) if errs else first_error(err))[:300]
            if status == "ok" and args.keep_generated:
                dest = os.path.join(args.out, "generated", safe)
                os.makedirs(dest, exist_ok=True)
                shutil.copy(os.path.join(tmp, "p", "src", "classifiers.rs"), dest)
                omitted = [l for l in out.decode("utf-8", "replace").splitlines()
                           if "feature_omitted" in l or "dependency_omitted" in l
                           or "class_omitted" in l]
                with open(os.path.join(dest, "omitted.jsonl"), "w") as f:
                    f.write("\n".join(omitted) + ("\n" if omitted else ""))
        finally:
            shutil.rmtree(tmp, ignore_errors=True)
    row["key"] = safe
    return row


RUN_FIELDS = ["id", "key", "path", "describe_status", "describe_sha", "describe_id",
              "describe_detail", "generate_status", "generate_tree", "generate_detail"]


def cmd_run(args):
    os.makedirs(args.out, exist_ok=True)
    os.makedirs(os.path.join(args.out, "descriptors"), exist_ok=True)
    os.makedirs(os.path.join(args.out, "generated"), exist_ok=True)
    args.tmp = tempfile.mkdtemp(prefix="ec01-", dir=args.tmp_root)
    items = list_inputs(args)
    rows = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
        for row in pool.map(lambda it: one_file(it, args), items):
            rows.append(row)
    shutil.rmtree(args.tmp, ignore_errors=True)
    with open(os.path.join(args.out, "files.csv"), "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=RUN_FIELDS, extrasaction="ignore")
        w.writeheader()
        for row in sorted(rows, key=lambda r: r["id"]):
            w.writerow(row)
    with open(os.path.join(args.out, "manifest.json"), "w") as f:
        json.dump({"bin": args.bin, "bin_sha256": sha256(open(args.bin, "rb").read()),
                   "files": len(rows), "timeout_s": args.timeout, "jobs": args.jobs,
                   "corpus": args.corpus, "fixture_dir": args.fixture_dir,
                   "describe": args.describe, "generate": args.generate}, f, indent=2)
    counts = collections.Counter((r.get("describe_status"), r.get("generate_status")) for r in rows)
    print(json.dumps({"files": len(rows), "statuses": {str(k): v for k, v in counts.items()}}))


def load_run(path):
    with open(os.path.join(path, "files.csv"), newline="") as f:
        return {row["id"]: row for row in csv.DictReader(f)}


ECORE_NS = "http://www.eclipse.org/emf/2002/Ecore"
XSI_TYPE = "{http://www.w3.org/2001/XMLSchema-instance}type"


def stated_change_flags(path):
    """Why the parser fixes may change Léo's generator output on this file, read from the XML:
    `multi-sup` (a class with two or more eSuperTypes: fix 4 may reorder its `_super` fields) and
    `subpackages` (fix 5 resolves `#//` paths inside a subpackage from the document root). Fixes
    1, 2 and 6 act only on files ebc3141 refuses, fix 3 on no field his generator reads."""
    flags = set()
    try:
        tree = ET.parse(path)
    except Exception:
        return {"unreadable"}
    for el in tree.iter():
        tag = el.tag.rsplit("}", 1)[-1]
        if tag == "eSubpackages":
            flags.add("subpackages")
        if tag == "eClassifiers" and len((el.get("eSuperTypes") or "").split()) >= 2:
            flags.add("multi-sup")
    return flags


def cmd_compare(args):
    left, right = load_run(args.left), load_run(args.right)
    kind = args.kind
    stage = "describe" if kind == "describe" else "generate"
    rows, classes = [], collections.Counter()
    by_scope = collections.defaultdict(collections.Counter)
    for fid in sorted(set(left) | set(right)):
        l, r = left.get(fid), right.get(fid)
        if l is None or r is None:
            cls = "missing-on-one-side"
        else:
            ls, rs = l[f"{stage}_status"], r[f"{stage}_status"]
            if ls == "ok" and rs == "ok":
                if kind == "describe":
                    same = l["describe_sha"] == r["describe_sha"] and l["describe_id"] == r["describe_id"]
                    cls = "identical" if same else "differ"
                else:
                    if l["generate_tree"] == r["generate_tree"]:
                        cls = "identical"
                    else:
                        flags = stated_change_flags(r["path"])
                        cls = ("differ-stated:" + "+".join(sorted(flags))) if flags else "differ-unexplained"
            elif ls == "ok":
                cls = f"left-only(right {rs})"
            elif rs == "ok":
                cls = f"right-only(left {ls})"
            else:
                cls = "both-refused"
        scope = fid.split("/", 1)[0]
        classes[cls] += 1
        by_scope[scope][cls] += 1
        rows.append({"id": fid, "class": cls,
                     "left_status": (l or {}).get(f"{stage}_status", ""),
                     "right_status": (r or {}).get(f"{stage}_status", ""),
                     "left_detail": (l or {}).get(f"{stage}_detail", ""),
                     "right_detail": (r or {}).get(f"{stage}_detail", "")})
    os.makedirs(args.out, exist_ok=True)
    with open(os.path.join(args.out, f"compare-{kind}.csv"), "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0].keys()))
        w.writeheader()
        w.writerows(rows)

    def agg(counter):
        out = collections.Counter()
        for cls, n in counter.items():
            out[re.sub(r"\(.*\)", "", cls) if cls.startswith(("left-only", "right-only")) else cls] += n
        return dict(sorted(out.items()))

    ms, fx = by_scope["modelset"], by_scope["fixture"]
    agreed_ms = sum(n for c, n in ms.items() if c in ("identical", "differ") or c.startswith("differ"))
    if kind == "describe":
        failures = {
            # A fixture f90795e refuses (UML.ecore, aas.ecore, Vehicle.ecore) is reported only.
            "fixture_not_identical": sum(
                n for c, n in fx.items()
                if c not in ("identical", "both-refused") and not c.startswith("right-only")),
            "modelset_both_accept_but_differ": ms.get("differ", 0),
            # Revision 1b (Cam, 2026-11-12): every ModelSet file f90795e describes is kept.
            "modelset_lost": sum(n for c, n in ms.items() if c.startswith("left-only")),
        }
    else:
        failures = {
            "fixture_lost_or_unexplained": sum(
                n for c, n in fx.items()
                if c.startswith("left-only") or c == "differ-unexplained"),
            "modelset_lost": sum(n for c, n in ms.items() if c.startswith("left-only")),
            "modelset_differ_unexplained": ms.get("differ-unexplained", 0),
        }
    summary = {
        "kind": kind, "left": args.left, "right": args.right,
        "fixture": agg(fx), "modelset": agg(ms),
        "modelset_agreed_set": agreed_ms,
        "modelset_identical": ms.get("identical", 0),
        "failures": failures,
        "pass": all(v == 0 for v in failures.values()),
    }
    with open(os.path.join(args.out, f"summary-{kind}.json"), "w") as f:
        json.dump(summary, f, indent=2)
    print(json.dumps(summary, indent=2))
    return 0 if summary["pass"] else 1


# ---------------------------------------------------------------- the census

def norm(name):
    return re.sub(r"[^a-z0-9]", "", name.lower())


def split_top(text, sep=","):
    """Split at `sep` outside <>, (), []."""
    parts, depth, cur = [], 0, []
    for ch in text:
        if ch in "<([":
            depth += 1
        elif ch in ">)]":
            depth -= 1
        if ch == sep and depth == 0:
            parts.append("".join(cur))
            cur = []
        else:
            cur.append(ch)
    if "".join(cur).strip():
        parts.append("".join(cur))
    return parts


def squeeze(t):
    t = re.sub(r"\s+", "", t)
    t = t.replace("__classifiers::", "")
    t = re.sub(r"\b(?:[a-z_][a-z0-9_]*::)+", "", t)  # module paths
    return t


def strip_wrappers(t):
    """Box<X> -> X; ProjectedLog<L,n,m> -> L (identity tracking, not a merge choice)."""
    projected = False
    while True:
        m = re.fullmatch(r"Box<(.*)>", t)
        if m:
            t = m.group(1)
            continue
        break
    def proj(match_text):
        nonlocal projected
        out, i = [], 0
        while True:
            j = match_text.find("ProjectedLog<", i)
            if j < 0:
                out.append(match_text[i:])
                break
            out.append(match_text[i:j])
            k, depth = j + len("ProjectedLog<"), 1
            start = k
            while depth:
                if match_text[k] == "<":
                    depth += 1
                elif match_text[k] == ">":
                    depth -= 1
                k += 1
            inner = split_top(match_text[start:k - 1])[0]
            out.append(inner)
            projected = True
            i = k
        return "".join(out)
    t = proj(t)
    t = re.sub(r"Box<([^<>]*)>", r"\1", t)
    return t, projected


def parse_records(path):
    """{norm(record name): {norm(field): log type}} from a generated classifiers.rs."""
    text = open(path).read()
    records = {}
    for m in re.finditer(r"record!\(\s*([A-Za-z0-9_]+)\s*\{(.*?)\}\s*\);", text, re.S):
        name, body = m.group(1), m.group(2)
        fields = {}
        for part in re.split(r",(?=\s*[a-z_][a-z0-9_]*\s*:[^:])", body):
            fm = re.match(r"\s*([a-z_][a-z0-9_]*)\s*:(.*)", part, re.S)
            if not fm:
                continue
            fname, rest = fm.group(1), fm.group(2)
            if "=>" not in rest:
                continue
            log = split_top(rest.split("=>")[0])[-1]
            fields[fname] = squeeze(log)
        records[norm(name)] = fields
    return records


def omitted_features(path):
    """{(norm class, norm feature): [properties]} from the feature_omitted warnings."""
    out = collections.defaultdict(list)
    if not os.path.exists(path):
        return out
    for line in open(path):
        try:
            msg = json.loads(line)
        except Exception:
            continue
        text = msg.get("message", "")
        m = re.search(r"Feature `([^`.]+)\.([^`]+)` has unsupported property `([^`]+)`", text)
        if m:
            out[(norm(m.group(1)), norm(m.group(2)))].append(m.group(3))
            continue
        m = re.search(r"`([^`.]+)\.([^`]+)`", text)
        if m and "dependency_omitted" in msg.get("code", ""):
            out[(norm(m.group(1)), norm(m.group(2)))].append("dependency")
    return out


REG = {"mv": "MVRegister", "lww": "LwwRegister", "fair": "FairRegister",
       "po": "PORegister", "to": "TORegister"}
KEY = {"str": "String", "bool": "bool", "char": "char"}


def leaf_log(leaf):
    k = leaf.get("kind")
    if k == "text":
        return "GraphLog<List<char>>"
    if k == "counter":
        return f"VecLog<Counter<{leaf['num']}>>"
    if k == "flag":
        return "VecLog<EWFlag>" if leaf.get("wins") == "enable" else "VecLog<DWFlag>"
    if k == "register":
        return f"VecLog<{REG[leaf['tie']]}<_>>"
    if k == "enum":
        return f"VecLog<{REG[leaf['tie']]}<{leaf.get('class', '_')}>>"
    if k == "optionalRegister":
        return "VecLog<MVRegister<Option<_>>>"
    return None


def expected_type(merge):
    """The log type our merge rule implies, after ip2's table (semantics.rs at f90795e), with `_`
    for a Rust value type the descriptor does not carry. None when the rule implies no field."""
    kind = merge.get("kind")
    shape = merge.get("shape", {})
    sk = shape.get("kind")
    if kind == "attribute":
        leaf = leaf_log(merge.get("leaf", {}))
        if leaf is None:
            return "?"
        if sk == "single":
            return leaf
        if sk == "optional":
            return f"OptionLog<{leaf}>"
        if sk == "sequence":
            return f"NestedListLog<{leaf}>"
        if sk == "orderedSet":
            return "GraphLog<List<_>>"
        if sk == "set":
            return "VecLog<AWSet<_>>" if shape.get("tie") == "aw" else "VecLog<RWSet<_>>"
        if sk == "bag":
            return "AWBagLog<_>"
        if sk == "keyed":
            key = shape.get("key", {})
            kk = KEY.get(key.get("kind"), key.get("num", "_"))
            return f"UWMapLog<{kk},{leaf}>"
        return "?"
    if kind == "containment":
        log = f"{merge.get('target', '_')}~Log"
        if sk == "single":
            return log
        if sk == "optional":
            return f"OptionLog<{log}>"
        if sk == "sequence":
            return f"NestedListLog<{log}>"
        if sk == "keyed":
            key = shape.get("key", {})
            kk = KEY.get(key.get("kind"), key.get("num", "_"))
            return f"UWMapLog<{kk},{log}>"
        return "?"
    return None


def canon(text):
    """Identifiers compared loosely: case and underscores dropped (the generator writes
    `PureExpKindLog` for a class `Pure_exp`)."""
    return re.sub(r"[A-Za-z0-9_]+", lambda m: m.group(0).replace("_", "").lower(), text)


def type_regex(expected, renamed=False):
    """`_` matches any one type; `Name~Log` matches the class's log or its family's (`Kind`) log.
    With `renamed`, a class or enum name may carry the suffix the generator adds when two
    classifiers collide (`AssignmentKind` emitted as `AssignmentKindModel`): a nominal match."""
    out = []
    suffix = r"(?:model\d*)?" if renamed else ""
    for tok in re.findall(r"[A-Za-z0-9_:]+~Log|[A-Za-z0-9_:]+|[<>,]", expected):
        if tok == "_":
            out.append(r"[^,<>]+(?:<.*?>)?")
        elif tok.endswith("~Log"):
            base = re.escape(canon(tok[:-4].split("::")[-1]))
            out.append(f"{base}{suffix}(?:kind{suffix})?log")
        elif tok in "<>,":
            out.append(re.escape(tok))
        else:
            out.append(re.escape(canon(tok)) + (suffix if tok[:1].isupper() and tok not in KNOWN else ""))
    return re.compile("".join(out) + r"\Z")


KNOWN = {"VecLog", "GraphLog", "List", "Counter", "EWFlag", "DWFlag", "MVRegister", "LwwRegister",
         "FairRegister", "PORegister", "TORegister", "OptionLog", "NestedListLog", "AWSet", "RWSet",
         "AWBagLog", "UWMapLog", "Option", "String"}


def cmd_census(args):
    ref, cand_desc, cand_gen = load_run(args.reference), load_run(args.candidate), load_run(args.generated)
    rows, per_class = [], collections.Counter()
    files = 0
    for fid in sorted(cand_desc):
        r, c, g = ref.get(fid), cand_desc[fid], cand_gen.get(fid)
        if not r or r["describe_status"] != "ok" or c["describe_status"] != "ok":
            continue
        if not g or g["generate_status"] != "ok":
            continue
        dpath = os.path.join(args.candidate, "descriptors", c["key"] + ".json")
        gdir = os.path.join(args.generated, "generated", g["key"])
        if not os.path.exists(dpath) or not os.path.exists(os.path.join(gdir, "classifiers.rs")):
            continue
        files += 1
        desc = json.load(open(dpath))
        records = parse_records(os.path.join(gdir, "classifiers.rs"))
        omitted = omitted_features(os.path.join(gdir, "omitted.jsonl"))
        for cname, cls in sorted(desc.get("classes", {}).items()):
            rec = records.get(norm(cname.split("::")[-1]))
            for group in ("attributes", "containments", "references"):
                for feat in cls.get(group, []):
                    merge = feat.get("merge", {})
                    fname = feat["name"]
                    key = (norm(cname.split("::")[-1]), norm(fname))
                    field = None
                    if rec is not None:
                        for fn, typ in rec.items():
                            if norm(fn) in (norm(fname), norm(fname) + "field"):
                                field = typ
                    skipped = omitted.get(key)
                    ours_unsupported = merge.get("kind") == "unsupported"
                    expected = expected_type(merge)
                    projected = False
                    if field is not None:
                        field, projected = strip_wrappers(field)
                    if cname.startswith("ecore::"):
                        cls_name = "ecore-class"
                    elif rec is None and not ours_unsupported:
                        cls_name = "ours-only:class-not-generated"
                    elif ours_unsupported and (skipped or field is None):
                        cls_name = "both-omit"
                    elif ours_unsupported:
                        cls_name = "his-only:field-ours-unsupported"
                    elif skipped and set(skipped) - {"dependency"}:
                        props = sorted(set(skipped))
                        cls_name = "skip-0d9e646:" + "+".join(props)
                    elif skipped:
                        cls_name = "skip-0d9e646:dependency"
                    elif merge.get("kind") in ("reference", "identityReference"):
                        cls_name = "reference:no-field" if field is None else "reference:his-field"
                    elif field is None:
                        cls_name = "ours-only:no-field"
                    elif expected in (None, "?"):
                        cls_name = "unmapped"
                    elif type_regex(expected).match(canon(field)):
                        cls_name = "agree" + (":projected" if projected else "")
                    elif type_regex(expected, renamed=True).match(canon(field)):
                        cls_name = "agree:nominal-rename" + ("+projected" if projected else "")
                    else:
                        cls_name = "type-or-shape"
                    per_class[cls_name] += 1
                    rows.append({"file": fid, "class": cname, "feature": fname, "group": group,
                                 "census": cls_name, "ours": json.dumps(merge, sort_keys=True),
                                 "expected": expected or "", "his": field or "",
                                 "his_skip": "+".join(skipped or [])})
    os.makedirs(args.out, exist_ok=True)
    with open(os.path.join(args.out, "census.csv"), "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=["file", "class", "feature", "group", "census", "ours",
                                          "expected", "his", "his_skip"])
        w.writeheader()
        w.writerows(rows)
    families = collections.Counter()
    for name, n in per_class.items():
        families[name.split(":")[0]] += n
    pairs = collections.Counter((row["expected"], row["his"]) for row in rows
                                if row["census"] == "type-or-shape")
    summary = {"files": files, "features": len(rows),
               "classes": dict(sorted(per_class.items())),
               "families": dict(sorted(families.items())),
               "type_or_shape_top_pairs": [[e, h, n] for (e, h), n in pairs.most_common(25)]}
    with open(os.path.join(args.out, "census-summary.json"), "w") as f:
        json.dump(summary, f, indent=2)
    print(json.dumps(summary, indent=2))
    return 0


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    r = sub.add_parser("run")
    r.add_argument("--bin", required=True)
    r.add_argument("--out", required=True)
    r.add_argument("--corpus", action="append")
    r.add_argument("--fixture-dir", action="append")
    r.add_argument("--describe", action="store_true")
    r.add_argument("--generate", action="store_true")
    r.add_argument("--keep-descriptors", action="store_true")
    r.add_argument("--keep-generated", action="store_true")
    r.add_argument("--timeout", type=float, default=30)
    r.add_argument("--jobs", type=int, default=4)
    r.add_argument("--tmp-root", default=os.environ.get("EC01_TMP", "/volatile/home/vp242103/.cache/cea-crdt-scratch/EC.01"))
    c = sub.add_parser("compare")
    c.add_argument("--kind", choices=["describe", "generate"], required=True)
    c.add_argument("--left", required=True)
    c.add_argument("--right", required=True)
    c.add_argument("--out", required=True)
    s = sub.add_parser("census")
    s.add_argument("--reference", required=True, help="run dir of f90795e describe")
    s.add_argument("--candidate", required=True, help="run dir of the candidate describe (descriptors kept)")
    s.add_argument("--generated", required=True, help="run dir of the candidate generate (classifiers kept)")
    s.add_argument("--out", required=True)
    args = p.parse_args()
    if args.cmd == "run":
        return cmd_run(args) or 0
    if args.cmd == "compare":
        return cmd_compare(args)
    return cmd_census(args)


if __name__ == "__main__":
    sys.exit(main())
