/// <reference types="node" />
// The store's two backends, held to one contract: the Node fs one for real in
// a temporary directory, the OPFS one over an in-memory fake of the directory
// handle, which is how it runs under vitest with no browser. The browser's
// own `navigator.storage.getDirectory()` is exercised by the level-4 harness.
import { mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterAll, describe, expect, it } from 'vitest';
import type { PlainJson } from '../api/types';
import { canonicalJson, sha256Hex } from './digest';
import { fsModelStore } from './fsStore';
import {
  FileModelStore,
  MODEL_FILE_SUFFIX,
  modelFileName,
  modelIdOfFile,
  openBrowserStore,
  OPFS_MODELS_DIR,
  opfsFilesBackend,
  opfsModelStore,
  parseModel,
  serializeModel,
  StoreError,
  type ModelStore,
  type OpfsDirectory,
  type OpfsFile,
  type TextFiles,
} from './store';

const A = 'a1b2c3d4a1b2c3d4a1b2c3d4a1b2c3d4';
const C = 'c3d4e5f6c3d4e5f6c3d4e5f6c3d4e5f6';

const docA: PlainJson = {
  eClass: 'Root',
  behaviortrees: [{ eClass: 'BehaviorTree', ID: 'main', child: { eClass: 'Sequence', name: 'root' } }],
  __model: { modelId: A, metamodelId: { nsURI: 'http://www.example.org/behaviortree', digest: 'f8e3' } },
};
const docC: PlainJson = { eClass: 'Model', classes: [{ eClass: 'Class', name: 'Door' }] };

/* ---------- an in-memory OPFS directory ---------- */

function notFound(name: string): Error {
  const err = new Error(`no entry ${name}`);
  err.name = 'NotFoundError';
  return err;
}

/** The slice of a directory handle the backend touches, in memory, with knobs for failure. */
class FakeOpfsDirectory implements OpfsDirectory {
  readonly files = new Map<string, string>();
  /** Set to make every `close` fail as a browser out of quota does. */
  failClose: Error | null = null;

  async getFileHandle(name: string, options?: { create?: boolean }): Promise<OpfsFile> {
    if (!this.files.has(name)) {
      if (!options?.create) throw notFound(name);
      this.files.set(name, '');
    }
    return {
      getFile: async () => ({ text: async () => this.files.get(name) ?? '' }),
      createWritable: async () => {
        let staged = '';
        let open = true;
        return {
          write: async (data: string) => {
            staged += data;
          },
          close: async () => {
            if (this.failClose !== null) throw this.failClose;
            if (open) this.files.set(name, staged);
            open = false;
          },
          abort: async () => {
            open = false;
          },
        };
      },
    };
  }

  async removeEntry(name: string): Promise<void> {
    if (!this.files.delete(name)) throw notFound(name);
  }

  async *keys(): AsyncIterable<string> {
    yield* [...this.files.keys()];
  }
}

/* ---------- the shared rules ---------- */

describe('model file names', () => {
  it('names the file by the model id with the json suffix', () => {
    expect(modelFileName(A)).toBe(`${A}.json`);
    expect(MODEL_FILE_SUFFIX).toBe('.json');
  });

  it('refuses anything that is not a 32-hex model id before it becomes a path', () => {
    for (const bad of ['../../etc/passwd', 'A1B2C3D4A1B2C3D4A1B2C3D4A1B2C3D4', A.slice(1), `${A}0`, '', 'models']) {
      expect(() => modelFileName(bad)).toThrow(StoreError);
    }
  });

  it('reads the id back from a file name and ignores every other file', () => {
    expect(modelIdOfFile(`${A}.json`)).toBe(A);
    expect(modelIdOfFile(`${A}.txt`)).toBeNull();
    expect(modelIdOfFile('notes.json')).toBeNull();
    expect(modelIdOfFile(`.${A}.json.123.0.tmp`)).toBeNull();
    expect(modelIdOfFile(`${A.toUpperCase()}.json`)).toBeNull();
  });
});

describe('model file bytes', () => {
  it('serializes as the canonical JSON the digest is over, and parses back to the same document', () => {
    const text = serializeModel(docA);
    expect(text).toBe(canonicalJson(docA));
    expect(text).not.toContain('\n');
    expect(text.startsWith('{"__model":')).toBe(true);
    expect(parseModel(text)).toEqual(docA);
  });

  it('a file that is not JSON is a StoreError', () => {
    expect(() => parseModel('{')).toThrow(StoreError);
    expect(() => parseModel('')).toThrow(StoreError);
  });
});

