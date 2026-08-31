/**
 * Live-synced input primitives implementing the qa_editor focus-preservation
 * algorithm. The editor's forms are built from these.
 *
 * SyncedTextInput (strings):
 * - on input: record the edit time, debounce (300 ms) a flush; flush also on
 *   blur and Enter. A flush diffs the per-field baseline against the local
 *   value into DeleteRange+Insert ops and enqueues them as ONE batch on the
 *   global FIFO, then advances the baseline.
 * - on remote refresh: an actively-typing field (focused + edited within the
 *   typing threshold) is never overwritten — the FieldRegistry already wrote
 *   its local value back into the doc. A focused-but-idle field IS updated,
 *   with selectionStart/End saved and restored around the assignment.
 *
 * NumberInput / BoolInput / EnumSelect: commit on blur/Enter (numbers as a
 * single relative Inc), immediately on toggle/select.
 */

import { useCallback, useEffect, useRef, useState } from 'react';
import type { JsonOp, Path } from '../api/types';
import { pathKey } from '../crdt/path';
import { setBooleanOps, setNumberOps, setStringOps } from '../crdt/ops';
import { FieldRegistry, TYPING_THRESHOLD_MS } from '../sync/fieldRegistry';
import type { BatchOutcome } from '../sync/opQueue';

export const DEBOUNCE_MS = 300;

export type SendOps = (
  description: string,
  ops: JsonOp[],
  optimistic?: { path: Path; value: import('../api/types').PlainJson },
) => Promise<BatchOutcome>;

interface SyncedTextInputProps {
  path: Path;
  /** Human name of the field for the action log, e.g. "TreeNode.name". */
  label: string;
  remoteValue: string;
  registry: FieldRegistry;
  sendOps: SendOps;
  placeholder?: string;
}

export function SyncedTextInput({
  path,
  label,
  remoteValue,
  registry,
  sendOps,
  placeholder,
}: SyncedTextInputProps) {
  const inputRef = useRef<HTMLInputElement | null>(null);
  const [localValue, setLocalValue] = useState(remoteValue);
  const localRef = useRef(localValue);
  localRef.current = localValue;
  /** Last value known to be on the server (diff baseline). */
  const baselineRef = useRef(remoteValue);
  const lastEditAtRef = useRef(0);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const flushingRef = useRef(false);

  const key = pathKey(path);

  // Register with the poll loop's overlay.
  useEffect(() => {
    return registry.register(key, () => ({
      value: localRef.current,
      lastEditAt: lastEditAtRef.current,
      focused: inputRef.current !== null && document.activeElement === inputRef.current,
    }));
  }, [registry, key]);

  const flush = useCallback(async () => {
    if (flushingRef.current) return;
    const target = localRef.current;
    const baseline = baselineRef.current;
    if (target === baseline) return;
    const ops = setStringOps(path, baseline, target);
    flushingRef.current = true;
    try {
      const result = await sendOps(`set ${label}`, ops, { path, value: target });
      if (result.outcome === 'ok') {
        baselineRef.current = target;
        // More typing may have happened during the await; re-flush.
        if (localRef.current !== target) void flush();
      }
      // refused/error: keep the old baseline; the poll loop reconciles and
      // the banner/log already carry the failure.
    } finally {
      flushingRef.current = false;
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [label, sendOps, key]);

  // Remote refresh handling.
  useEffect(() => {
    const input = inputRef.current;
    const focused = input !== null && document.activeElement === input;
    const activelyTyping = focused && Date.now() - lastEditAtRef.current < TYPING_THRESHOLD_MS;
    if (activelyTyping) return; // overlay preserved our value; never clobber
    if (remoteValue === localRef.current) {
      baselineRef.current = remoteValue;
      return;
    }
    if (focused && input !== null) {
      const start = input.selectionStart;
      const end = input.selectionEnd;
      setLocalValue(remoteValue);
      baselineRef.current = remoteValue;
      requestAnimationFrame(() => {
        if (document.activeElement === input) {
          const max = remoteValue.length;
          input.setSelectionRange(Math.min(start ?? max, max), Math.min(end ?? max, max));
        }
      });
    } else {
      setLocalValue(remoteValue);
      baselineRef.current = remoteValue;
    }
  }, [remoteValue]);

  useEffect(() => {
    return () => {
      if (debounceRef.current !== null) clearTimeout(debounceRef.current);
    };
  }, []);

  return (
    <input
      ref={inputRef}
      type="text"
      value={localValue}
      placeholder={placeholder}
      onChange={(e) => {
        setLocalValue(e.target.value);
        lastEditAtRef.current = Date.now();
        if (debounceRef.current !== null) clearTimeout(debounceRef.current);
        debounceRef.current = setTimeout(() => void flush(), DEBOUNCE_MS);
      }}
      onBlur={() => {
        if (debounceRef.current !== null) clearTimeout(debounceRef.current);
        void flush();
      }}
      onKeyDown={(e) => {
        if (e.key === 'Enter') {
          if (debounceRef.current !== null) clearTimeout(debounceRef.current);
          void flush();
        }
      }}
    />
  );
}

interface NumberInputProps {
  path: Path;
  label: string;
  remoteValue: number;
  sendOps: SendOps;
  integer?: boolean;
}

/** Numbers commit on blur/Enter only, as a single relative Inc. */
export function NumberInput({ path, label, remoteValue, sendOps, integer }: NumberInputProps) {
  const [text, setText] = useState(String(remoteValue));
  const editingRef = useRef(false);

  useEffect(() => {
    if (!editingRef.current) setText(String(remoteValue));
  }, [remoteValue]);

  const commit = useCallback(() => {
    editingRef.current = false;
    const parsed = integer === true ? parseInt(text, 10) : parseFloat(text);
    if (Number.isNaN(parsed)) {
      setText(String(remoteValue));
      return;
    }
    const ops = setNumberOps(path, remoteValue, parsed);
    if (ops.length > 0) {
      void sendOps(`set ${label} = ${parsed}`, ops, { path, value: parsed });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [text, remoteValue, integer, label, sendOps, pathKey(path)]);

  return (
    <input
      type="number"
      step={integer === true ? 1 : 'any'}
      value={text}
      onChange={(e) => {
        editingRef.current = true;
        setText(e.target.value);
      }}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === 'Enter') commit();
      }}
    />
  );
}

interface BoolInputProps {
  path: Path;
  label: string;
  remoteValue: boolean;
  sendOps: SendOps;
}

export function BoolInput({ path, label, remoteValue, sendOps }: BoolInputProps) {
  return (
    <input
      type="checkbox"
      checked={remoteValue}
      onChange={(e) => {
        const next = e.target.checked;
        void sendOps(`set ${label} = ${next}`, setBooleanOps(path, next), { path, value: next });
      }}
    />
  );
}

interface EnumSelectProps {
  path: Path;
  label: string;
  remoteValue: string;
  literals: string[];
  sendOps: SendOps;
}

/** Enums are strings on the wire: the select commits a full string diff. */
export function EnumSelect({ path, label, remoteValue, literals, sendOps }: EnumSelectProps) {
  return (
    <select
      value={remoteValue}
      onChange={(e) => {
        const next = e.target.value;
        void sendOps(`set ${label} = ${next}`, setStringOps(path, remoteValue, next), {
          path,
          value: next,
        });
      }}
    >
      {!literals.includes(remoteValue) && <option value={remoteValue}>{remoteValue === '' ? '(unset)' : remoteValue}</option>}
      {literals.map((lit) => (
        <option key={lit} value={lit}>
          {lit}
        </option>
      ))}
    </select>
  );
}
