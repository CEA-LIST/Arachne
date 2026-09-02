/// <reference types="node" />
/**
 * The model store over a directory, through Node.js `fs`: the backend the
 * tests and the level-4 harness use. Same store, same rules (model/store.ts);
 * only the bytes land in a directory of the caller's choosing. The app never
 * imports this module, so the browser bundle carries no `node:fs`.
 *
 * A write goes to a temporary name beside the file and is renamed over it,
 * so a reader sees the old file or the new one and never a partial one.
 */

import { mkdir, readFile, readdir, rename, rm, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { FileModelStore, type ModelStore, type TextFiles } from './store';

function isMissing(err: unknown): boolean {
  return typeof err === 'object' && err !== null && (err as { code?: unknown }).code === 'ENOENT';
}

let swapCounter = 0;

/** A `TextFiles` over `dir`, which is created on the first write. */
export function fsFilesBackend(dir: string): TextFiles {
  return {
    location: dir,
    async write(name, text) {
      await mkdir(dir, { recursive: true });
      const swap = join(dir, `.${name}.${process.pid}.${swapCounter++}.tmp`);
      await writeFile(swap, text, 'utf8');
      await rename(swap, join(dir, name));
    },
    async read(name) {
      try {
        return await readFile(join(dir, name), 'utf8');
      } catch (err) {
        if (isMissing(err)) return null;
        throw err;
      }
    },
    async list() {
      try {
        return await readdir(dir);
      } catch (err) {
        if (isMissing(err)) return [];
        throw err;
      }
    },
    remove(name) {
      return rm(join(dir, name), { force: true });
    },
  };
}

/** The model store over a directory. */
export function fsModelStore(dir: string): ModelStore {
  return new FileModelStore(fsFilesBackend(dir));
}
