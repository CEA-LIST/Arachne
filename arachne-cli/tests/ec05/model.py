"""EC.05's model (spec: cea-cdrt-knowledge/spec/items/EC.05.md): the keys EC.05 adds to a 40f4043
descriptor, read from the XML with ElementTree, never from the code under test.

    extend(descriptor, path) -> the descriptor with every added key, from the file `path` and the
                                files it names by relative `.ecore#//` references
    strip(descriptor)        -> the descriptor with every added key removed
    pretty(d), digest(d)     -> serde_json's `{:#}` plus a newline, and `metamodel_digest`
    carried_line(line, text) -> whether an EC.04 warning line names a fact EC.05 carries

Raises LookupError when the XML does not map onto the descriptor (a listed classifier or feature
the files do not declare, a supertype order it cannot place).
"""
import copy
import hashlib
import json
import os
import re
import xml.etree.ElementTree as ET

FEATURE_KEYS = ("lowerBound", "upperBound", "default", "changeable", "unsettable", "eKeys")
CLASS_KEYS = ("interface", "superTypeOrder")
TOP_KEYS = ("nsPrefix", "enumLiterals")
PARTS = ("attributes", "containments", "references")
# EC.04 kinds whose fact EC.05 carries; defaultValueLiteral is carried unless `defaultValue` is set.
CARRIED = {"bounds", "changeable", "eKeys", "interface", "literalValue", "nsPrefix",
           "superTypeOrder", "unsettable"}
XSI_TYPE = "{http://www.w3.org/2001/XMLSchema-instance}type"
REF = re.compile(r"([^\s\"#]+\.ecore)#//")


def local(tag):
    return tag.rsplit("}", 1)[-1]


def kids(el, name):
    return [c for c in el if local(c.tag) == name]


def boolean(el, name, default):
    v = el.get(name)
    return default if v is None else v in ("true", "True", "1")


def pretty(d):
    return (json.dumps(d, indent=2, ensure_ascii=False, sort_keys=True) + "\n").encode()


def digest(d):
    compact = json.dumps(d, separators=(",", ":"), ensure_ascii=False, sort_keys=True)
    return hashlib.sha256(compact.encode()).hexdigest()


def strip(d):
    d = copy.deepcopy(d)
    for k in TOP_KEYS:
        d.pop(k, None)
    for entry in d.get("classes", {}).values():
        for k in CLASS_KEYS:
            entry.pop(k, None)
        for part in PARTS:
            for f in entry.get(part, []):
                for k in FEATURE_KEYS:
                    f.pop(k, None)
    return d


def carried_line(line, text):
    m = re.fullmatch(r"warning: dropped (\S+) at \S+", line)
    if not m:
        return False
    kind = m.group(1)
    return kind in CARRIED or (kind == "defaultValueLiteral"
                               and not re.search(r"\sdefaultValue\s*=", text))


def packages(root):
    return [root] if local(root.tag) == "EPackage" else kids(root, "EPackage")


def own_classifiers(pack, prefix, out):
    for c in kids(pack, "eClassifiers"):
        out.setdefault(prefix + (c.get("name") or ""), c)
    for sub in kids(pack, "eSubpackages") + kids(pack, "EPackage"):
        own_classifiers(sub, prefix + (sub.get("name") or "") + "::", out)


def dependency_paths(path):
    """The files `path` names by relative `.ecore#//` references, directly."""
    found = []
    for rel in sorted(set(REF.findall(open(path, encoding="utf-8", errors="replace").read()))):
        dep = os.path.realpath(os.path.join(os.path.dirname(path), rel))
        if ":" not in rel and not os.path.isabs(rel) and os.path.exists(dep):
            found.append(dep)
    return found


def dependency_roots(path, seen=None):
    """{nsURI: root EPackage} of the files `path` names, transitively."""
    seen = set() if seen is None else seen
    found = {}
    for dep in dependency_paths(path):
        if dep in seen:
            continue
        seen.add(dep)
        for p in packages(ET.parse(dep).getroot()):
            found.setdefault(p.get("nsURI"), p)
        found.update({k: v for k, v in dependency_roots(dep, seen).items() if k not in found})
    return found


