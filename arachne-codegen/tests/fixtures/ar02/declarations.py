"""AR.02: the declarations an ecore file makes on its references, read with Python's XML parser
and nothing of Arachne, and the descriptor they make when added to the descriptor of today.

  python3 declarations.py <file.ecore> <today.metamodel.json> <out.metamodel.json>

For every EReference (containment or not) of every EClass: `eOpposite` as the opposite's name
(the text after the last `/` of the `eOpposite` attribute), and `subsets` and `redefines` as
lists of {"class", "feature"} objects, one per entry of the `references` attribute of every
EAnnotation whose `source` is that word, in document order. A key is written only when the
ecore declares at least one such thing. Attributes get nothing. Prints the digests.
"""
import hashlib, json, sys
import xml.etree.ElementTree as ET

T = '{http://www.w3.org/2001/XMLSchema-instance}type'

def fragment(uri):
    # `#//Class/feature` -> ("Class", "feature")
    path = uri.split('#//', 1)[1]
    cls, feat = path.rsplit('/', 1)
    return cls, feat

def declarations(ecore):
    out = {}
    for c in ET.parse(ecore).getroot().iter('eClassifiers'):
        if c.get(T) != 'ecore:EClass':
            continue
        for f in c.findall('eStructuralFeatures'):
            if f.get(T) != 'ecore:EReference':
                continue
            keys = {}
            if f.get('eOpposite'):
                keys['eOpposite'] = fragment(f.get('eOpposite'))[1]
            for source in ('subsets', 'redefines'):
                refs = []
                for a in f.findall('eAnnotations'):
                    if a.get('source') == source:
                        for r in (a.get('references') or '').split():
                            cls, feat = fragment(r)
                            refs.append({'class': cls, 'feature': feat})
                if refs:
                    keys[source] = refs
            if keys:
                out[(c.get('name'), f.get('name'))] = keys
    return out

def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':'),
                                     ensure_ascii=False).encode()).hexdigest()

def main(ecore, today, out):
    descriptor = json.load(open(today))
    before = digest(descriptor)
    decl = declarations(ecore)
    placed = 0
    for (cls, feat), keys in decl.items():
        entry = descriptor['classes'].get(cls)
        if entry is None:
            print(f'not described: class {cls}', file=sys.stderr); continue
        hits = [e for k in ('containments', 'references') for e in entry[k] if e['name'] == feat]
        if len(hits) != 1:
            print(f'not described: {cls}.{feat}', file=sys.stderr); continue
        hits[0].update(keys)
        placed += 1
    with open(out, 'w') as fh:
        fh.write(json.dumps(descriptor, sort_keys=True, indent=2, ensure_ascii=False) + '\n')
    print(json.dumps({'declared': len(decl), 'placed': placed,
                      'eOpposite': sum('eOpposite' in k for k in decl.values()),
                      'subsets': sum(len(k.get('subsets', [])) for k in decl.values()),
                      'redefines': sum(len(k.get('redefines', [])) for k in decl.values()),
                      'digest_before': before, 'digest_after': digest(descriptor)}))

if __name__ == '__main__':
    main(*sys.argv[1:4])
