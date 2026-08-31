import type { PlainJson } from '../api/types';
import { setAtPath } from '../crdt/path';

/**
 * Focus preservation, part 1 (the qa_editor algorithm): a field is "actively
 * typing" iff it has DOM focus AND was edited within TYPING_THRESHOLD_MS.
 * Such fields are never overwritten by a poll refresh; instead their local
 * value is written back into the refreshed document copy, so the rendered
 * state and the diff baselines stay coherent while the user types.
 *
 * Text inputs register a snapshot getter here; the poll loop calls overlay()
 * on every refreshed document before it reaches the store.
 */

export const TYPING_THRESHOLD_MS = 500;

export interface FieldSnapshot {
  value: string;
  lastEditAt: number;
  focused: boolean;
}

export class FieldRegistry {
  private readonly fields = new Map<string, () => FieldSnapshot>();

  /** `key` is pathKey(path) of the string the field edits. */
  register(key: string, getSnapshot: () => FieldSnapshot): () => void {
    this.fields.set(key, getSnapshot);
    return () => {
      if (this.fields.get(key) === getSnapshot) this.fields.delete(key);
    };
  }

  /** Write every actively-typing field's local value back into `doc`. */
  overlay(doc: PlainJson, now: number = Date.now()): PlainJson {
    let result = doc;
    for (const [key, getSnapshot] of this.fields) {
      const snap = getSnapshot();
      if (snap.focused && now - snap.lastEditAt < TYPING_THRESHOLD_MS) {
        result = setAtPath(result, JSON.parse(key) as (string | number)[], snap.value);
      }
    }
    return result;
  }
}
