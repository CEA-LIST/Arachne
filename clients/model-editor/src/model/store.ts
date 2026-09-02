/**
 * The model store: one file per `ModelId`, holding the decoded document.
 *
 * The file is a PROJECTION of the log and never a source of truth. The
 * adaptation layer writes it after each apply the binding check lets through
 * as `bound` (model/projection.ts); nothing reads it to build state. On
 * connect the document comes from the node and the file is overwritten, so a
 * stale or tampered file never survives a reconnect as rendered state, and a
 * restart trusts the log. The `read` here exists for inspection (a test, a
 * person opening the model file) and for nothing else: the sync path
 * (sync/useSync.ts) never calls it.
 *
 * Naming and bytes. The file for model `a1b2…` is `a1b2….json` in the store's
 * one directory; the id is checked for its 32-hex shape before it becomes a
 * file name, since it arrives over the wire in the header. The content is the
 * canonical JSON of the document, compact with keys sorted at every level,
 * the same text model/digest.ts hashes, and nothing else, no trailing newline:
 * so `sha256(file bytes)` is `sha256(canonicalJson(doc))`, and two editors'
 * files for one model can be digested and compared.
 *
 * Backends. The browser writes the origin-private file system (OPFS), chosen
 * over the File System Access API because it needs no permission prompt and
 * no user gesture, works in headless Chrome, and is private to the origin;
 * exporting to a folder of the user's choice is a later action. Tests and the
 * level-4 harness use the same store over a directory (model/fsStore.ts). Both
 * are a `TextFiles` under one `FileModelStore`, so the model-level rules above
 * are written once. Every backend failure surfaces as a `StoreError`; the
 * projection layer turns it into a report and keeps the view.
 */

import { isModelId, type ModelId, type PlainJson } from '../api/types';
import { canonicalJson, sha256Hex } from './digest';

/** Any failure of the store: an unavailable backend, quota, a name that is not a model id, a file that is not JSON. */
export class StoreError extends Error {
  constructor(message: string, options?: { cause?: unknown }) {
    super(message, options);
    this.name = 'StoreError';
  }
}

/** What one write left on disk. */
export interface StoredFile {
  modelId: ModelId;
  /** The file name inside the store's directory. */
  file: string;
  /** UTF-8 length of the text written. */
  bytes: number;
  /** SHA-256, lowercase hex, of the text written: the projection's digest. */
  digest: string;
}

export interface ModelStore {
  /** Where the files are, for a person who wants to look. */
  readonly location: string;
  /** Write the document as the model's file, replacing what was there. */
  write(modelId: ModelId, doc: PlainJson): Promise<StoredFile>;
  /** The stored document, or null when the model has no file. Inspection only: never the view's source. */
  read(modelId: ModelId): Promise<PlainJson | null>;
  /** The ids that have a file, sorted. */
  list(): Promise<ModelId[]>;
  /** Delete the model's file; a model with no file is not an error. */
  remove(modelId: ModelId): Promise<void>;
}

/* ---------- the model-level rules, shared by every backend ---------- */

export const MODEL_FILE_SUFFIX = '.json';

/** `<modelId>.json`; throws when the id does not have the 32-hex shape, so no header can name a path. */
export function modelFileName(modelId: ModelId): string {
  if (!isModelId(modelId)) {
    throw new StoreError(`not a model id, so not a file name: ${JSON.stringify(modelId).slice(0, 80)}`);
  }
  return `${modelId}${MODEL_FILE_SUFFIX}`;
}

/** The inverse of `modelFileName`: the id a file name is for, or null for any other file. */
export function modelIdOfFile(name: string): ModelId | null {
  if (!name.endsWith(MODEL_FILE_SUFFIX)) return null;
  const id = name.slice(0, -MODEL_FILE_SUFFIX.length);
  return isModelId(id) ? id : null;
}

/** The bytes of a model file: canonical JSON, the text model/digest.ts hashes. */
export function serializeModel(doc: PlainJson): string {
  return canonicalJson(doc);
}

/** A model file's text back into a document; a file that is not JSON is a StoreError. */
export function parseModel(text: string): PlainJson {
  try {
    return JSON.parse(text) as PlainJson;
  } catch (err) {
    throw new StoreError(`model file is not JSON: ${err instanceof Error ? err.message : String(err)}`, {
      cause: err,
    });
  }
}

/* ---------- backends: text under a name, nothing about models ---------- */

export interface TextFiles {
  readonly location: string;
  /** Replace the file's content whole; a reader never sees a partial file. */
  write(name: string, text: string): Promise<void>;
  /** The file's text, or null when there is no such file. */
  read(name: string): Promise<string | null>;
  /** Every file name in the directory. */
  list(): Promise<string[]>;
  /** Delete the file; no such file is not an error. */
  remove(name: string): Promise<void>;
}

/**
 * The store over a backend. Operations run one at a time, in order, so a
 * write never interleaves with another write of the same file and a read
 * that follows a write sees it.
 */
export class FileModelStore implements ModelStore {
  readonly #files: TextFiles;
  #queue: Promise<unknown> = Promise.resolve();

  constructor(files: TextFiles) {
    this.#files = files;
  }

  get location(): string {
    return this.#files.location;
  }

