"""EC.M1k, D's judgment (spec: cea-cdrt-knowledge/spec/items/EC.M1k.md).

    python3 -I arachne-cli/tests/ecm1k/census.py <run.sh out-dir> <base files.csv> <base bin>

The base is EC.M1i's assessed run of ecm1b/run.sh at c3dfb6b (its cand/files.csv and its
arachne-candidate, pinned by sha256 in run.sh). A user datatype is a classifier of xsi:type
ecore:EDataType, a zero-literal enum one of xsi:type ecore:EEnum with no eLiterals (read from the
.ecore file, never from the descriptor). Judged per input of the census:
  K1 no-eclass.txt: the candidate refuses, nothing on stdout, the first stderr line is the root-class
     line (`[ERROR arachne] Error: Failed to find a root class in `<package>` package`) and a later
     line says `declares no EClass`;
  K2 no-eclass-refused.txt (refused at c3dfb6b with the root-class line first, root package with no
     EClass): still refused, empty stdout, the first line equal to the base's, a later line says
     `declares no EClass`;
  K3 no-root.txt: refused, empty stdout, first line the root-class line, no line saying
     `declares no EClass`;
  K4 changed.txt: both describe, and the candidate's descriptor equals expected(): the base's with
     the root package's datatypes and zero-literal enums gone from classes and rootClasses, each
     zero-literal enum listed under enums as [], and every attribute of a root-package class typed
     by a datatype (any package of the file) or by Ecore's Object, or by a zero-literal enum,
     rewritten by rewrite(); and (round 5, the orchestrator's ruling: a moved dependency moves its
     importer's dependsOn digest, Mscript.ecore on TypeSystem.ecore) each dependsOn entry whose
     digest is the base identity digest of a file the .ecore references by a relative href that
     is itself in changed.txt or abstract-roots.txt takes that file's candidate identity digest,
     and the input passes only if that file passes its own judgment (K4 or K5); any other
     dependsOn difference fails;
  K5 abstract-roots.txt: the candidate describes; rootClasses is non-empty and every root is an
     abstract class it writes; no datatype or zero-literal enum of the root package is a class or
     root; every zero-literal enum is listed as []; the descriptor is closed; every rewritten
     attribute it writes is what rewrite() makes of it;
  K6 every other input: status, stdout sha256 and identity line equal to the base's.
Writes census.csv, census-failures.txt and attributes.csv (every attribute K4 rewrites, with its
category) in <out-dir>/d. Exit 0 all hold, 1 one fails, 2 the instrument did not run.
"""
import copy
import csv
import json
import os
import re
import resource
import subprocess
import sys
import xml.etree.ElementTree as ET

HERE = os.path.dirname(os.path.abspath(__file__))
ANSI = re.compile(r"\x1b\[[0-9;]*m")
XSI = "{http://www.w3.org/2001/XMLSchema-instance}type"
ROOT_LINE = "[ERROR arachne] Error: Failed to find a root class in `"
NO_ECLASS = "declares no EClass"
OBJECT = "http://www.eclipse.org/emf/2002/Ecore#//Object"

FLAG = {"kind": "flag", "wins": "enable"}
MV = {"kind": "register", "tie": "mv"}


def counter(num):
    return {"kind": "counter", "num": num, "resettable": True}


# EMF's instance class -> the built-in's (kind, leaf, leaf provenance); a wrapper takes its primitive's.
BUILTIN = {}
for names, rule in [
    (("boolean", "java.lang.Boolean"), ("bool", FLAG, "houseDefault")),
    (("byte", "java.lang.Byte"), ("int", counter("i8"), "houseDefault")),
    (("short", "java.lang.Short"), ("int", counter("i16"), "houseDefault")),
    (("int", "java.lang.Integer"), ("int", counter("i32"), "houseDefault")),
    (("long", "java.lang.Long"), ("int", counter("i64"), "houseDefault")),
    (("float", "java.lang.Float"), ("float", counter("f32"), "houseDefault")),
    (("double", "java.lang.Double"), ("float", counter("f64"), "houseDefault")),
    (("char", "java.lang.Character"), ("string", MV, "houseDefault")),
    (("java.lang.String",), ("string", {"kind": "text"}, "declared")),
]:
    for name in names:
        BUILTIN[name] = rule
