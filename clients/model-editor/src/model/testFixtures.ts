/// <reference types="node" />
/**
 * Fixtures the model-plane tests share: the repository's two descriptors and
 * the digests Rust recorded for them (examples/fixtures/metamodel-digests.json,
 * held to by test mp6), and a behaviour-tree document under a header.
 * Test-only: imported by *.test.ts files, never by the app.
 */

import { readFileSync } from 'node:fs';
import type { Descriptor, MetamodelId, ModelHeader, PlainJson } from '../api/types';

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