def extend_file(d, path, describe):
    """extend, and each `dependsOn` digest moved to its dependency's extended descriptor's.
    `describe(path)` gives 40f4043's descriptor of a file, or None when it refuses."""
    e = extend(d, path)
    if "dependsOn" not in d:
        return e
    # A dependency's own dependencies are listed too: walk the files transitively.
    moved = {}
    todo = list(dependency_paths(path))
    seen = set(todo)
    while todo:
        dep = todo.pop()
        dd = describe(dep)
        if dd is not None:
            moved.setdefault(digest(dd), digest(extend_file(dd, dep, describe)))
        for nxt in dependency_paths(dep):
            if nxt not in seen:
                seen.add(nxt)
                todo.append(nxt)
    ids = []
    for x in d["dependsOn"]:
        if x["digest"] not in moved:
            raise LookupError(f"dependency {x['nsURI']}")
        ids.append({"digest": moved[x["digest"]], "nsURI": x["nsURI"]})
    e["dependsOn"] = sorted(ids, key=lambda x: json.dumps(x, separators=(",", ":"), sort_keys=True))
    return e


def bounds(f):
    lb = int(f.get("lowerBound") or 0)
    ub = f.get("upperBound")
    return lb, (int(ub) if ub is not None else (lb if lb >= 2 else 1))


def super_order(c, names):
    tokens = [t for t in (c.get("eSuperTypes") or "").split() if "#" in t]
    tokens += [e.get("href") or "" for e in kids(c, "eSuperTypes")]
    bare = [t.rsplit("#", 1)[1].rstrip("/").rsplit("/", 1)[-1] for t in tokens]
    by_bare = {}
    for n in names:
        by_bare.setdefault(n.split("::")[-1], []).append(n)
    if sorted(bare) != sorted(n.split("::")[-1] for n in names) or any(len(v) > 1 for v in by_bare.values()):
        raise LookupError(f"supertype order of {c.get('name')}")
    return [by_bare[b][0] for b in bare]


def literal_value(v):
    if v is None:
        return None
    return int(v) if re.fullmatch(r"[+-]?[0-9]+", v) and -2**31 <= int(v) < 2**31 else v


def extend(d, path):
    d = copy.deepcopy(d)
    root = ET.parse(path).getroot()
    mine = [p for p in packages(root) if p.get("name") == d["package"] and p.get("nsURI") == d["nsURI"]]
    if len(mine) != 1:
        raise LookupError("the described package")
    pack = mine[0]
    own = {}
    own_classifiers(pack, "", own)
    deps = None

    def element(key, origin):
        nonlocal deps
        if origin is None and key in own:
            return own[key]
        if deps is None:
            deps = dependency_roots(path)
        cands = [deps[origin]] if origin in deps else [p for p in deps.values()
                                                        if key.split("::", 1)[0] == p.get("name")]
        for p in cands:
            for c in kids(p, "eClassifiers"):
                if c.get("name") == key.split("::")[-1]:
                    return c
        raise LookupError(f"classifier {key}")

    d["nsPrefix"] = pack.get("nsPrefix")
    lits = {}
    for key, names in d["enums"].items():
        c = element(key, None)
        got = [{"name": e.get("name"), "value": literal_value(e.get("value")), "literal": e.get("literal")}
               for e in kids(c, "eLiterals")]
        if [g["name"] for g in got] != names:
            raise LookupError(f"enum {key}")
        lits[key] = got
    d["enumLiterals"] = lits
    for key, entry in d["classes"].items():
        if key.startswith("ecore::"):
            entry["interface"] = False
            entry["superTypeOrder"] = list(entry["superTypes"])
            for part in PARTS:
                for f in entry[part]:
                    f.update(lowerBound=0, upperBound=-1 if f["many"] else 1, default=None,
                             changeable=True, unsettable=False, eKeys=[])
            continue
        c = element(key, entry.get("origin"))
        entry["interface"] = boolean(c, "interface", False)
        entry["superTypeOrder"] = super_order(c, entry["superTypes"])
        declared = {f.get("name"): f for f in kids(c, "eStructuralFeatures")}
        for part in PARTS:
            for f in entry[part]:
                x = declared.get(f["name"])
                if x is None:
                    raise LookupError(f"feature {key}.{f['name']}")
                lb, ub = bounds(x)
                f.update(lowerBound=lb, upperBound=ub, default=x.get("defaultValueLiteral"),
                         changeable=boolean(x, "changeable", True),
                         unsettable=boolean(x, "unsettable", False),
                         eKeys=(x.get("eKeys") or "").split())
    return d
