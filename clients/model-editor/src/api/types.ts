/**
 * Wire-level and metamodel-descriptor types for the moirai HTTP API.
 *
 * Wire truth (verified against a live network_node on this base):
 * - GET /api/state returns {"json": <WireNode>}; a fresh node returns {"json": "Unset"}.
 * - Every populated node is wrapped: {"Value": {...}}; strings are char arrays.
 * - POST /api/op takes {"JsonKind": <JsonOp>} and answers {"success": bool, "message": string}.
 * - GET /api/metamodel returns a formatVersion 1 or 2 descriptor, or 404 when the node serves none.
 * - GET /api/model/{id}/state, POST /api/model/{id}/op and GET /api/model/{id}/metamodel are
 *   the same, scoped to one hosted model; a malformed id is 400, an id the node does not host is 404.
 * - GET /api/models lists the hosted models as {"models": [{model_id, metamodel_id}]}, the
 *   metamodel_id null for the default log; POST /api/models registers one: {metamodel_id} creates
 *   (201, the node mints the id), {model_id, metamodel_id} joins (200); 409 already hosted, 422
 *   unknown metamodel, both with an "error" text.
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

/* ---------- Metamodel descriptor (formatVersion 1 and 2) ---------- */

export type AttributeKind = 'string' | 'int' | 'float' | 'bool' | 'enum';

/**
 * The four keys formatVersion 2 adds to every feature entry.
 *
 * Version 1 described a metamodel for an editor: enough shape to draw a form.
 * Version 2 additionally publishes how the node merges each feature, and where
 * every part of that decision came from, so a node can merge by the descriptor
 * instead of being compiled against it. The editor reads none of it today and
 * every key is optional, which is why a version 1 descriptor still validates.
 */

/** `ordered` and `unique` exactly as the .ecore file wrote them; null where it was silent. */
export interface FacetsDesc {
  ordered: boolean | null;
  unique: boolean | null;
}

/** Where one facet of a merge rule came from. */
export type FacetSource =
  | 'declared'
  | 'ecoreDefault'
  | 'houseDefault'
  | 'annotation'
  | 'notApplicable';

/** The source of each of the four facets of a merge rule. */
export interface ProvenanceDesc {
  ordered: FacetSource;
  unique: FacetSource;
  leaf: FacetSource;
  presence: FacetSource;
}

/** The collection a feature's values sit in. */
export interface ShapeDesc {
  kind: 'single' | 'optional' | 'sequence' | 'set' | 'bag' | 'orderedSet';
  /** Set when kind === 'set'. */
  tie?: 'aw' | 'rw';
}

/** The innermost CRDT of an attribute. */
export interface LeafDesc {
  kind: 'text' | 'counter' | 'flag' | 'register' | 'enum';
  /** Set when kind === 'counter': the Rust width the generator compiles. */
  num?: 'u8' | 'i16' | 'i32' | 'i64' | 'f32' | 'f64';
  /** Set when kind === 'counter'. */
  resettable?: boolean;
  /** Set when kind === 'flag'. */
  wins?: 'enable' | 'disable';
  /** Set when kind === 'register' or 'enum'. */
  tie?: 'mv' | 'lww' | 'fair' | 'po' | 'to';
  /** Set when kind === 'enum': key into Descriptor.enums. */
  class?: string;
}

/** The CRDT construction the generator compiles for one feature. */
export interface MergeDesc {
  kind: 'attribute' | 'containment' | 'reference' | 'unsupported';
  /** Set when kind === 'attribute' or 'containment'. */
  shape?: ShapeDesc;
  /** Set when kind === 'attribute'. */
  leaf?: LeafDesc;
  /** Set when kind === 'containment' or 'reference'. */
  target?: string;
  /** Set when kind === 'reference'. */
  many?: boolean;
  /** Set when kind === 'unsupported'. */
  reason?: 'keyed' | 'transparent' | 'derived' | 'transient' | 'volatile';
}

/** The formatVersion 2 keys, absent on a version 1 descriptor. */
export interface FeatureSemantics {
  facets?: FacetsDesc;
  annotation?: string | null;
  merge?: MergeDesc;
  provenance?: ProvenanceDesc;
}

export interface AttributeDesc extends FeatureSemantics {
  name: string;
  kind: AttributeKind;
  /** Set when kind === 'enum': key into Descriptor.enums. */
  enum?: string;
  many: boolean;
  required: boolean;
  isId: boolean;
}

export interface ContainmentDesc extends FeatureSemantics {
  name: string;
  target: string;
  many: boolean;
  required: boolean;
  ordered: boolean;
}

export interface ReferenceDesc extends FeatureSemantics {
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
 * Whether a string has the shape of a `ModelId`: exactly 32 lowercase hex
 * characters, as `LogId::parse` on the node accepts. The header's `modelId`
 * arrives over the wire from whoever wrote it, so anything that turns it into
 * a file name or a route checks the shape first.
 */
export function isModelId(value: string): boolean {
  return /^[0-9a-f]{32}$/.test(value);
}

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

/** One entry of GET /api/models: a hosted model and the metamodel it was registered under, null for the default log. */
export interface HostedModel {
  modelId: ModelId;
  metamodelId: MetamodelId | null;
}

/** What POST /api/models answers: the model's id, minted by the node on a create, and the metamodel it was registered under. */
export interface Registration {
  modelId: ModelId;
  metamodelId: MetamodelId;
  created: boolean;
}
