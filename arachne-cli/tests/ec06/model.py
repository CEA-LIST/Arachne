"""EC.06's model (spec: cea-cdrt-knowledge/spec/items/EC.06.md): the `datatypes` key EC.06 adds to a
9908485 descriptor, read from the XML with ElementTree, never from the code under test.

    extend_file(d, path, describe) -> d with `datatypes` (absent when there is none) and each
                                      `dependsOn` digest moved to its dependency's extended descriptor's
    strip(d)                       -> d without `datatypes`
    carried_line(line, plain)      -> whether an EC.04 warning line is one EC.06 takes out of the block

A datatype is an `eClassifiers` element whose xsi:type is `ecore:EDataType`. The described package's
own datatypes, at every subpackage depth, are keyed by their `::` path; a dependency's datatype
`import_class` brings in is keyed `<its file's package>::<name>`. Raises LookupError when the XML
does not map onto the descriptor.
"""
import copy
import importlib.util
import json
import os
import xml.etree.ElementTree as ET

_spec = importlib.util.spec_from_file_location(
    "ec05_model", os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "ec05", "model.py"))
ec05 = importlib.util.module_from_spec(_spec)  # pretty, digest, packages, kids, own_classifiers, dependency_paths
_spec.loader.exec_module(ec05)

pretty, digest, kids, local = ec05.pretty, ec05.digest, ec05.kids, ec05.local
XSI_TYPE = ec05.XSI_TYPE
READ_SOURCES = {"urn:arachne:semantics", "urn:arachne:representation", "subsets", "redefines"}
PLAIN_ATTRS = {XSI_TYPE, "name", "instanceClassName"}


def is_datatype(el):
    return local(el.tag) == "eClassifiers" and (el.get(XSI_TYPE) or "").rsplit(":", 1)[-1] == "EDataType"


def entry(el):
    return {"instanceClassName": el.get("instanceClassName"),
            "serializable": ec05.boolean(el, "serializable", True)}


def plain(el):
    """Nothing of the datatype is left out once name, instanceClassName and serializable are carried:
    no other attribute, and no child but annotations of a source the descriptor reads."""
    return set(el.attrib) <= PLAIN_ATTRS and all(
        local(c.tag) == "eAnnotations" and c.get("source") in READ_SOURCES for c in el)


def strip(d):
    d = copy.deepcopy(d)
    d.pop("datatypes", None)
    return d


def carried_line(line, plain_keys):
    return line.startswith("warning: dropped eDataType at ") and line[len("warning: dropped eDataType at "):] in plain_keys


def described_package(d, path):
    mine = [p for p in ec05.packages(ET.parse(path).getroot())
            if p.get("name") == d["package"] and p.get("nsURI") == d["nsURI"]]
    if len(mine) != 1:
        raise LookupError("the described package")
    return mine[0]


def own_datatypes(d, path):
    own = {}
    ec05.own_classifiers(described_package(d, path), "", own)
    return {k: el for k, el in own.items() if is_datatype(el)}


TYPE_ATTRS = ("eType", "eSuperTypes")


def refs(el_iter):
    """(file, classifier) of every `<file>.ecore#//<name>` word of an eType or eSuperTypes attribute."""
    for el in el_iter:
        for a in TYPE_ATTRS:
            for word in (el.get(a) or "").split():
                f, sep, name = word.partition("#//")
                if sep and f.endswith(".ecore") and ":" not in f and not os.path.isabs(f):
                    yield f, name


def imported_datatypes(path):
    """The datatypes `import_class` copies: each classifier the file names in another file, then
    upward (supertypes) and along structural feature types, in the declaring files."""
    out, seen = {}, set()
    todo = [(os.path.realpath(os.path.join(os.path.dirname(path), f)), n) for f, n in refs(ET.parse(path).getroot().iter())]
    while todo:
        file, name = todo.pop()
        if (file, name) in seen:
            continue
        seen.add((file, name))
        roots = ec05.packages(ET.parse(file).getroot())
        if len(roots) != 1:
            raise LookupError(f"dependency {file}")
        found = [c for c in kids(roots[0], "eClassifiers") if c.get("name") == name]
        if len(found) != 1:
            raise LookupError(f"classifier {name} of {file}")
        c = found[0]
        if is_datatype(c):
            out[f"{roots[0].get('name')}::{name}"] = c
        here = [c] + kids(c, "eStructuralFeatures")
        for el in here:
            for a in TYPE_ATTRS:
                for word in (el.get(a) or "").split():
                    f, sep, n = word.partition("#//")
                    if not sep or ":" in f or (f and not f.endswith(".ecore")):
                        continue
                    if "/" in n:
                        raise LookupError(f"a subpackage path {word}")
                    todo.append((os.path.realpath(os.path.join(os.path.dirname(file), f)) if f else file, n))
    return out


def extend(d, path):
    d = copy.deepcopy(d)
    found = {k: entry(el) for k, el in own_datatypes(d, path).items()}
    if "dependsOn" in d:
        for k, el in imported_datatypes(path).items():
            if k in found:
                raise LookupError(f"datatype key {k} twice")
            found[k] = entry(el)
    if found:
        d["datatypes"] = found
    return d


def extend_file(d, path, describe):
    """extend, and each `dependsOn` digest moved to its dependency's extended descriptor's.
    `describe(path)` gives 9908485's descriptor of a file, or None when it refuses."""
    e = extend(d, path)
    if "dependsOn" not in d:
        return e
    moved = {}
    todo = list(ec05.dependency_paths(path))
    seen = set(todo)
    while todo:
        dep = todo.pop()
        dd = describe(dep)
        if dd is not None:
            moved.setdefault(digest(dd), digest(extend_file(dd, dep, describe)))
        for nxt in ec05.dependency_paths(dep):
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


def plain_keys(d, path):
    """Keys of the own datatypes whose `eDataType` warning line leaves the block."""
    return {k for k, el in own_datatypes(d, path).items() if plain(el)}
