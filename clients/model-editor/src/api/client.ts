/**
 * Typed client for the moirai node HTTP API.
 *
 * Endpoints (this base): GET /api/health; the model routes GET /api/models,
 * POST /api/models, GET /api/metamodels (the descriptors the node holds, each
 * with its digest), GET /api/model/{id}/state, POST /api/model/{id}/op and
 * GET /api/model/{id}/metamodel; and the unscoped GET /api/state, POST /api/op
 * and GET /api/metamodel, which serve the node's default log and stay for the
 * compatibility step the design names.
 *
 * Error contract: network failures and non-OK statuses throw ApiError, the
 * status on it and the node's own `error` text in the message; the op routes
 * additionally return {"success": false, ...} with HTTP 200 for well-formed
 * but refused ops — callers MUST branch on `.success`, and the op queue turns
 * that case into a visible error (never swallowed).
 */

import type {
  Descriptor,
  HostedModel,
  MetamodelId,
  MetamodelListing,
  ModelId,
  ModelOp,
  OpResult,
  PlainJson,
  Registration,
} from './types';

export class ApiError extends Error {
  readonly status: number | null;

  constructor(message: string, status: number | null = null) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
  }
}

export interface HealthInfo {
  replicaId: string;
  raw: Record<string, unknown>;
}

function normalizeBase(url: string): string {
  return url.replace(/\/+$/, '');
}

async function request(base: string, path: string, init?: RequestInit): Promise<Response> {
  let response: Response;
  try {
    response = await fetch(`${normalizeBase(base)}${path}`, init);
  } catch (err) {
    throw new ApiError(`network error on ${path}: ${err instanceof Error ? err.message : String(err)}`);
  }
  return response;
}

async function readJson(response: Response, path: string): Promise<unknown> {
  const text = await response.text();
  try {
    return JSON.parse(text);
  } catch {
    throw new ApiError(`${path} returned non-JSON (${response.status}): ${text.slice(0, 200)}`, response.status);
  }
}

/** GET /api/health — returns the replica id (field name tolerated as replica_id | replicaId | id). */
export async function getHealth(base: string): Promise<HealthInfo> {
  const response = await request(base, '/api/health');
  if (!response.ok) throw new ApiError(`/api/health returned ${response.status}`, response.status);
  const body = (await readJson(response, '/api/health')) as Record<string, unknown>;
  const id = body['replica_id'] ?? body['replicaId'] ?? body['id'];
  return { replicaId: typeof id === 'string' || typeof id === 'number' ? String(id) : 'unknown', raw: body };
}

/** A non-OK reply as an ApiError, carrying the body's `error` text when it has one. */
async function failure(response: Response, path: string): Promise<ApiError> {
  const text = await response.text();
  let detail = '';
  try {
    const body = JSON.parse(text) as Record<string, unknown>;
    if (typeof body['error'] === 'string') detail = `: ${body['error']}`;
  } catch {
    // Not JSON: the status is the message.
  }
  return new ApiError(`${path} returned ${response.status}${detail}`, response.status);
}

/** The scoped route for one hosted model. */
function modelPath(id: ModelId, leaf: 'state' | 'metamodel' | 'op'): string {
  return `/api/model/${encodeURIComponent(id)}/${leaf}`;
}

/**
 * The state routes serve the model document itself: the canonical form the
 * interpreted node renders, `null` for a model whose root has not been
 * written. There is no envelope to unwrap and no per-character array to join.
 */
async function readState(base: string, path: string): Promise<PlainJson> {
  const response = await request(base, path);
  if (!response.ok) throw await failure(response, path);
  return (await readJson(response, path)) as PlainJson;
}

/** The descriptor routes: a descriptor of a format version the editor reads, or null on 404. */
async function readDescriptor(base: string, path: string): Promise<Descriptor | null> {
  const response = await request(base, path);
  if (response.status === 404) return null;
  if (!response.ok) throw await failure(response, path);
  return validateDescriptor(await readJson(response, path));
}

