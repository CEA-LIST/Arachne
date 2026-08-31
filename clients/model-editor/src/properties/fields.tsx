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
 * - Esc reverts the field to the last server baseline and blurs. No op, no
 *   store change: the baseline is what the node last confirmed.
 *
 * NumberInput / BoolInput / EnumSelect: commit on blur/Enter (numbers as a
 * single relative Inc), immediately on toggle/select.
 *
 * Each field owns a small state slot (pending dot -> check -> danger dot with
 * the node's own message and a Retry). Per-field truth, no modal, no layout
 * shift — the redesign's "visible but calm" rule.
 */

import { useCallback, useEffect, useRef, useState } from 'react';
import type { JsonOp, Path } from '../api/types';
import { pathKey } from '../crdt/path';
import { setBooleanOps, setNumberOps, setStringOps } from '../crdt/ops';
import { FieldRegistry, TYPING_THRESHOLD_MS } from '../sync/fieldRegistry';
import type { BatchOutcome } from '../sync/opQueue';
import { Check } from '../ui/icons';
import { ICON } from '../ui/iconProps';

export const DEBOUNCE_MS = 300;
/** How long the success check stays before it fades. */
export const OK_FLASH_MS = 1200;

export type SendOps = (
  description: string,
  ops: JsonOp[],
  optimistic?: { path: Path; value: import('../api/types').PlainJson },
) => Promise<BatchOutcome>;

type FieldStatus = 'idle' | 'pending' | 'ok' | 'error';

interface FieldStateSlotProps {
  status: FieldStatus;
  message: string | null;
  onRetry: () => void;
}

/** The 6px slot at a field's right edge, plus the failure line beneath it. */
function FieldStateSlot({ status, message, onRetry }: FieldStateSlotProps) {
  return (
    <>
      <span className="me-field__slot" aria-hidden={status === 'idle'}>
        {status === 'pending' && <span className="me-dot me-dot--accent me-dot--pulse" />}
        {status === 'ok' && <Check {...ICON} size={13} className="me-field__ok" />}
        {status === 'error' && <span className="me-dot me-dot--danger" />}
      </span>
      {status === 'error' && message !== null && (
        <span className="me-form__error" role="status">
          {message}
          <button type="button" className="me-btn me-btn--sm" onClick={onRetry}>
            Retry
          </button>
        </span>
      )}
    </>
  );
}

/** Shared status bookkeeping: pending -> ok (fades) | error (sticks). */
function useFieldStatus() {
  const [status, setStatus] = useState<FieldStatus>('idle');
  const [message, setMessage] = useState<string | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(
    () => () => {
      if (timer.current !== null) clearTimeout(timer.current);
    },
    [],
  );

  const begin = useCallback(() => {
    if (timer.current !== null) clearTimeout(timer.current);
    setStatus('pending');
    setMessage(null);
  }, []);

  const settle = useCallback((outcome: BatchOutcome) => {
    if (outcome.outcome === 'ok') {
      setStatus('ok');
      setMessage(null);
      if (timer.current !== null) clearTimeout(timer.current);
      timer.current = setTimeout(() => setStatus('idle'), OK_FLASH_MS);
    } else {
      setStatus('error');
      setMessage(
        outcome.detail !== undefined && outcome.detail.length > 0
          ? outcome.detail
          : outcome.outcome === 'refused'
            ? 'the replica refused this operation'
            : 'the operation failed',
      );
    }
  }, []);

  return { status, message, begin, settle };
}

interface SyncedTextInputProps {
  path: Path;
  /** Human name of the field for the action log, e.g. "<eClass>.<feature>". */
  label: string;
  remoteValue: string;
  registry: FieldRegistry;
  sendOps: SendOps;
  placeholder?: string;
  inputRef?: (element: HTMLInputElement | null) => void;
}

export function SyncedTextInput({
  path,
  label,
  remoteValue,
  registry,
  sendOps,
  placeholder,
  inputRef: exposeRef,
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
  const { status, message, begin, settle } = useFieldStatus();

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
    begin();
    try {
      const result = await sendOps(`set ${label}`, ops, { path, value: target });
      settle(result);
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
  }, [label, sendOps, key, begin, settle]);

  // Remote refresh handling.
  useEffect(() => {
    const input = inputRef.current;
    const focused = input !== null && document.activeElement === input;
    const activelyTyping = focused && Date.now() - lastEditAtRef.current < TYPING_THRESHOLD_MS;
    if (activelyTyping) return; // overlay preserved our value; never clobber
    if (remoteValue === localRef.current) {
      // Advance the baseline only for unfocused fields: their value cannot
      // hold unflushed edits (blur flushes). A focused field's remoteValue may
      // be its own overlaid local value (the poll writes actively-typing
      // values back into the doc); treating that echo as server truth would
      // advance the baseline without the ops ever being sent. The focused
      // field's flush maintains its baseline itself.
      if (inputRef.current === null || document.activeElement !== inputRef.current) {
        baselineRef.current = remoteValue;
      }
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
    <span className="me-field__wrap">
      <input
        ref={(element) => {
          inputRef.current = element;
          exposeRef?.(element);
        }}
        className="me-input"
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
          } else if (e.key === 'Escape') {
            // Revert to the last value the node confirmed. No op is needed:
            // the baseline IS the server's value.
            e.stopPropagation();
            if (debounceRef.current !== null) clearTimeout(debounceRef.current);
            lastEditAtRef.current = 0;
            setLocalValue(baselineRef.current);
            e.currentTarget.blur();
          }
        }}
      />
      <FieldStateSlot status={status} message={message} onRetry={() => void flush()} />
    </span>
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
  const { status, message, begin, settle } = useFieldStatus();

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
      begin();
      void sendOps(`set ${label} = ${parsed}`, ops, { path, value: parsed }).then(settle);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [text, remoteValue, integer, label, sendOps, pathKey(path), begin, settle]);

  return (
    <span className="me-field__wrap">
      <input
        className="me-input me-input--num"
        type="number"
        step={integer === true ? 1 : 'any'}
        title="Sent as a relative Inc: the wire has no absolute set for numbers."
        value={text}
        onChange={(e) => {
          editingRef.current = true;
          setText(e.target.value);
        }}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === 'Enter') commit();
          else if (e.key === 'Escape') {
            e.stopPropagation();
            editingRef.current = false;
            setText(String(remoteValue));
            e.currentTarget.blur();
          }
        }}
      />
      <FieldStateSlot status={status} message={message} onRetry={commit} />
    </span>
  );
}

