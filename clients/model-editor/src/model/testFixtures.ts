/// <reference types="node" />
/**
 * Fixtures the model-plane tests share: the repository's two descriptors and
 * the digests Rust recorded for them (examples/fixtures/metamodel-digests.json,
 * held to by test mp6), a behaviour-tree document under a header, and the
 * node's wire encoding of a plain document. Test-only: imported by *.test.ts
 * files, never by the app, which is why the node types are scoped here.
 */

import { readFileSync } from 'node:fs';
import type { Descriptor, MetamodelId, ModelHeader, PlainJson, WireNode } from '../api/types';

/** The repository's `examples/`: the descriptors the rig image ships under `/metamodels`. */
export const examples = new URL('../../../../examples/', import.meta.url);

export function readExample(file: string): unknown {
  return JSON.parse(readFileSync(new URL(file, examples), 'utf8'));
}

export const digestFixture = readExample('fixtures/metamodel-digests.json') as Record<string, MetamodelId>;
export const bt = readExample('bt.metamodel.json') as Descriptor;
export const uml = readExample('uml.metamodel.json') as Descriptor;
export const btId: MetamodelId = digestFixture['bt.metamodel.json'];
export const umlId: MetamodelId = digestFixture['uml.metamodel.json'];

/** The validation plan's behaviour-tree model, `a1b2…`. */
export const MODEL_ID = 'a1b2c3d4a1b2c3d4a1b2c3d4a1b2c3d4';
export const btHeader: ModelHeader = { modelId: MODEL_ID, metamodelId: btId };
export const umlHeader: ModelHeader = { modelId: MODEL_ID, metamodelId: umlId };

/** A behaviour-tree document under `header`: a Root, a BehaviorTree, a Sequence. */
export function btDocument(header: ModelHeader | null): PlainJson {
  return {
    ...(header === null ? {} : { __model: header as unknown as PlainJson }),
    eClass: 'Root',
    behaviortrees: [
      {
        eClass: 'BehaviorTree',
        ID: 'main',
        child: { eClass: 'Sequence', name: 'root' },
      },
    ],
  };
}

/** The node's wire encoding of a plain document (strings as char arrays, every node wrapped). */
export function encodeWire(value: PlainJson): WireNode {
  if (value === null) return 'Unset';
  if (typeof value === 'string') return { Value: { String: Array.from(value) } };
  if (typeof value === 'number') return { Value: { Number: value } };
  if (typeof value === 'boolean') return { Value: { Boolean: value } };
  if (Array.isArray(value)) return { Value: { Array: value.map(encodeWire) } };
  return {
    Value: { Object: Object.fromEntries(Object.entries(value).map(([k, v]) => [k, encodeWire(v)])) },
  };
}
