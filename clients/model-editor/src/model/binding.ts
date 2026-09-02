/**
 * The binding check at apply: a fetched state reaches the model view only
 * under the metamodel its own header names.
 *
 * A model carries `__model` ({modelId, metamodelId: {nsURI, digest}}), written
 * once by the node that created it (model/instance.ts). Before the editor
 * renders a state it hashes the descriptor the document would be rendered
 * under, with the rule the node uses (model/digest.ts), and compares that
 * digest with the header's. The digest is always computed over the
 * descriptor's own bytes and never read from a label the node reports, so a
 * node that serves the wrong file under the right name is caught as well.
 *
 * The bad day this prevents, seen on the rig: the editor opens a behaviour-tree
 * model on a node that serves the SimpleUML descriptor for it, `Sequence` and
 * `Fallback` match no class there, and the tree renders as blank rows with no
 * error anywhere. Here that is a refusal naming both pairs.
 *
 * Outcomes:
 * - `unbound`: no header. Every step-1 log is one; it is applied under the
 *   loaded descriptor exactly as before this check existed.
 * - `bound`: header and descriptor agree by digest.
 * - `no-descriptor`: a header, and nothing to check it against (the node serves
 *   none and no file is loaded). The document is applied: with no descriptor
 *   nothing renders, so nothing can be mis-rendered, and the next descriptor
 *   loaded is checked on the next poll.
 * - `mismatch`: header and descriptor disagree. Refused.
 * - `header-rewritten`: the header differs from the one recorded at the first
 *   apply. The header is immutable by rule, so a peer has ignored it. Refused.
 *   This is the comparison that still holds when a node serves whatever the
 *   header happens to name.
 *
 * Pure: no React, no I/O beyond the hash. `applyModel` is the whole step from
 * wire state to a decision, so the sync path calls one function.
 */

import type { Descriptor, MetamodelId, ModelHeader, PlainJson, WireNode } from '../api/types';
import { decodeState } from '../crdt/decode';
import { metamodelIdOf } from './digest';
import { modelHeaderOf } from './instance';

export type Binding =
  | { kind: 'unbound' }
  | { kind: 'bound'; header: ModelHeader; served: MetamodelId }
  | { kind: 'no-descriptor'; header: ModelHeader }
  | { kind: 'mismatch'; header: ModelHeader; served: MetamodelId }
  | { kind: 'header-rewritten'; recorded: ModelHeader; header: ModelHeader | null };

/** The two verdicts under which nothing is applied. */
export type Refusal = Extract<Binding, { kind: 'mismatch' | 'header-rewritten' }>;

export function isRefusal(binding: Binding): binding is Refusal {
  return binding.kind === 'mismatch' || binding.kind === 'header-rewritten';
}

export interface Applied {
  applied: true;
  doc: PlainJson;
  binding: Exclude<Binding, Refusal>;
}

export interface Refused {
  applied: false;
  binding: Refusal;
  /** The sentence the user sees: both pairs, and that editing is disabled. */
  message: string;
}

export type ApplyResult = Applied | Refused;

function sameId(a: MetamodelId, b: MetamodelId): boolean {
  return a.nsURI === b.nsURI && a.digest === b.digest;
}

function sameHeader(a: ModelHeader | null, b: ModelHeader | null): boolean {
  if (a === null || b === null) return a === b;
  return a.modelId === b.modelId && sameId(a.metamodelId, b.metamodelId);
}

/**
 * The verdict for a document whose header is `header`, to be rendered under
 * the descriptor identified by `served`, given the header `recorded` at the
 * first apply (null before one). The recorded comparison comes first: a
 * header that moved is refused whatever the descriptor says.
 */