interface BoolInputProps {
  path: Path;
  label: string;
  remoteValue: boolean;
  sendOps: SendOps;
}

export function BoolInput({ path, label, remoteValue, sendOps }: BoolInputProps) {
  const { status, message, begin, settle } = useFieldStatus();
  return (
    <span className="me-field__wrap">
      <label className="me-switch">
        <input
          type="checkbox"
          checked={remoteValue}
          onChange={(e) => {
            const next = e.target.checked;
            begin();
            void sendOps(`set ${label} = ${next}`, setBooleanOps(path, next), {
              path,
              value: next,
            }).then(settle);
          }}
        />
        <span className="me-switch__track" aria-hidden="true" />
        <span className="me-switch__value">{String(remoteValue)}</span>
      </label>
      <FieldStateSlot status={status} message={message} onRetry={() => {}} />
    </span>
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
  const { status, message, begin, settle } = useFieldStatus();
  return (
    <span className="me-field__wrap">
      <select
        className="me-select"
        value={remoteValue}
        onChange={(e) => {
          const next = e.target.value;
          begin();
          void sendOps(`set ${label} = ${next}`, setStringOps(path, remoteValue, next), {
            path,
            value: next,
          }).then(settle);
        }}
      >
        {!literals.includes(remoteValue) && (
          <option value={remoteValue}>
            {remoteValue === '' ? '(unset)' : `${remoteValue} (not a literal)`}
          </option>
        )}
        {literals.map((lit) => (
          <option key={lit} value={lit}>
            {lit}
          </option>
        ))}
      </select>
      <FieldStateSlot status={status} message={message} onRetry={() => {}} />
    </span>
  );
}
