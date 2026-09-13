/**
 * Wire-level and metamodel-descriptor types for the moirai HTTP API.
 *
 * Wire truth (verified against a live interpreted `model_node` on this base):
 * - GET /api/state and GET /api/model/{id}/state return the model document as
 *   plain JSON: an object per element carrying `eClass` and one key per
 *   feature its class can see, and bare `null` for a model whose root has not
 *   been written. There is no envelope and no per-character array: the
 *   interpreted node renders the canonical form itself.
 * - POST /api/op and POST /api/model/{id}/op take a `ModelOp` as the whole
 *   body — `{"Instance": ...}` or `{"Install": ...}`, nothing wrapping it —
 *   and answer {"success": bool, "message": string}. A body of any other
 *   shape is HTTP 400 with `Invalid op JSON`.
 * - GET /api/metamodel returns a formatVersion 2 descriptor, or 404 when the
 *   node serves none.
 * - GET /api/models lists the hosted models as {"models": [{model_id, metamodel_id}]}, the
 *   metamodel_id null for the default log; POST /api/models registers one: {metamodel_id} creates
 *   (201, the node mints the id), {model_id, metamodel_id} joins (200); 409 already hosted, 422
 *   unknown metamodel, both with an "error" text.
 */

/* ---------- The document as the node serves it ---------- */

/** Decoded, plain-JSON view of the document. `null` means an unwritten root (empty doc). */
export type PlainJson =
  | null
  | string
  | number
  | boolean
  | PlainJson[]
  | { [key: string]: PlainJson };

/* ---------- Operations (the interpreted `ModelOp` grammar) ---------- */

/**
 * One value a leaf can hold (moirai-interp/src/leaf.rs `Scalar`).
 *
 * `Float` carries the IEEE-754 bit pattern of the `f64`, not the value, which
 * is what makes the Rust type `Eq + Hash + Ord`; it is a `bigint` here
 * because the pattern does not fit a JavaScript number, and `stringifyModelOp`
 * in api/client.ts is what writes it as an unquoted JSON integer.
 */
export type Scalar =
  | 'Null'
  | { Bool: boolean }
  | { Int: number }
  | { Float: bigint }
  | { Char: string }
  | { Str: string }
  /** [slot of the enum in the table's `enums`, position of the literal in its declaration order] */
  | { Enum: [number, number] };

/** One write to one leaf (moirai-interp/src/leaf.rs `LeafOp`). The leaf's own rule decides which it takes. */
export type LeafOp =
  | { InsertChar: { pos: number; ch: string } }
  | { DeleteChar: { pos: number } }
  | { DeleteRange: { start: number; len: number } }
  | { Inc: Scalar }
  | { Dec: Scalar }
  | 'Reset'
  | 'Enable'
  | 'Disable'
  | { Write: Scalar }
  | { Add: Scalar }
  | { Remove: Scalar }
  | 'Clear';

/** A sequence step; `pos` is a position among the visible children, resolved against the writer's own version. */
export type SeqOp =
  | { Insert: { pos: number; op: InstanceOp } }
  | { Update: { pos: number; op: InstanceOp } }
  | { Delete: { pos: number } };

/** An optional step. */
export type OptOp = { Set: InstanceOp } | 'Unset';

/** A keyed step; the key is carried by value and never resolved against a version. */
export type MapOp =
  | { Update: { key: Scalar; op: InstanceOp } }
  | { Remove: { key: Scalar } }
  | 'Clear';

/**
 * One step of the path from the model root to the write at its end
 * (moirai-interp/src/op.rs `InstanceOp`).
 *
 * `Field` carries a feature's VISIBLE slot: its position in the class's
 * visible features, own and inherited, sorted by name (crdt/table.ts computes
 * it exactly as moirai-semantics/src/parse.rs does). `Variant` names the
 * concrete class of the object sitting in a containment, on every operation
 * that reaches through the slot and not only on the one that created it.
 */
export type InstanceOp =
  | { Field: [number, InstanceOp] }
  | { Variant: [number, InstanceOp] }
  | { Seq: SeqOp }
  | { Opt: OptOp }
  | { Map: MapOp }
  | 'New'
  | { Leaf: LeafOp };

/**
 * One operation on a model log: the whole body of a POST to an op route.
 *
 * The editor never writes `Install`: the node that creates the model opens
 * its log, and a joiner receives it by transfer or by delta replay.
 */
export type ModelOp =
  | { Install: { model_id: string; metamodel_id: string; descriptor: string } }
  | { Instance: InstanceOp };

/** POST /api/op response body (HTTP 200 even when the op is refused). */
export interface OpResult {
  success: boolean;
  message: string;
}

/* ---------- Metamodel descriptor (formatVersion 2) ---------- */

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
  kind: 'single' | 'optional' | 'sequence' | 'set' | 'bag' | 'keyed' | 'orderedSet';
  /** Set when kind === 'set'. */
  tie?: 'aw' | 'rw';
  /** Set when kind === 'keyed': what the entries are addressed by. */
  key?: { kind: string; num?: string; class?: string };
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
  /**
   * The one feature the generator represents this class as
   * (`urn:arachne:representation` kind="transparent"), by name; absent on
   * every other class. Such a class has no record of its own: the read-out
   * renders an instance of it as that feature's value, with no `eClass` and
   * no wrapper.
   */
  transparent?: string | null;
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

/**
 * What GET /api/models answers, both halves of it: the models the node hosts,
 * and the bare ids of models it does not host but has seen traffic for since
 * it connected.
 *
 * `seen` is ids and nothing else. A node cannot say which metamodel a model
 * it does not host is bound to — that binding lives in the model's first
 * operation, in a log the node does not hold — so joining one of these still
 * means choosing the metamodel, and a wrong choice is refused by the node
 * with a 422 naming the digest.
 */
export interface ModelListing {
  hosted: HostedModel[];
  seen: ModelId[];
}

/** What POST /api/models answers: the model's id, minted by the node on a create, and the metamodel it was registered under. */
export interface Registration {
  modelId: ModelId;
  metamodelId: MetamodelId;
  created: boolean;
}

/**
 * What POST /api/metamodels answers: the entry the node listed for the
 * descriptor it was given, and the whole listing as it stands afterwards.
 *
 * `added` is false when the node already held this digest, which is not a
 * failure — the same descriptor posted twice is the same metamodel, since the
 * digest is taken over what the node parsed and not over the file's bytes.
 * `metamodels` is what `GET /api/metamodels` would answer on the next request,
 * so a caller that has just posted needs no second round trip to refresh.
 */
export interface MetamodelAdded {
  added: boolean;
  metamodel: MetamodelListing;
  metamodels: MetamodelListing[];
}
