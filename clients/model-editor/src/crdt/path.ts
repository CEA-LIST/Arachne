import type { Path, PlainJson } from '../api/types';

/** Stable string key for a path (Map/Record keying). */
export function pathKey(path: Path): string {
  return JSON.stringify(path);
}

/** Read the value at `path`, or undefined when the path does not exist. */
export function getAtPath(doc: PlainJson, path: Path): PlainJson | undefined {
  let node: PlainJson | undefined = doc;
  for (const seg of path) {
    if (node === null || node === undefined || typeof node !== 'object') return undefined;
    if (typeof seg === 'number') {
      if (!Array.isArray(node)) return undefined;
      node = node[seg];
    } else {
      if (Array.isArray(node)) return undefined;
      node = node[seg];
    }
  }
  return node;
}

/**
 * Return a copy of `doc` with `value` written at `path` (containers along the
 * path are shallow-copied; missing intermediate containers abort and return
 * `doc` unchanged — the poll loop will reconcile).
 */
export function setAtPath(doc: PlainJson, path: Path, value: PlainJson): PlainJson {
  if (path.length === 0) return value;
  if (doc === null || typeof doc !== 'object') return doc;
  const [head, ...rest] = path;
  if (typeof head === 'number') {
    if (!Array.isArray(doc) || head < 0 || head >= doc.length) return doc;
    const copy = doc.slice();
    copy[head] = setAtPath(doc[head], rest, value);
    return copy;
  }
  if (Array.isArray(doc) || !(head in doc)) {
    if (Array.isArray(doc)) return doc;
    // Creating a new key is fine for optimistic patches.
    return { ...doc, [head]: setAtPath(rest.length > 0 ? {} : null, rest, value) };
  }
  return { ...doc, [head]: setAtPath(doc[head], rest, value) };
}