/* ---------- one contract, two backends ---------- */

interface Backend {
  name: string;
  store: ModelStore;
  /** The raw bytes of a file in the backend, or null. */
  raw: (file: string) => string | null;
  /** Every file name in the backend, whatever it is. */
  names: () => string[];
}

const tempDir = mkdtempSync(join(tmpdir(), 'model-store-'));
afterAll(() => rmSync(tempDir, { recursive: true, force: true }));

const fake = new FakeOpfsDirectory();

const backends: Backend[] = [
  {
    name: 'node fs directory',
    store: fsModelStore(join(tempDir, 'models')),
    raw: (file) => {
      try {
        return readFileSync(join(tempDir, 'models', file), 'utf8');
      } catch {
        return null;
      }
    },
    names: () => {
      try {
        return readdirSync(join(tempDir, 'models'));
      } catch {
        return [];
      }
    },
  },
  {
    name: 'origin-private file system (faked in memory)',
    store: opfsModelStore(async () => fake),
    raw: (file) => fake.files.get(file) ?? null,
    names: () => [...fake.files.keys()],
  },
];

describe.each(backends)('the model store over $name', ({ store, raw, names }) => {
  it('starts empty: nothing listed, nothing to read', async () => {
    expect(await store.list()).toEqual([]);
    expect(await store.read(A)).toBeNull();
  });

  it('writes one file per model id, whose bytes are exactly the canonical JSON of the document', async () => {
    const written = await store.write(A, docA);
    const text = canonicalJson(docA);
    expect(written).toEqual({
      modelId: A,
      file: `${A}.json`,
      bytes: new TextEncoder().encode(text).length,
      digest: await sha256Hex(text),
    });
    expect(raw(`${A}.json`)).toBe(text);
    expect(JSON.parse(raw(`${A}.json`) ?? '')).toEqual(docA);
    expect(await store.read(A)).toEqual(docA);
    expect(await store.list()).toEqual([A]);
  });

  it('a second model gets its own file and the listing is sorted', async () => {
    await store.write(C, docC);
    expect(await store.list()).toEqual([A, C].sort());
    expect(await store.read(C)).toEqual(docC);
    expect(await store.read(A)).toEqual(docA);
  });

  it('a rewrite replaces the file whole', async () => {
    const smaller: PlainJson = { eClass: 'Root', behaviortrees: [] };
    await store.write(A, smaller);
    expect(raw(`${A}.json`)).toBe(canonicalJson(smaller));
    expect(await store.read(A)).toEqual(smaller);
    await store.write(A, docA);
    expect(await store.read(A)).toEqual(docA);
  });

  it('writes issued together land in order, the last one winning, with no swap file left behind', async () => {
    const docs = Array.from({ length: 20 }, (_, i): PlainJson => ({ eClass: 'Root', n: i }));
    await Promise.all(docs.map((doc) => store.write(C, doc)));
    expect(await store.read(C)).toEqual(docs[docs.length - 1]);
    expect(names().filter((name) => name.endsWith('.tmp'))).toEqual([]);
  });

  it('remove deletes the file, twice is fine, and the model reads as absent', async () => {
    await store.remove(C);
    await store.remove(C);
    expect(await store.read(C)).toBeNull();
    expect(await store.list()).toEqual([A]);
    expect(raw(`${C}.json`)).toBeNull();
  });

  it('refuses a name that is not a model id on every operation', async () => {
    const bad = '../escape';
    await expect(store.write(bad, docA)).rejects.toThrow(StoreError);
    await expect(store.read(bad)).rejects.toThrow(StoreError);
    await expect(store.remove(bad)).rejects.toThrow(StoreError);
    expect(names().some((name) => name.includes('escape'))).toBe(false);
  });

  it('a tampered file that is not JSON reads as a StoreError, and a rewrite repairs it', async () => {
    // A store never trusts its file; here it merely reports it.
    if (store.location.startsWith('opfs:')) fake.files.set(`${A}.json`, '{not json');
    else writeFileSync(join(store.location, `${A}.json`), '{not json');
    await expect(store.read(A)).rejects.toThrow(StoreError);
    await store.write(A, docA);
    expect(await store.read(A)).toEqual(docA);
  });

  it('lists only model files, whatever else the directory holds', async () => {
    if (store.location.startsWith('opfs:')) fake.files.set('notes.txt', 'x');
    else writeFileSync(join(store.location, 'notes.txt'), 'x');
    expect(await store.list()).toEqual([A]);
  });
});

