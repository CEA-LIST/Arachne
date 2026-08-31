import type { PlainJson, WireNode, WireValue } from '../api/types';

/**
 * Decode the node's serialized CRDT state into plain JSON.
 *
 * "Unset" (a fresh replica) decodes to null. Every populated node is a
 * {"Value": ...} wrapper; strings arrive as arrays of single-char strings.
 * Unknown shapes throw: a decode failure means the wire contract changed and
 * must be surfaced, not guessed around.
 */
export function decodeState(node: WireNode): PlainJson {
  if (node === 'Unset') return null;
  if (typeof node === 'object' && node !== null && 'Value' in node) {
    return decodeValue(node.Value);
  }
  throw new Error(`unrecognized state node: ${JSON.stringify(node).slice(0, 200)}`);
}

function decodeValue(value: WireValue): PlainJson {
  if (typeof value !== 'object' || value === null) {
    throw new Error(`unrecognized state value: ${JSON.stringify(value).slice(0, 200)}`);
  }
  if ('Object' in value) {
    const out: { [key: string]: PlainJson } = {};
    for (const [key, child] of Object.entries(value.Object)) {
      out[key] = decodeState(child);
    }
    return out;
  }
  if ('Array' in value) {
    return value.Array.map(decodeState);
  }
  if ('String' in value) {
    return value.String.join('');
  }
  if ('Number' in value) {
    return value.Number;
  }
  if ('Boolean' in value) {
    return value.Boolean;
  }
  throw new Error(`unrecognized state value: ${JSON.stringify(value).slice(0, 200)}`);
}
