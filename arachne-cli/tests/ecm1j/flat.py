"""EC.M1j, the flat twin of an Ecore package tree (spec: cea-cdrt-knowledge/spec/items/EC.M1j.md).

    python3 -I arachne-cli/tests/ecm1j/flat.py <in.ecore> <out.ecore>   (writes the twin, prints the map)

The twin moves every classifier of every subpackage into the root package under the mangled name
`a__b__Name` and rewrites every in-document reference `#//a/b/Name[/...]` to `#//a__b__Name[/...]`,
the walk the parser takes (package names from the root package, then one classifier). Describing
the twin with a build that describes one package (18ccade) and renaming `a__b__Name` back to
`a::b::Name` gives the descriptor EC.M1j's describe must write for the tree, plus the `packages`
map read from the file. `skip` names what this model does not cover; such an input is judged by
properties only.
"""
import hashlib
import json
import sys
import xml.etree.ElementTree as ET

XSI = "{http://www.w3.org/2001/XMLSchema-instance}type"
# Names the parser reads as Ecore built-ins whatever the package (builtin.rs FromStr).
BUILTIN = {"Object", "EByte", "EShort", "EInt", "ELong", "EFloat", "EDouble", "EBoolean", "EChar", "EString"}
SEP = "__"
NAMED = ("target", "enum", "class")  # keys whose string value is a classifier name
LISTED = ("rootClasses", "superTypes")


def tag(e):
    return e.tag.rsplit("}", 1)[-1]


def is_sub(e):
    """A subpackage: `eSubpackages`, or a nested `ecore:EPackage` element, which the parser reads alike."""
    return tag(e) in ("eSubpackages", "EPackage")


class Tree:
    """The root package of one .ecore file and its subpackages, by path (a tuple of names)."""

    def __init__(self, path):
        for _, (prefix, uri) in ET.iterparse(path, events=("start-ns",)):
            if prefix:
                ET.register_namespace(prefix, uri)
        self.doc = ET.parse(path)
        top = self.doc.getroot()
        packs = [top] if tag(top) == "EPackage" else [e for e in top if tag(e) == "EPackage"]
        self.skip = []
        if len(packs) != 1:
            self.skip.append("not one root package")
        self.root = packs[0] if packs else None
        self.subs, self.classifiers = {}, {}
        if self.root is not None:
            self._walk(self.root, ())

    def _walk(self, pack, path):
        names = [e.get("name") for e in pack if tag(e) == "eClassifiers"]
        self.classifiers[path] = {e.get("name"): e for e in pack if tag(e) == "eClassifiers"}
        if len(set(names)) != len(names):
            self.skip.append(f"a classifier name repeats in {'::'.join(path) or 'the root'}")
        for sub in (e for e in pack if is_sub(e)):
            p = path + (sub.get("name"),)
            if p in self.subs:
                self.skip.append(f"subpackage {'::'.join(p)} twice")
            if sub.get("name") in self.classifiers[path]:
                self.skip.append(f"subpackage {'::'.join(p)} named like a classifier")
            self.subs[p] = sub
            self._walk(sub, p)

    def sub_classifiers(self):
        return [(p, n) for p, cs in self.classifiers.items() if p for n in cs]

    def packages(self):
        """Every subpackage of the tree, at every depth, with its nsURI and nsPrefix."""
        return {"::".join(p): {"nsURI": s.get("nsURI"), "nsPrefix": s.get("nsPrefix")}
                for p, s in self.subs.items()}

    def rename(self):
        return {SEP.join(p + (n,)): "::".join(p + (n,)) for p, n in self.sub_classifiers()}

    def resolve(self, word):
        """`#//a/b/Name/rest` with (a, b) a subpackage and Name its classifier -> the twin's word."""
        segs = word[3:].split("/")
        path, i = (), 0
        while i < len(segs) - 1 and path + (segs[i],) in self.subs:
            path, i = path + (segs[i],), i + 1
        if not path:
            return word
        if segs[i] not in self.classifiers[path]:
            self.skip.append(f"unresolved reference {word}")
            return word
        return "#//" + "/".join([SEP.join(path + (segs[i],))] + segs[i + 1:])

    def flatten(self, out):
        """Write the twin to `out`; returns {mangled: a::b::Name}."""
        names = self.rename()
        own = set(self.classifiers.get((), {}))
        if any(SEP in n for p, cs in self.classifiers.items() for n in cs) or own & set(names):
            self.skip.append("a classifier name holds the mangling separator")
        if any(n in BUILTIN and self.classifiers[p][n].get(XSI) != "ecore:EClass" for p, n in self.sub_classifiers()):
            self.skip.append("a subpackage datatype is named like an Ecore built-in")
        uris = {s.get("nsURI") for s in self.subs.values() if s.get("nsURI")}
        for e in self.doc.iter():
            for k, v in list(e.attrib.items()):
                words = v.split()
                if any("#//" in w and w.split("#//", 1)[0] in uris for w in words):
                    self.skip.append("a reference by a subpackage's nsURI")
                if any(w.startswith("#//") for w in words):
                    new = [self.resolve(w) if w.startswith("#//") else w for w in words]
                    if new != words:
                        e.set(k, " ".join(new))
        for path, sub in self.subs.items():
            for anno in (e for e in sub if tag(e) == "eAnnotations"):
                if (anno.get("source") or "").startswith("urn:arachne:"):
                    self.skip.append("an arachne annotation on a subpackage")
        for (path, name) in self.sub_classifiers():
            c = self.classifiers[path][name]
            c.set("name", SEP.join(path + (name,)))
        moved = [self.classifiers[p][n] for p, n in self.sub_classifiers()]
        for sub in [e for e in self.root if is_sub(e)]:
            self.root.remove(sub)
        at = max([i + 1 for i, e in enumerate(self.root) if tag(e) == "eClassifiers"] or [len(self.root)])
        for k, c in enumerate(moved):
            self.root.insert(at + k, c)
        self.doc.write(out, encoding="utf-8", xml_declaration=True)
        return names


def expected(flat_descriptor, names, packages):
    """The tree's descriptor from the twin's: names renamed, lists re-sorted, `packages` added when a
    subpackage classifier is written."""

    def walk(v, key=None):
        if isinstance(v, dict):
            return {names.get(k, k) if key in ("classes", "enums") else k: walk(x, k) for k, x in v.items()}
        if isinstance(v, list):
            out = [walk(x, key) for x in v]
            return sorted(out) if key in LISTED else out
        if isinstance(v, str) and key in NAMED + LISTED:
            return names.get(v, v)
        return v

    d = walk(flat_descriptor)
    written = set(names.values()) & (set(d["classes"]) | set(d["enums"]))
    if written:
        d["packages"] = packages
    return d, bool(written)


def digest(descriptor):
    """metamodel_digest: SHA-256 of serde_json's compact, key-sorted serialization."""
    text = json.dumps(descriptor, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    t = Tree(sys.argv[1])
    print(json.dumps({"rename": t.flatten(sys.argv[2]), "packages": t.packages(), "skip": t.skip}, indent=1))