/* ---------- OPFS specifics ---------- */

describe('the OPFS backend', () => {
  it('opens the directory once, and tries again after a failed open', async () => {
    let attempts = 0;
    let available = false;
    const dir = new FakeOpfsDirectory();
    const files = opfsFilesBackend(async () => {
      attempts++;
      if (!available) throw new DOMException('no storage', 'SecurityError');
      return dir;
    });
    await expect(files.write('x', 'y')).rejects.toThrow(StoreError);
    await expect(files.write('x', 'y')).rejects.toThrow(/SecurityError: no storage/);
    expect(attempts).toBe(2);
    available = true;
    await files.write('x', 'y');
    await files.write('z', 'w');
    expect(attempts).toBe(3);
    expect(dir.files.get('x')).toBe('y');
  });

  it('a failed close (quota) is a StoreError naming the cause, and the old content stands', async () => {
    const dir = new FakeOpfsDirectory();
    const store = opfsModelStore(async () => dir);
    await store.write(A, docA);
    dir.failClose = new DOMException('The quota has been exceeded.', 'QuotaExceededError');
    await expect(store.write(A, docC)).rejects.toThrow(/QuotaExceededError: The quota has been exceeded/);
    dir.failClose = null;
    expect(await store.read(A)).toEqual(docA);
  });

  it('read and remove treat NotFoundError as absence and surface anything else', async () => {
    const dir = new FakeOpfsDirectory();
    const files = opfsFilesBackend(async () => dir);
    expect(await files.read('missing')).toBeNull();
    await files.remove('missing');
    dir.getFileHandle = async () => {
      throw new DOMException('gone', 'InvalidStateError');
    };
    await expect(files.read('x')).rejects.toThrow(/InvalidStateError: gone/);
  });
});

describe('openBrowserStore', () => {
  it('is null in a context with no origin-private file system, such as this one', () => {
    expect(openBrowserStore()).toBeNull();
    expect(openBrowserStore(undefined)).toBeNull();
  });

  it('opens the models directory under the origin root when the browser has one', async () => {
    const root = new Map<string, FakeOpfsDirectory>();
    const store = openBrowserStore({
      getDirectory: async () => ({
        getDirectoryHandle: async (name: string, options?: { create?: boolean }) => {
          let dir = root.get(name);
          if (dir === undefined) {
            if (!options?.create) throw notFound(name);
            dir = new FakeOpfsDirectory();
            root.set(name, dir);
          }
          return dir;
        },
      }),
    });
    expect(store).not.toBeNull();
    if (store === null) return;
    expect(store.location).toBe(`opfs:/${OPFS_MODELS_DIR}`);
    await store.write(A, docA);
    expect([...root.keys()]).toEqual([OPFS_MODELS_DIR]);
    expect(root.get(OPFS_MODELS_DIR)?.files.get(`${A}.json`)).toBe(canonicalJson(docA));
  });
});

describe('FileModelStore', () => {
  it('runs operations one at a time, so a read that follows a write sees it', async () => {
    const order: string[] = [];
    let content: string | null = null;
    const slow: TextFiles = {
      location: 'slow',
      async write(_name, text) {
        order.push('write-start');
        await new Promise((resolve) => setTimeout(resolve, 5));
        content = text;
        order.push('write-end');
      },
      async read() {
        order.push('read');
        return content;
      },
      async list() {
        return [];
      },
      async remove() {},
    };
    const store = new FileModelStore(slow);
    const [, read] = await Promise.all([store.write(A, docA), store.read(A)]);
    expect(read).toEqual(docA);
    expect(order).toEqual(['write-start', 'write-end', 'read']);
  });

  it('a failed operation does not block the ones after it', async () => {
    let fail = true;
    const flaky: TextFiles = {
      location: 'flaky',
      async write() {
        if (fail) throw new Error('disk on fire');
      },
      async read() {
        return null;
      },
      async list() {
        return [];
      },
      async remove() {},
    };
    const store = new FileModelStore(flaky);
    await expect(store.write(A, docA)).rejects.toThrow('disk on fire');
    fail = false;
    await expect(store.write(A, docA)).resolves.toMatchObject({ modelId: A });
  });
});
