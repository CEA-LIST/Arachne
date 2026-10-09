"""AR.04: the SysML descriptor with SysON's strings and literal numbers as multi-value registers,
derived from the descriptor of today and SysON's .ecore by Python alone, nothing of Arachne.

  python3 -I registers.py <sysml.ecore> <today.metamodel.json> <out.metamodel.json>

Which entries change is read from the .ecore with Python's XML parser: every EAttribute of an
EClass typed by a string or a number datatype of Ecore that the node replicates (not derived, not
transient, not volatile), plus the three string attributes of Ecore's own classes the descriptor
lists (EAnnotation.source, EStringToStringMapEntry.key and .value, which Arachne builds in, as
EMF's Ecore.ecore declares them), less Comment.body and TextualRepresentation.body. The result
must be the 17 entries the claim names (AR.04 section 3, C1), or the script stops.

Each of those entries gets exactly what an `urn:arachne:semantics` `datatype="mv-register"`
annotation writes on an attribute (the rule `examples/umlclass.ecore` and `examples/notation.ecore`
already show): `"annotation": "mv-register"`, `merge.leaf` `{"kind": "register", "tie": "mv"}`,
and `provenance.leaf` `"annotation"`. Nothing else in the descriptor changes. The file is written
the way `arachne describe` writes one (sorted keys, two-space indent, a final newline), which this
script checks by rendering today's descriptor and comparing bytes. Prints the digests.
"""
import hashlib, json, sys
import xml.etree.ElementTree as ET

T = '{http://www.w3.org/2001/XMLSchema-instance}type'
ECORE = 'http://www.eclipse.org/emf/2002/Ecore#//'
STRING = {'EString'}
NUMBER = {'EByte', 'EShort', 'EInt', 'ELong', 'EFloat', 'EDouble'}
# Ecore's own string attributes on the classes the SysML descriptor lists (Ecore.ecore, as
# arachne-parser/src/repr/ecore.rs builds them): what no SysON EAttribute element declares.
ECORE_BUILTIN = [('ecore::EAnnotation', 'source'), ('ecore::EStringToStringMapEntry', 'key'),
                 ('ecore::EStringToStringMapEntry', 'value')]
KEPT_AS_TEXT = [('Comment', 'body'), ('TextualRepresentation', 'body')]
# The claim's list (AR.04 section 3, C1), written out by hand: 15 strings and 2 numbers.
CLAIMED = sorted([
    ('Element', 'elementId'), ('Element', 'aliasIds'), ('Element', 'declaredName'),
    ('Element', 'declaredShortName'), ('Membership', 'memberName'),
    ('Membership', 'memberShortName'), ('RequirementDefinition', 'reqId'),
    ('RequirementUsage', 'reqId'), ('OperatorExpression', 'operator'),
    ('TextualRepresentation', 'language'), ('Comment', 'locale'), ('LiteralString', 'value'),
    ('ecore::EAnnotation', 'source'), ('ecore::EStringToStringMapEntry', 'key'),
    ('ecore::EStringToStringMapEntry', 'value'),
    ('LiteralInteger', 'value'), ('LiteralRational', 'value'),
])

def render(value):
    return json.dumps(value, sort_keys=True, indent=2, ensure_ascii=False) + '\n'

def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':'),
                                     ensure_ascii=False).encode()).hexdigest()

def replicated_scalars(ecore):
    out = []
    for c in ET.parse(ecore).getroot().iter('eClassifiers'):
        if c.get(T) != 'ecore:EClass':
            continue
        for f in c.findall('eStructuralFeatures'):
            if f.get(T) != 'ecore:EAttribute':
                continue
            etype = f.get('eType') or ''
            if ECORE not in etype:
                continue
            name = etype.split(ECORE, 1)[1]
            if name not in STRING | NUMBER:
                continue
            if any(f.get(k) == 'true' for k in ('derived', 'transient', 'volatile')):
                continue
            out.append((c.get('name'), f.get('name')))
    return out

def main(ecore, today_path, out_path):
    today_bytes = open(today_path, 'rb').read()
    descriptor = json.loads(today_bytes)
    if render(descriptor).encode() != today_bytes:
        sys.exit('set-up: rendering today\'s descriptor does not give back its bytes')
    before = digest(descriptor)
    found = replicated_scalars(ecore) + ECORE_BUILTIN
    changed = sorted(e for e in found if e not in KEPT_AS_TEXT)
    if changed != CLAIMED:
        sys.exit(f'the .ecore gives {changed}, the claim names {CLAIMED}')
    for kept in KEPT_AS_TEXT:
        if kept not in found:
            sys.exit(f'{kept} is not a replicated string of the .ecore')
    for cls, feat in changed:
        hits = [e for e in descriptor['classes'][cls]['attributes'] if e['name'] == feat]
        if len(hits) != 1:
            sys.exit(f'not described once: {cls}.{feat}')
        entry = hits[0]
        if entry['merge']['kind'] != 'attribute' or entry['merge']['leaf']['kind'] not in ('text', 'counter'):
            sys.exit(f'{cls}.{feat} is not a text or counter attribute today: {entry["merge"]}')
        if entry['annotation'] is not None:
            sys.exit(f'{cls}.{feat} carries an annotation today: {entry["annotation"]}')
        entry['annotation'] = 'mv-register'
        entry['merge']['leaf'] = {'kind': 'register', 'tie': 'mv'}
        entry['provenance']['leaf'] = 'annotation'
    with open(out_path, 'w') as fh:
        fh.write(render(descriptor))
    print(json.dumps({'changed': len(changed), 'digest_before': before,
                      'digest_after': digest(descriptor)}))

if __name__ == '__main__':
    main(*sys.argv[1:4])