UNMAPPED = ("string", MV, "houseDefault")


class Unmodeled(Exception):
    """An input this judge has no rule for (registered as an instrument failure, exit 2)."""


def cap():
    resource.setrlimit(resource.RLIMIT_AS, (3 << 30, 3 << 30))
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))


def describe(binary, path):
    p = subprocess.run([binary, "describe", path], capture_output=True, timeout=60, preexec_fn=cap)
    return p.returncode, p.stdout, ANSI.sub("", p.stderr.decode("utf-8", "replace")).splitlines()


def tag(e):
    return e.tag.rsplit("}", 1)[-1]


def local(t):
    return (t or "").rsplit(":", 1)[-1]


def root_package(path):
    """The document's root EPackage element, or None when the file is not XML ElementTree reads."""
    try:
        root = ET.parse(path).getroot()
    except ET.ParseError:
        return None
    if tag(root) == "EPackage":
        return root
    return next((e for e in root if tag(e) == "EPackage"), None)


def is_moved(e):
    """A user datatype or a zero-literal enum."""
    kind = local(e.get(XSI))
    return kind == "EDataType" or (kind == "EEnum" and not any(tag(l) == "eLiterals" for l in e))


def instance_class(e):
    """EMF's instance class name of a datatype: instanceClassName, else instanceTypeName's erasure."""
    if e.get("instanceClassName") is not None:
        return e.get("instanceClassName")
    typ = e.get("instanceTypeName")
    return typ.split("<", 1)[0].strip() if typ is not None else None


def scan(pkg):
    """(moved root-package names, zero-literal enum names, {class: {attribute: target element}}).

    A target is a moved classifier of any package of the file, by its `#//a/b/Name` path, or the
    string OBJECT for Ecore's Object."""
    paths = {}

    def walk(p, prefix):
        for e in p:
            if tag(e) == "eClassifiers":
                paths[prefix + (e.get("name"),)] = e
            elif tag(e) == "eSubpackages":
                walk(e, prefix + (e.get("name"),))

    walk(pkg, ())
    own = [e for e in pkg if tag(e) == "eClassifiers"]
    moved = {e.get("name") for e in own if is_moved(e)}
    empty = {e.get("name") for e in own if local(e.get(XSI)) == "EEnum" and e.get("name") in moved}
    attrs = {}
    for c in own:
        if local(c.get(XSI)) != "EClass":
            continue
        for f in c:
            if tag(f) != "eStructuralFeatures" or local(f.get(XSI)) != "EAttribute":
                continue
            etype = f.get("eType")
            if etype is None:
                g = next((x for x in f if tag(x) == "eGenericType"), None)
                etype = g.get("eClassifier") if g is not None else None
            if not etype:
                continue
            if etype.endswith(OBJECT):
                attrs.setdefault(c.get("name"), {})[f.get("name")] = OBJECT
            elif etype.startswith("#//"):
                t = paths.get(tuple(etype[3:].split("/")))
                if t is not None and is_moved(t):
                    attrs.setdefault(c.get("name"), {})[f.get("name")] = t
    return moved, empty, attrs


def rewrite(entry, target):
    """The attribute entry as EC.M1k writes it, and its category."""
    entry = copy.deepcopy(entry)
    merge, prov = entry["merge"], entry["provenance"]
    if prov.get("leaf") == "annotation":
        raise Unmodeled("an annotation-governed attribute typed by a datatype or empty enum")
    if target is not OBJECT and local(target.get(XSI)) == "EEnum":
        name = target.get("name")
        entry["kind"], entry["enum"] = "enum", name
        leaf, source, category = {"kind": "enum", "class": name, "tie": "mv"}, "houseDefault", "empty enum"
    else:
        ic = "java.lang.Object" if target is OBJECT else instance_class(target)
        kind, leaf, source = BUILTIN.get(ic, UNMAPPED)
        entry["kind"] = kind
        category = ("Ecore Object" if target is OBJECT else
                    "unmapped" if ic not in BUILTIN else
                    "String" if ic == "java.lang.String" else
                    "wrapper" if "." in ic else "primitive")
    if merge["kind"] == "attribute":
        merge["leaf"], prov["leaf"] = leaf, source
    else:
        category += ", rule unsupported (kind only)"
    return entry, category


