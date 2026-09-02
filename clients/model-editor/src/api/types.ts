/**
 * Wire-level and metamodel-descriptor types for the moirai HTTP API.
 *
 * Wire truth (verified against a live network_node on this base):
 * - GET /api/state returns {"json": <WireNode>}; a fresh node returns {"json": "Unset"}.
 * - Every populated node is wrapped: {"Value": {...}}; strings are char arrays.
 * - POST /api/op takes {"JsonKind": <JsonOp>} and answers {"success": bool, "message": string}.
 * - GET /api/metamodel returns a formatVersion-1 descriptor, or 404 when the node serves none.
 * - GET /api/model/{id}/state and /metamodel are the same, scoped to one hosted model;
 *   a malformed id is 400, an id the node does not host is 404.
 */

/* ---------- CRDT state as serialized by the node ---------- */

export type WireNode = 'Unset' | { Value: WireValue };

export type WireValue =
  | { Object: Record<string, WireNode> }
  | { Array: WireNode[] }
  | { String: string[] }
  | { Number: number }
  | { Boolean: boolean };

/** Decoded, plain-JSON view of the document. `null` means "Unset" (empty doc). */
export type PlainJson =
  | null
  | string
  | number
  | boolean
  | PlainJson[]
  | { [key: string]: PlainJson };

/* ---------- Operations (the JsonKind grammar) ---------- */

export type ObjectOp =
  | { Update: [string, JsonOp] }
  | { Remove: string }
  | 'Clear';

export type ArrayOp =
  | { Insert: { pos: number; op: JsonOp } }
  | { Update: { pos: number; op: JsonOp } }
  | { Delete: { pos: number } };

export type StringOp =
  | { Insert: { content: string; pos: number } } // content MUST be exactly one char
  | { Delete: { pos: number } }
  | { DeleteRange: { start: number; len: number } };

export type JsonOp =
  | { Object: ObjectOp }
  | { Array: ArrayOp }
  | { String: StringOp }
  | { Number: { Inc: number } }
  | { Boolean: 'Enable' | 'Disable' };

/** POST /api/op envelope. */
export interface OpEnvelope {
  JsonKind: JsonOp;
}

/** POST /api/op response body (HTTP 200 even when the op is refused). */
export interface OpResult {
  success: boolean;
  message: string;
}

/* ---------- Metamodel descriptor (formatVersion 1) ---------- */

export type AttributeKind = 'string' | 'int' | 'float' | 'bool' | 'enum';

export interface AttributeDesc {
  name: string;
  kind: AttributeKind;
  /** Set when kind === 'enum': key into Descriptor.enums. */
  enum?: string;
  many: boolean;
  required: boolean;
  isId: boolean;
}

export interface ContainmentDesc {
  name: string;
  target: string;
  many: boolean;
  required: boolean;
  ordered: boolean;
}

export interface ReferenceDesc {
  name: string;
  target: string;
  many: boolean;
  required: boolean;
}

export interface ClassDesc {
  abstract: boolean;
  superTypes: string[];
  attributes: AttributeDesc[];
  containments: ContainmentDesc[];
  references: ReferenceDesc[];
}

export interface Descriptor {
  formatVersion: number;
  package: string;
  nsURI: string;
  rootClasses: string[];
  classes: Record<string, ClassDesc>;
  enums: Record<string, string[]>;
}

/** A location inside the decoded document: object keys and array indices. */
export type Path = (string | number)[];

/* ---------- Model identity and the header (the model plane) ---------- */

/** A model is its log: the 32 lowercase hex characters the wire carries as the log id. */
export type ModelId = string;

/**
 * The identity of a metamodel: the descriptor's nsURI beside the SHA-256,
 * lowercase hex, of its canonical JSON (compact, keys sorted at every level).
 * `model/digest.ts` computes it; the node computes the same for what it serves
 * and lists it on GET /api/metamodels.
 */
export interface MetamodelId {
  nsURI: string;
  digest: string;
}

/**
 * The reserved root key every model carries its header under. Written once by
 * the node that created the model, received by transfer everywhere else, and
 * never the target of an op the editor emits (crdt/ops.ts refuses); the root
 * walkers in model/instance.ts skip it.
 */
export const MODEL_HEADER_KEY = '__model';

/** The header under MODEL_HEADER_KEY: the model's id and the metamodel it is bound to. */
export interface ModelHeader {
  modelId: ModelId;
  metamodelId: MetamodelId;
}

/** One entry of GET /api/metamodels: a descriptor the node holds, keyed by its digest. */
export interface MetamodelListing extends MetamodelId {
  package: string;
}
