/**
 * One reference feature as a card. References are id strings on the wire, so
 * the picker lists the document's existing instances of the target family
 * (collectInstances) by their id-attribute value.
 *
 * Clicking a resolved reference selects its target in the tree — which is why
 * references need no second tree semantics of their own.
 */

import { useState } from 'react';
import type { Descriptor, Path, PlainJson, ReferenceDesc } from '../api/types';
import { pathKey } from '../crdt/path';
import { addManyReferenceOps, clearStringOps, removeFromArrayOps, setStringOps } from '../crdt/ops';
import { collectInstances, type InstanceRef } from '../model/instance';
import { Link2, Plus, Trash2, TriangleAlert } from '../ui/icons';
import { ICON } from '../ui/iconProps';
import type { SendOps } from './fields';

interface ReferenceBlockProps {
  descriptor: Descriptor;
  doc: PlainJson;
  element: Record<string, PlainJson>;
  elementPath: Path;
  eClass: string;
  desc: ReferenceDesc;
  sendOps: SendOps;
  onSelectPath: (path: Path) => void;
}

function optionLabel(instance: InstanceRef): string {
  return `${instance.label} · ${instance.eClass} · ${instance.id}`;
}

export function ReferenceBlock({
  descriptor,
  doc,
  element,
  elementPath,
  eClass,
  desc,
  sendOps,
  onSelectPath,
}: ReferenceBlockProps) {
  const label = `${eClass}.${desc.name}`;
  // Only instances carrying an id can be referenced at all.
  const instances = collectInstances(descriptor, doc, desc.target).filter((i) => i.id !== '');
  const raw = element[desc.name];

  const header = (count: number | null) => (
    <div className="me-block__header">
      <Link2 {...ICON} size={14} aria-hidden="true" />
      <span className="me-block__name">{desc.name}</span>
      <span className="me-subtle me-mono">→ {desc.target}</span>
      {desc.required && (
        <span className="me-req" title="required feature">
          *
        </span>
      )}
      {count !== null && <span className="me-badge me-num">{count}</span>}
    </div>
  );

  if (!desc.many) {
    const path = [...elementPath, desc.name];
    const current = typeof raw === 'string' ? raw : '';
    const target = instances.find((i) => i.id === current);
    return (
      <div className="me-block me-card">
        {header(null)}
        <div className="me-block__body">
          <select
            className="me-select"
            aria-label={label}
            value={current}
            onChange={(event) => {
              const id = event.target.value;
              const ops = id === '' ? clearStringOps(path, current) : setStringOps(path, current, id);
              if (ops.length > 0) {
                void sendOps(`set ${label} → ${id === '' ? '(none)' : id}`, ops, { path, value: id });
              }
            }}
          >
            <option value="">(none)</option>
            {current !== '' && target === undefined && <option value={current}>{current}</option>}
            {instances.map((instance) => (
              <option key={pathKey(instance.path)} value={instance.id}>
                {optionLabel(instance)}
              </option>
            ))}
          </select>
          {current !== '' && target === undefined && (
            <span className="me-chip me-chip--warn">
              <TriangleAlert {...ICON} size={13} aria-hidden="true" />
              {current} — not found in this document
            </span>
          )}
          {target !== undefined && (
            <button
              type="button"
              className="me-btn me-btn--sm"
              onClick={() => onSelectPath(target.path)}
            >
              Go to target
            </button>
          )}
        </div>
      </div>
    );
  }

  const arrayPath = [...elementPath, desc.name];
  const ids = Array.isArray(raw) ? raw.map((v) => (typeof v === 'string' ? v : '')) : [];
  const addable = instances.filter((i) => !ids.includes(i.id));

  return (
    <div className="me-block me-card">
      {header(ids.length)}
      <div className="me-block__body">
        <div className="me-refchips">
          {ids.map((id, index) => {
            const target = instances.find((i) => i.id === id);
            return (
              <span
                key={`${index}-${id}`}
                className={target === undefined ? 'me-chip me-chip--warn' : 'me-chip'}
              >
                {target === undefined && <TriangleAlert {...ICON} size={12} aria-hidden="true" />}
                {target === undefined ? (
                  <span className="me-mono">{id === '' ? '(empty)' : id}</span>
                ) : (
                  <button
                    type="button"
                    className="me-chip__link me-mono"
                    onClick={() => onSelectPath(target.path)}
                  >
                    {id}
                  </button>
                )}
                <button
                  type="button"
                  className="me-iconbtn"
                  aria-label={`Remove ${label}[${index}]`}
                  title="Remove"
                  onClick={() =>
                    void sendOps(`remove ${label}[${index}]`, removeFromArrayOps(arrayPath, index), {
                      path: arrayPath,
                      value: ids.filter((_, i) => i !== index),
                    })
                  }
                >
                  <Trash2 {...ICON} size={12} aria-hidden="true" />
                </button>
              </span>
            );
          })}
        </div>
        {addable.length > 0 ? (
          <ManyReferenceAdd
            addable={addable}
            onAdd={(id) =>
              void sendOps(
                `add ${label}[${ids.length}] → ${id}`,
                addManyReferenceOps(arrayPath, ids.length, id),
                { path: arrayPath, value: [...ids, id] },
              )
            }
          />
        ) : (
          <p className="me-muted">No further {desc.target} instances with an id to reference.</p>
        )}
      </div>
    </div>
  );
}

interface ManyReferenceAddProps {
  addable: InstanceRef[];
  onAdd: (id: string) => void;
}

function ManyReferenceAdd({ addable, onAdd }: ManyReferenceAddProps) {
  const [choice, setChoice] = useState('');
  const selected = addable.some((i) => i.id === choice) ? choice : addable[0].id;
  return (
    <div className="me-block__add">
      <select
        className="me-select"
        aria-label="Reference to add"
        value={selected}
        onChange={(e) => setChoice(e.target.value)}
      >
        {addable.map((instance) => (
          <option key={pathKey(instance.path)} value={instance.id}>
            {optionLabel(instance)}
          </option>
        ))}
      </select>
      <button type="button" className="me-btn" onClick={() => onAdd(selected)}>
        <Plus {...ICON} size={14} aria-hidden="true" />
        Add
      </button>
    </div>
  );
}