def expected(base, pkg):
    """(the descriptor K4 expects, [(class, attribute, category)])."""
    moved, empty, attrs = scan(pkg)
    d = copy.deepcopy(base)
    for name in moved:
        d["classes"].pop(name, None)
    d["rootClasses"] = [r for r in d["rootClasses"] if r not in moved]
    for name in empty:
        d["enums"][name] = []
    done = []
    for cname, features in sorted(attrs.items()):
        cls = d["classes"].get(cname)
        if cls is None:
            continue
        for i, a in enumerate(cls["attributes"]):
            if a["name"] in features:
                cls["attributes"][i], category = rewrite(a, features[a["name"]])
                done.append((cname, a["name"], category))
    return d, done


def undefined(d):
    classes, enums = d["classes"], d["enums"]
    bad = []
    for c in classes.values():
        bad += [s for s in c["superTypes"] if s not in classes]
        for f in c["containments"] + c["references"]:
            bad += [n for n in (f["target"], f["merge"].get("target")) if n is not None and n not in classes]
        for a in c["attributes"]:
            if "enum" in a and a["enum"] not in enums:
                bad.append(a["enum"])
            leaf = a["merge"].get("leaf") or {}
            if "class" in leaf and leaf["class"] not in classes and leaf["class"] not in enums:
                bad.append(leaf["class"])
    return set(bad)


def abstract_roots_hold(d, pkg):
    moved, empty, attrs = scan(pkg)
    classes = d["classes"]
    ok = bool(d["rootClasses"]) and all(classes.get(r, {}).get("abstract") is True for r in d["rootClasses"])
    ok = ok and not (moved & (set(classes) | set(d["rootClasses"])))
    ok = ok and all(d["enums"].get(n) == [] for n in empty) and not undefined(d)
    for cname, features in attrs.items():
        for a in classes.get(cname, {}).get("attributes", []):
            if a["name"] in features and rewrite(a, features[a["name"]])[0] != a:
                ok = False
    return ok


def referenced(path):
    """The files a .ecore names by a relative href (`../x/Other.ecore#//A`), as normalized paths."""
    found = set()
    for e in ET.parse(path).iter():
        for v in e.attrib.values():
            for tok in v.split():
                ref = tok.split("#", 1)[0]
                if "#" in tok and ref.endswith(".ecore") and ":" not in ref:
                    found.add(os.path.normpath(os.path.join(os.path.dirname(path), ref)))
    return found


def digest(row):
    return json.loads(row["describe_id"])["digest"] if row["describe_id"] else None


def lines(name):
    return [l for l in open(os.path.join(HERE, name)).read().splitlines() if l]


def refused(rc, out, err, first=None, cause=True):
    """Refused with nothing on stdout, the first stderr line `first` (else the root-class line), and
    a later line saying `declares no EClass` when `cause`, no such line anywhere when not."""
    if rc <= 0 or out or not err:
        return False
    if not (err[0].startswith(ROOT_LINE) if first is None else err[0] == first):
        return False
    if cause:
        return any(NO_ECLASS in l for l in err[1:])
    return not any(NO_ECLASS in l for l in err)