/** GET /api/state — the node's default log. */
export function getState(base: string): Promise<PlainJson> {
  return readState(base, '/api/state');
}

/** GET /api/model/{id}/state — one hosted model. A malformed id (400) and an unhosted one (404) both throw. */
export function getModelState(base: string, id: ModelId): Promise<PlainJson> {
  return readState(base, modelPath(id, 'state'));
}

/**
 * GET /api/metamodel — the node's descriptor, or null when the node serves
 * none (404). Validates formatVersion so a future format fails loudly.
 */
export function getMetamodel(base: string): Promise<Descriptor | null> {
  return readDescriptor(base, '/api/metamodel');
}

/**
 * GET /api/model/{id}/metamodel — the descriptor the model was registered
 * under, or null when the node holds none for it (404). An id the node does
 * not host is 404 as well; fetch the model's state first, which tells the two
 * apart by throwing.
 */
export function getModelMetamodel(base: string, id: ModelId): Promise<Descriptor | null> {
  return readDescriptor(base, modelPath(id, 'metamodel'));
}

/**
 * GET /api/metamodels — the descriptors the node holds, each listed as
 * {nsURI, package, digest}. A registration names one by its `{nsURI, digest}`,
 * and a model's header records the same pair.
 */
export async function getMetamodels(base: string): Promise<MetamodelListing[]> {
  const response = await request(base, '/api/metamodels');
  if (!response.ok) throw new ApiError(`/api/metamodels returned ${response.status}`, response.status);
  const body = (await readJson(response, '/api/metamodels')) as Record<string, unknown>;
  const entries = body['metamodels'];
  if (!Array.isArray(entries)) throw new ApiError('/api/metamodels body has no "metamodels" array');
  return entries.map(validateMetamodelListing);
}

/** Validate one entry of the metamodel listing. */
export function validateMetamodelListing(entry: unknown): MetamodelListing {
  if (typeof entry !== 'object' || entry === null) {
    throw new ApiError('metamodel listing entry is not a JSON object');
  }
  const { nsURI, digest, package: pkg } = entry as Partial<MetamodelListing>;
  if (typeof nsURI !== 'string' || typeof digest !== 'string') {
    throw new ApiError('metamodel listing entry has no nsURI/digest strings');
  }
  return { nsURI, digest, package: typeof pkg === 'string' ? pkg : '' };
}

/**
 * A `metamodel_id` as the node echoes it: the {nsURI, digest} pair it was
 * registered with, a bare digest string (which the node also accepts, and is
 * listed with an empty nsURI), or null for the default log, which has none.
 */
function readMetamodelId(raw: unknown, where: string): MetamodelId | null {
  if (raw === null || raw === undefined) return null;
  if (typeof raw === 'string') return { nsURI: '', digest: raw };
  if (typeof raw === 'object') {
    const { nsURI, digest } = raw as Partial<MetamodelId>;
    if (typeof digest === 'string') return { nsURI: typeof nsURI === 'string' ? nsURI : '', digest };
  }
  throw new ApiError(`${where} has a metamodel_id that is neither a {nsURI, digest} pair nor a digest`);
}

/** GET /api/models — the models the node hosts, the default log among them with a null metamodel. */
export async function getModels(base: string): Promise<HostedModel[]> {
  const response = await request(base, '/api/models');
  if (!response.ok) throw await failure(response, '/api/models');
  const body = (await readJson(response, '/api/models')) as Record<string, unknown>;
  const entries = body['models'];
  if (!Array.isArray(entries)) throw new ApiError('/api/models body has no "models" array');
  return entries.map((entry): HostedModel => {
    if (typeof entry !== 'object' || entry === null) throw new ApiError('/api/models entry is not a JSON object');
    const record = entry as Record<string, unknown>;
    const modelId = record['model_id'];
    if (typeof modelId !== 'string') throw new ApiError('/api/models entry has no model_id string');
    return { modelId, metamodelId: readMetamodelId(record['metamodel_id'], '/api/models entry') };
  });
}