  #serial<T>(task: () => Promise<T>): Promise<T> {
    const run = this.#queue.then(task, task);
    this.#queue = run.catch(() => undefined);
    return run;
  }

  async write(modelId: ModelId, doc: PlainJson): Promise<StoredFile> {
    const file = modelFileName(modelId);
    const text = serializeModel(doc);
    await this.#serial(() => this.#files.write(file, text));
    return { modelId, file, bytes: new TextEncoder().encode(text).length, digest: await sha256Hex(text) };
  }

  async read(modelId: ModelId): Promise<PlainJson | null> {
    const file = modelFileName(modelId);
    const text = await this.#serial(() => this.#files.read(file));
    return text === null ? null : parseModel(text);
  }

  async list(): Promise<ModelId[]> {
    const names = await this.#serial(() => this.#files.list());
    return names
      .map(modelIdOfFile)
      .filter((id): id is ModelId => id !== null)
      .sort();
  }

  async remove(modelId: ModelId): Promise<void> {
    const file = modelFileName(modelId);
    await this.#serial(() => this.#files.remove(file));
  }
}

/* ---------- the browser backend: the origin-private file system ---------- */

/** The name of the store's directory under the origin's OPFS root. */
export const OPFS_MODELS_DIR = 'models';

/**
 * The slice of `FileSystemDirectoryHandle` the backend uses. The real handle
 * satisfies it as it is; a test fakes it in memory, which is how the backend
 * runs under vitest with no browser.
 */
export interface OpfsDirectory {
  getFileHandle(name: string, options?: { create?: boolean }): Promise<OpfsFile>;
  removeEntry(name: string): Promise<void>;
  keys(): AsyncIterable<string>;
}

export interface OpfsFile {
  getFile(): Promise<{ text(): Promise<string> }>;
  createWritable(): Promise<{
    write(data: string): Promise<void>;
    close(): Promise<void>;
    abort(reason?: unknown): Promise<void>;
  }>;
}

/** The OPFS root as `navigator.storage.getDirectory()` answers it. */
export interface OpfsRoot {
  getDirectoryHandle(name: string, options?: { create?: boolean }): Promise<OpfsDirectory>;
}

function isNotFound(err: unknown): boolean {
  return err instanceof Error && err.name === 'NotFoundError';
}

function wrap(what: string, err: unknown): StoreError {
  if (err instanceof StoreError) return err;
  const name = err instanceof Error && err.name !== 'Error' ? `${err.name}: ` : '';
  return new StoreError(`${what}: ${name}${err instanceof Error ? err.message : String(err)}`, { cause: err });
}

/**
 * A `TextFiles` over one OPFS directory, opened on first use through
 * `openDirectory`. A failed open is not remembered, so a later call tries
 * again. `createWritable` writes to a swap file the browser commits on
 * `close`, which is what makes a write whole or absent.
 */
export function opfsFilesBackend(openDirectory: () => Promise<OpfsDirectory>, location = `opfs:/${OPFS_MODELS_DIR}`): TextFiles {
  let directory: Promise<OpfsDirectory> | null = null;
  const dir = (): Promise<OpfsDirectory> => {
    if (directory === null) {
      directory = openDirectory().catch((err: unknown) => {
        directory = null;
        throw wrap('the origin-private file system could not be opened', err);
      });
    }
    return directory;
  };
  return {
    location,
    async write(name, text) {
      let writable: Awaited<ReturnType<OpfsFile['createWritable']>>;
      try {
        const handle = await (await dir()).getFileHandle(name, { create: true });
        writable = await handle.createWritable();
      } catch (err) {
        throw wrap(`cannot open ${name} for writing`, err);
      }
      try {
        await writable.write(text);
        await writable.close();
      } catch (err) {
        await writable.abort().catch(() => undefined);
        throw wrap(`cannot write ${name}`, err);
      }
    },
    async read(name) {
      let handle: OpfsFile;
      try {
        handle = await (await dir()).getFileHandle(name);
      } catch (err) {
        if (isNotFound(err)) return null;
        throw wrap(`cannot open ${name}`, err);
      }
      try {
        return await (await handle.getFile()).text();
      } catch (err) {
        throw wrap(`cannot read ${name}`, err);
      }
    },
    async list() {
      const names: string[] = [];
      try {
        for await (const name of (await dir()).keys()) names.push(name);
      } catch (err) {
        throw wrap('cannot list the store', err);
      }
      return names;
    },
    async remove(name) {
      try {
        await (await dir()).removeEntry(name);
      } catch (err) {
        if (isNotFound(err)) return;
        throw wrap(`cannot remove ${name}`, err);
      }
    },
  };
}

/** The store over the OPFS directory `openDirectory` answers. */
export function opfsModelStore(openDirectory: () => Promise<OpfsDirectory>): ModelStore {
  return new FileModelStore(opfsFilesBackend(openDirectory));
}

/** What the browser exposes as `navigator.storage`, when it does. */
export interface BrowserStorage {
  getDirectory(): Promise<OpfsRoot>;
}

/**
 * The store for the app: OPFS, directory `models` under the origin's root.
 * Null where there is no origin-private file system (a browser without it, a
 * non-browser context), which the caller reports as an unavailable store and
 * nothing more: applying never depends on it. OPFS needs a secure context
 * (https, localhost, 127.0.0.1), as `crypto.subtle` in model/digest.ts does.
 */
export function openBrowserStore(
  storage: BrowserStorage | undefined = (globalThis as { navigator?: { storage?: BrowserStorage } }).navigator
    ?.storage,
): ModelStore | null {
  if (storage === undefined || typeof storage.getDirectory !== 'function') return null;
  return opfsModelStore(async () =>
    (await storage.getDirectory()).getDirectoryHandle(OPFS_MODELS_DIR, { create: true }),
  );
}
