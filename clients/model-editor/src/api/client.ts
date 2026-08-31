/**
 * Typed client for the moirai node HTTP API.
 *
 * Endpoints (this base): GET /api/health, GET /api/state, POST /api/op,
 * GET /api/metamodel (404 when the node serves no descriptor).
 *
 * Error contract: network failures and non-OK statuses throw ApiError;
 * POST /api/op additionally returns {"success": false, ...} with HTTP 200 for
 * well-formed but refused ops — callers MUST branch on `.success`, and the op
 * queue turns that case into a visible error (never swallowed).
 */

import type { Descriptor, JsonOp, OpResult, WireNode } from './types';

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

/** GET /api/state — unwraps the {"json": ...} envelope. */
export async function getState(base: string): Promise<WireNode> {
  const response = await request(base, '/api/state');
  if (!response.ok) throw new ApiError(`/api/state returned ${response.status}`, response.status);
  const body = (await readJson(response, '/api/state')) as Record<string, unknown>;
  if (!('json' in body)) throw new ApiError('/api/state body has no "json" field');
  return body['json'] as WireNode;
}

/**
 * GET /api/metamodel — the node's descriptor, or null when the node serves
 * none (404). Validates formatVersion so a future format fails loudly.
 */
export async function getMetamodel(base: string): Promise<Descriptor | null> {
  const response = await request(base, '/api/metamodel');
  if (response.status === 404) return null;
  if (!response.ok) throw new ApiError(`/api/metamodel returned ${response.status}`, response.status);
  const body = await readJson(response, '/api/metamodel');
  return validateDescriptor(body);
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
