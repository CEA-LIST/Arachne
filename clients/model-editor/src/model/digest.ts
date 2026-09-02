/**
 * The metamodel digest, as the node computes it: SHA-256, lowercase hex, over
 * the canonical JSON of the parsed descriptor.
 *
 * Canonical means what serde_json emits for a parsed value on the node: no
 * whitespace, object keys sorted (BTreeMap order, which is code-point order),
 * arrays as they are. It is over the parsed value and never over file bytes,
 * so a reformatted descriptor keeps its digest and its models, and an edited
 * one gets another. The descriptor format carries strings, booleans, arrays,
 * objects and one integer, on which JSON.stringify and serde_json agree byte
 * for byte; a non-integer number would not (serde_json writes `1.0`, JS `1`)
 * and none exists in the format. The fixture in
 * examples/fixtures/metamodel-digests.json holds this and the Rust side to
 * one answer (test mp6).
 *
 * Hashing uses WebCrypto (`crypto.subtle`), which browsers expose in secure
 * contexts only: https, localhost and 127.0.0.1. Node 24 exposes it globally.
 */

import type { Descriptor, MetamodelId } from '../api/types';

/** Code-point order, which is what a byte-wise comparison of UTF-8 keys gives on the node. */
function byCodePoint(a: string, b: string): number {
  const left = Array.from(a);
  const right = Array.from(b);
  const length = Math.min(left.length, right.length);
  for (let i = 0; i < length; i++) {
    const delta = (left[i].codePointAt(0) ?? 0) - (right[i].codePointAt(0) ?? 0);
    if (delta !== 0) return delta;
  }
  return left.length - right.length;
}

/** The canonical JSON text of `value`: compact, keys sorted at every level. */
export function canonicalJson(value: unknown): string {
  if (
    value === null ||
    typeof value === 'boolean' ||
    typeof value === 'number' ||
    typeof value === 'string'
  ) {
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) {
    return `[${value.map(canonicalJson).join(',')}]`;
  }
  if (typeof value === 'object') {
    const record = value as Record<string, unknown>;
    const keys = Object.keys(record)
      .filter((key) => record[key] !== undefined)
      .sort(byCodePoint);
    return `{${keys.map((key) => `${JSON.stringify(key)}:${canonicalJson(record[key])}`).join(',')}}`;
  }
  throw new Error(`cannot canonicalize a value of type ${typeof value}`);
}

/** SHA-256 of the UTF-8 encoding of `text`, as lowercase hex. */
export async function sha256Hex(text: string): Promise<string> {
  const hash = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(text));
  return Array.from(new Uint8Array(hash), (byte) => byte.toString(16).padStart(2, '0')).join('');
}

/** The digest half of a MetamodelId for a parsed descriptor. */
export function metamodelDigest(descriptor: unknown): Promise<string> {
  return sha256Hex(canonicalJson(descriptor));
}

/** The identity of a descriptor: its nsURI beside its digest. */
export async function metamodelIdOf(descriptor: Descriptor): Promise<MetamodelId> {
  return { nsURI: descriptor.nsURI, digest: await metamodelDigest(descriptor) };
}