/**
 * POST /api/models — register a model on the node. Without `modelId` the node
 * creates one and mints its id (201): the editor never chooses an id. With
 * `modelId` the node joins that model by id (200) and writes no header, since
 * the header travels with the log. A 409 (already hosted), 422 (a metamodel
 * the node does not hold) or 400 throws an ApiError carrying the status and
 * the node's sentence.
 */
export async function registerModel(
  base: string,
  registration: { modelId?: ModelId; metamodelId: MetamodelId },
): Promise<Registration> {
  const body: Record<string, unknown> = { metamodel_id: registration.metamodelId };
  if (registration.modelId !== undefined) body['model_id'] = registration.modelId;
  const response = await request(base, '/api/models', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
  if (!response.ok) throw await failure(response, '/api/models');
  const reply = (await readJson(response, '/api/models')) as Record<string, unknown>;
  const modelId = reply['model_id'];
  if (typeof modelId !== 'string') throw new ApiError('/api/models reply has no model_id string');
  const metamodelId = readMetamodelId(reply['metamodel_id'], '/api/models reply');
  if (metamodelId === null) throw new ApiError('/api/models reply has no metamodel_id');
  return { modelId, metamodelId, created: reply['created'] === true };
}

/**
 * A `ModelOp` as the node's `serde_json` reads it.
 *
 * `Scalar::Float` carries the double's bit pattern, a `u64` that does not fit
 * a JavaScript number, so the encoder builds it as a `bigint` and it is
 * written here as an unquoted JSON integer — which is the only spelling
 * serde reads back as a `u64`. Every other value goes through
 * `JSON.stringify` untouched.
 */
export function stringifyModelOp(op: ModelOp): string {
  const marker = '\u0000bigint:';
  const text = JSON.stringify(op, (_key, value: unknown) =>
    typeof value === 'bigint' ? `${marker}${value}` : value,
  );
  return text.replace(/"\\u0000bigint:(\d+)"/g, '$1');
}

/** The op routes take the operation as the whole body, and answer {"success", "message"}. */
async function submitOp(base: string, path: string, op: ModelOp): Promise<OpResult> {
  const response = await request(base, path, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: stringifyModelOp(op),
  });
  const body = (await readJson(response, path)) as Record<string, unknown>;
  if (!response.ok) {
    const detail = typeof body['error'] === 'string' ? body['error'] : JSON.stringify(body);
    throw new ApiError(`${path} returned ${response.status}: ${detail}`, response.status);
  }
  if (typeof body['success'] !== 'boolean') {
    throw new ApiError(`${path} body has no boolean "success" field`);
  }
  return { success: body['success'], message: typeof body['message'] === 'string' ? body['message'] : '' };
}

/**
 * POST /api/op — the default log. Malformed ops are HTTP 400 (throws
 * ApiError); refused ops come back HTTP 200 with success:false — returned
 * as-is for the caller to surface.
 */
export function postOp(base: string, op: ModelOp): Promise<OpResult> {
  return submitOp(base, '/api/op', op);
}

/** POST /api/model/{id}/op — one hosted model, the same contract as postOp; an unhosted id is 404 and throws. */
export function postModelOp(base: string, id: ModelId, op: ModelOp): Promise<OpResult> {
  return submitOp(base, modelPath(id, 'op'), op);
}

/** Validate a descriptor loaded from the node or from a file. */
export function validateDescriptor(body: unknown): Descriptor {
  if (typeof body !== 'object' || body === null) {
    throw new ApiError('metamodel descriptor is not a JSON object');
  }
  const desc = body as Partial<Descriptor>;
  // Version 2 is the one an interpreted node reads: it carries the merge rule
  // every feature is routed by, and the slot table is computed from it.
  if (desc.formatVersion !== 2) {
    throw new ApiError(`unsupported descriptor formatVersion: ${String(desc.formatVersion)}`);
  }
  if (typeof desc.classes !== 'object' || desc.classes === null || !Array.isArray(desc.rootClasses)) {
    throw new ApiError('metamodel descriptor missing classes/rootClasses');
  }
  return desc as Descriptor;
}
