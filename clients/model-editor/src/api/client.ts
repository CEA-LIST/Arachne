/**
 * Typed client for the moirai node HTTP API.
 *
 * Endpoints (this base): GET /api/health, GET /api/state, POST /api/op,
 * GET /api/metamodel (404 when the node serves no descriptor),
 * GET /api/metamodels (the descriptors the node holds, each with its digest),
 * and the model-scoped GET /api/model/{id}/state and /metamodel.
 *
 * Error contract: network failures and non-OK statuses throw ApiError;
 * POST /api/op additionally returns {"success": false, ...} with HTTP 200 for
 * well-formed but refused ops — callers MUST branch on `.success`, and the op
 * queue turns that case into a visible error (never swallowed).
 */

import type { Descriptor, JsonOp, MetamodelListing, ModelId, OpResult, WireNode } from './types';

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
function modelPath(id: ModelId, leaf: 'state' | 'metamodel'): string {
  return `/api/model/${encodeURIComponent(id)}/${leaf}`;
}

/** The state routes share one envelope: {"json": <WireNode>}. */
async function readState(base: string, path: string): Promise<WireNode> {
  const response = await request(base, path);
  if (!response.ok) throw await failure(response, path);
  const body = (await readJson(response, path)) as Record<string, unknown>;
  if (!('json' in body)) throw new ApiError(`${path} body has no "json" field`);
  return body['json'] as WireNode;
}

/** The descriptor routes: a formatVersion-1 descriptor, or null on 404. */
async function readDescriptor(base: string, path: string): Promise<Descriptor | null> {
  const response = await request(base, path);
  if (response.status === 404) return null;
  if (!response.ok) throw await failure(response, path);
  return validateDescriptor(await readJson(response, path));
}

/** GET /api/state — the node's default log. */
export function getState(base: string): Promise<WireNode> {
  return readState(base, '/api/state');
}

/** GET /api/model/{id}/state — one hosted model. A malformed id (400) and an unhosted one (404) both throw. */
export function getModelState(base: string, id: ModelId): Promise<WireNode> {
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

/** Validate a descriptor loaded from the node or from a file. */
export function validateDescriptor(body: unknown): Descriptor {
  if (typeof body !== 'object' || body === null) {
    throw new ApiError('metamodel descriptor is not a JSON object');
  }
  const desc = body as Partial<Descriptor>;
  if (desc.formatVersion !== 1) {
    throw new ApiError(`unsupported descriptor formatVersion: ${String(desc.formatVersion)}`);
  }
  if (typeof desc.classes !== 'object' || desc.classes === null || !Array.isArray(desc.rootClasses)) {
    throw new ApiError('metamodel descriptor missing classes/rootClasses');
  }
  return desc as Descriptor;
}

/**
 * POST /api/op with the {"JsonKind": op} envelope.
 * Malformed ops are HTTP 400 (throws ApiError); refused ops come back
 * HTTP 200 with success:false — returned as-is for the caller to surface.
 */
export async function postOp(base: string, op: JsonOp): Promise<OpResult> {
  const response = await request(base, '/api/op', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ JsonKind: op }),
  });
  const body = (await readJson(response, '/api/op')) as Record<string, unknown>;
  if (!response.ok) {
    const detail = typeof body['error'] === 'string' ? body['error'] : JSON.stringify(body);
    throw new ApiError(`/api/op returned ${response.status}: ${detail}`, response.status);
  }
  if (typeof body['success'] !== 'boolean') {
    throw new ApiError('/api/op body has no boolean "success" field');
  }
  return { success: body['success'], message: typeof body['message'] === 'string' ? body['message'] : '' };
}