export function checkBinding(
  header: ModelHeader | null,
  served: MetamodelId | null,
  recorded: ModelHeader | null,
): Binding {
  if (recorded !== null && !sameHeader(recorded, header)) {
    return { kind: 'header-rewritten', recorded, header };
  }
  if (header === null) return { kind: 'unbound' };
  if (served === null) return { kind: 'no-descriptor', header };
  if (header.metamodelId.digest === served.digest) return { kind: 'bound', header, served };
  return { kind: 'mismatch', header, served };
}

/**
 * The whole step: decode the wire state, read its header, hash the descriptor
 * it would be rendered under, decide. Throws where `decodeState` and
 * `modelHeaderOf` throw, which is a wire-contract violation and is surfaced
 * as a failed sync rather than guessed around.
 */
export async function applyModel(
  wire: WireNode,
  served: Descriptor | null,
  recorded: ModelHeader | null,
): Promise<ApplyResult> {
  const doc = decodeState(wire);
  const header = modelHeaderOf(doc);
  // Over the descriptor's bytes, here, every time: never a digest the node
  // reports for it. A headerless log has nothing to compare, so no hash.
  const servedId = header === null || served === null ? null : await metamodelIdOf(served);
  const binding = checkBinding(header, servedId, recorded);
  if (isRefusal(binding)) {
    return { applied: false, binding, message: describeBinding(binding) };
  }
  return { applied: true, doc, binding };
}

/** Whether two verdicts say the same thing, so a change is reported once and not on every poll. */
export function sameBinding(a: Binding, b: Binding): boolean {
  switch (a.kind) {
    case 'unbound':
      return b.kind === 'unbound';
    case 'bound':
      return b.kind === 'bound' && sameHeader(a.header, b.header) && sameId(a.served, b.served);
    case 'no-descriptor':
      return b.kind === 'no-descriptor' && sameHeader(a.header, b.header);
    case 'mismatch':
      return b.kind === 'mismatch' && sameHeader(a.header, b.header) && sameId(a.served, b.served);
    case 'header-rewritten':
      return (
        b.kind === 'header-rewritten' &&
        sameHeader(a.recorded, b.recorded) &&
        sameHeader(a.header, b.header)
      );
  }
}

function pair(id: MetamodelId): string {
  return `${id.nsURI} (digest ${id.digest})`;
}

function headerText(header: ModelHeader): string {
  return `model ${header.modelId} bound to ${pair(header.metamodelId)}`;
}

/** One sentence per verdict: the alert, the log row, the held controls and the chip's title all say this. */
export function describeBinding(binding: Binding): string {
  switch (binding.kind) {
    case 'unbound':
      return 'unbound: the log carries no __model header, so the document is applied under the loaded descriptor as before';
    case 'bound':
      return `bound to ${pair(binding.header.metamodelId)}`;
    case 'no-descriptor':
      return `bound to ${pair(binding.header.metamodelId)}, and no descriptor is loaded to check it against: load the one with that digest`;
    case 'mismatch':
      return `apply refused: the header binds this model to ${pair(binding.header.metamodelId)} but the descriptor loaded for it is ${pair(binding.served)}; nothing was applied and editing is disabled`;
    case 'header-rewritten':
      return `apply refused: the __model header changed after it was first read, from ${headerText(binding.recorded)} to ${
        binding.header === null ? 'no header at all' : headerText(binding.header)
      }; the header is written once by the creating node, so a peer has ignored the rule; nothing was applied and editing is disabled`;
  }
}

/** The word the top bar shows beside the metamodel. */
export function bindingLabel(binding: Binding): string {
  switch (binding.kind) {
    case 'unbound':
      return 'unbound';
    case 'bound':
      return 'bound';
    case 'no-descriptor':
      return 'bound, no descriptor';
    case 'mismatch':
    case 'header-rewritten':
      return 'not applied';
  }
}

/** The refusal sentence when the last verdict refused, else null: what the edit gate holds on. */
export function refusalOf(binding: Binding | null): string | null {
  return binding !== null && isRefusal(binding) ? describeBinding(binding) : null;
}