def main(out, base_csv, base_bin):
    cand_bin = os.path.join(out, "arachne-candidate")
    sets = {n: set(lines(n + ".txt")) for n in
            ("no-eclass", "no-eclass-refused", "no-root", "changed", "abstract-roots")}
    sizes = {n: len(s) for n, s in sets.items()}
    if sizes != {"no-eclass": 34, "no-eclass-refused": 68, "no-root": 7, "changed": 141, "abstract-roots": 6} \
            or sum(sizes.values()) != len(set().union(*sets.values())):
        print(f"INSTRUMENT: the registered lists do not have the registered sizes or overlap: {sizes}")
        return 2
    of = {i: n for n, s in sets.items() for i in s}
    base = {r["id"]: r for r in csv.DictReader(open(base_csv))}
    cand = {r["id"]: r for r in csv.DictReader(open(os.path.join(out, "cand", "files.csv")))}
    if set(base) != set(cand) or not set(of) <= set(base):
        print("INSTRUMENT: the base and candidate censuses do not hold the same inputs, or a registered id is missing")
        return 2
    movable = {base[j]["path"]: j for j in sets["changed"] | sets["abstract-roots"]}
    result, needs, rewritten = {}, {}, []
    for i in sorted(cand):
        b, c, kind = base[i], cand[i], of.get(i)
        if kind in ("no-eclass", "no-root"):
            rc, so, se = describe(cand_bin, c["path"])
            ok = refused(rc, so, se, cause=kind == "no-eclass")
            cls = f"refused: {kind}" if ok else f"registered {kind} refusal not met"
        elif kind == "no-eclass-refused":
            _, _, sb = describe(base_bin, c["path"])
            rc, so, se = describe(cand_bin, c["path"])
            ok = bool(sb) and refused(rc, so, se, first=sb[0])
            cls = "refused: cause added" if ok else "registered cause not added"
        elif kind in ("changed", "abstract-roots"):
            rb, ob, _ = describe(base_bin, c["path"])
            rc, oc, _ = describe(cand_bin, c["path"])
            pkg = root_package(c["path"])
            ok = rb == 0 and rc == 0 and pkg is not None
            if ok and kind == "changed":
                want, done = expected(json.loads(ob), pkg)
                deps = {digest(base[j]): j for j in map(movable.get, referenced(c["path"])) if j}
                for dep in want.get("dependsOn", []):
                    j = deps.get(dep["digest"])
                    if j is not None:
                        dep["digest"] = digest(cand[j])
                        needs.setdefault(i, []).append(j)
                ok = json.loads(oc) == want
                rewritten += [(i, *x) for x in done]
            elif ok:
                ok = abstract_roots_hold(json.loads(oc), pkg)
            cls = f"{kind}: as registered" if ok else f"registered {kind} not met"
        else:
            ok = all(b[k] == c[k] for k in ("describe_status", "describe_sha", "describe_id"))
            cls = "identical" if ok else "differs from c3dfb6b"
        result[i] = (ok, cls)
    for i, js in needs.items():  # K4: a moved dependency must pass its own judgment
        if result[i][0] and not all(result[j][0] for j in js):
            result[i] = (False, "registered changed not met: a moved dependency failed")
    rows = [(i, cls) for i, (_, cls) in sorted(result.items())]
    fails = [f"{i}\t{cls}\n" for i, (ok, cls) in sorted(result.items()) if not ok]
    d = os.path.join(out, "d")
    os.makedirs(d, exist_ok=True)
    with open(os.path.join(d, "census.csv"), "w", newline="") as f:
        csv.writer(f).writerows([("id", "class")] + rows)
    with open(os.path.join(d, "attributes.csv"), "w", newline="") as f:
        csv.writer(f).writerows([("id", "class", "attribute", "category")] + rewritten)
    open(os.path.join(d, "census-failures.txt"), "w").write("".join(fails))
    counts, cats = {}, {}
    for _, cls in rows:
        counts[cls] = counts.get(cls, 0) + 1
    for *_, cat in rewritten:
        cats[cat] = cats.get(cat, 0) + 1
    print("D attributes rewritten: " + json.dumps(dict(sorted(cats.items()))) + f"; total {len(rewritten)}")
    print("D census: " + json.dumps(dict(sorted(counts.items()))) + f"; failures {len(fails)}")
    return 1 if fails else 0


if __name__ == "__main__":
    if len(sys.argv) != 4:
        sys.exit(__doc__)
    try:
        sys.exit(main(*sys.argv[1:]))
    except (OSError, KeyError, ValueError, Unmodeled, subprocess.TimeoutExpired) as e:
        print(f"INSTRUMENT: {e!r}")
        sys.exit(2)
