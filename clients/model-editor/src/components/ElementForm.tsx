/**
 * The typed form for the selected element: attributes as typed inputs (the
 * live-synced primitives from fields.tsx), containments as child lists with
 * add (concrete-subtype menu) / remove / move, references as pickers over the
 * document's existing instances of the target family.
 *
 * Every edit intent maps through src/crdt/ops.ts and is enqueued via sendOps,
 * so each one lands in the action log with its exact op payloads and verdict.
 */

import { useState } from 'react';
import type { AttributeDesc, ContainmentDesc, Descriptor, Path, PlainJson, ReferenceDesc } from '../api/types';
import { getAtPath, pathKey } from '../crdt/path';
import {
  addChildOps,
  addManyReferenceOps,
  clearStringOps,
  createSingleContainmentOps,
  insertIntoArrayOps,
  removeFromArrayOps,
  reorderArrayOps,
  setStringOps,
  unsetFeatureOps,
} from '../crdt/ops';
import {
  attributeValue,
  collectInstances,
  concreteSubtypes,
  defaultForKind,
  flattenFeatures,
  isPresent,
  labelFor,
  type InstanceRef,
} from '../model/instance';
import type { FieldRegistry } from '../sync/fieldRegistry';
import { BoolInput, EnumSelect, NumberInput, SyncedTextInput, type SendOps } from './fields';

interface ElementFormProps {
  descriptor: Descriptor;
  doc: PlainJson;
  path: Path;
  registry: FieldRegistry;
  sendOps: SendOps;
  onSelectPath: (path: Path) => void;
}

export function ElementForm({ descriptor, doc, path, registry, sendOps, onSelectPath }: ElementFormProps) {
  const element = getAtPath(doc, path);
  if (!isPresent(element)) {
    return <p className="muted">Select an element in the tree.</p>;
  }
  const eClass = element['eClass'] as string;
  const known = descriptor.classes[eClass] !== undefined;
  const flat = flattenFeatures(descriptor, eClass);

  return (
    <div className="element-form">
      <div className="element-header">
        <h2>{labelFor(descriptor, element)}</h2>
        <span className="tree-eclass">{eClass}</span>
        <span className="muted element-path">/{path.join('/')}</span>
      </div>
      {!known && (
        <p className="connect-error">
          Class <code>{eClass}</code> is not in the discovered metamodel; no features to edit.
        </p>
      )}
      {flat.attributes.length > 0 && (
        <section>
          <h3>Attributes</h3>
          {flat.attributes.map((attr) => (
            <AttributeRow
              key={attr.name}
              descriptor={descriptor}
              element={element}
              elementPath={path}
              eClass={eClass}
              attr={attr}
              registry={registry}
              sendOps={sendOps}
            />
          ))}
        </section>
      )}
      {flat.containments.length > 0 && (
        <section>
          <h3>Containments</h3>
          {flat.containments.map((desc) => (
            <ContainmentSection
              key={desc.name}
              descriptor={descriptor}
              element={element}
              elementPath={path}
              eClass={eClass}
              desc={desc}
              sendOps={sendOps}
              onSelectPath={onSelectPath}
            />
          ))}
        </section>
      )}
      {flat.references.length > 0 && (
        <section>
          <h3>References</h3>
          {flat.references.map((desc) => (
            <ReferenceSection
              key={desc.name}
              descriptor={descriptor}
              doc={doc}
              element={element}
              elementPath={path}
              eClass={eClass}
              desc={desc}
              registry={registry}
              sendOps={sendOps}
            />
          ))}
        </section>
      )}
    </div>
  );
}

/* ---------- attributes ---------- */

interface AttributeRowProps {
  descriptor: Descriptor;
  element: Record<string, PlainJson>;
  elementPath: Path;
  eClass: string;
  attr: AttributeDesc;
  registry: FieldRegistry;
  sendOps: SendOps;
}

function AttributeRow({ descriptor, element, elementPath, eClass, attr, registry, sendOps }: AttributeRowProps) {
  const label = `${eClass}.${attr.name}`;
  if (attr.many) {
    return (
      <ManyAttributeRow
        descriptor={descriptor}
        element={element}
        elementPath={elementPath}
        label={label}
        attr={attr}
        registry={registry}
        sendOps={sendOps}
      />
    );
  }
  const path = [...elementPath, attr.name];
  const value = attributeValue(element, attr);
  return (
    <div className="form-row">
      <label>
        {attr.name}
        {attr.required && <span className="required">*</span>}
      </label>
      <AttributeInput
        descriptor={descriptor}
        path={path}
        label={label}
        attr={attr}
        value={value}
        registry={registry}
        sendOps={sendOps}
      />
    </div>
  );
}

interface AttributeInputProps {
  descriptor: Descriptor;
  path: Path;
  label: string;
  attr: AttributeDesc;
  value: string | number | boolean;
  registry: FieldRegistry;
  sendOps: SendOps;
}

function AttributeInput({ descriptor, path, label, attr, value, registry, sendOps }: AttributeInputProps) {
  switch (attr.kind) {
    case 'int':
    case 'float':
      return (
        <NumberInput
          path={path}
          label={label}
          remoteValue={typeof value === 'number' ? value : 0}
          sendOps={sendOps}
          integer={attr.kind === 'int'}
        />
      );
    case 'bool':
      return (
        <BoolInput
          path={path}
          label={label}
          remoteValue={value === true}
          sendOps={sendOps}
        />
      );
    case 'enum':
      return (
        <EnumSelect
          path={path}
          label={label}
          remoteValue={typeof value === 'string' ? value : ''}
          literals={descriptor.enums[attr.enum ?? ''] ?? []}
          sendOps={sendOps}
        />
      );
    default:
      return (
        <SyncedTextInput
          path={path}
          label={label}
          remoteValue={typeof value === 'string' ? value : ''}
          registry={registry}
          sendOps={sendOps}
        />
      );
  }
}

interface ManyAttributeRowProps {
  descriptor: Descriptor;
  element: Record<string, PlainJson>;
  elementPath: Path;
  label: string;
  attr: AttributeDesc;
  registry: FieldRegistry;
  sendOps: SendOps;
}

function ManyAttributeRow({ descriptor, element, elementPath, label, attr, registry, sendOps }: ManyAttributeRowProps) {
  const arrayPath = [...elementPath, attr.name];
  const raw = element[attr.name];
  const values = Array.isArray(raw) ? raw : [];
  return (
    <div className="form-row many">
      <label>{attr.name}</label>
      <div className="many-list">
        {values.map((entry, index) => (
          <div className="many-item" key={pathKey([...arrayPath, index])}>
            <AttributeInput
              descriptor={descriptor}
              path={[...arrayPath, index]}
              label={`${label}[${index}]`}
              attr={attr}
              value={
                typeof entry === 'string' || typeof entry === 'number' || typeof entry === 'boolean'
                  ? entry
                  : defaultForKind(attr.kind)
              }
              registry={registry}
              sendOps={sendOps}
            />
            <button
              type="button"
              title="Remove"
              onClick={() =>
                void sendOps(`remove ${label}[${index}]`, removeFromArrayOps(arrayPath, index), {
                  path: arrayPath,
                  value: values.filter((_, i) => i !== index),
                })
              }
            >
              ✕
            </button>
          </div>
        ))}
        <button
          type="button"
          onClick={() =>
            void sendOps(
              `add ${label}[${values.length}]`,
              insertIntoArrayOps(arrayPath, values.length, defaultForKind(attr.kind)),
              { path: arrayPath, value: [...values, defaultForKind(attr.kind)] },
            )
          }
        >
          + Add
        </button>
      </div>
    </div>
  );
}

/* ---------- containments ---------- */

interface ContainmentSectionProps {
  descriptor: Descriptor;
  element: Record<string, PlainJson>;
  elementPath: Path;
  eClass: string;
  desc: ContainmentDesc;
  sendOps: SendOps;
  onSelectPath: (path: Path) => void;
}

function ContainmentSection({ descriptor, element, elementPath, eClass, desc, sendOps, onSelectPath }: ContainmentSectionProps) {
  const label = `${eClass}.${desc.name}`;
  const options = concreteSubtypes(descriptor, desc.target);

  if (!desc.many) {
    const childPath = [...elementPath, desc.name];
    const child = element[desc.name];
    return (
      <div className="feature-block">
        <h4>
          {desc.name} <span className="muted">: {desc.target}</span>
          {desc.required && <span className="required">*</span>}
        </h4>
        {isPresent(child) ? (
          <div className="child-row">
            <button type="button" className="child-link" onClick={() => onSelectPath(childPath)}>
              {labelFor(descriptor, child)} <span className="tree-eclass">{child['eClass'] as string}</span>
            </button>
            <button
              type="button"
              title="Unset (resets the slot; the key remains with default values)"
              onClick={() => {
                onSelectPath(elementPath);
                void sendOps(`unset ${label}`, unsetFeatureOps(elementPath, desc.name), {
                  path: childPath,
                  value: { eClass: '' },
                });
              }}
            >
              Unset
            </button>
          </div>
        ) : (
          <AddControl
            options={options}
            verb="Create"
            onAdd={(cls) =>
              void sendOps(
                `create ${cls} in ${label}`,
                createSingleContainmentOps(elementPath, desc.name, cls),
                { path: childPath, value: { eClass: cls } },
              )
            }
          />
        )}
      </div>
    );
  }

  const arrayPath = [...elementPath, desc.name];
  const raw = element[desc.name];
  const children = Array.isArray(raw) ? raw : [];
  return (
    <div className="feature-block">
      <h4>
        {desc.name} <span className="muted">: {desc.target} [{children.length}]</span>
      </h4>
      {children.map((child, index) => {
        const childPath = [...arrayPath, index];
        const present = isPresent(child);
        return (
          <div className="child-row" key={pathKey(childPath)}>
            <button
              type="button"
              className="child-link"
              disabled={!present}
              onClick={() => onSelectPath(childPath)}
            >
              {present ? labelFor(descriptor, child) : '(invalid element)'}
              {present && <span className="tree-eclass">{child['eClass'] as string}</span>}
            </button>
            <button
              type="button"
              title="Move up (delete + re-create: no move op on the wire)"
              disabled={index === 0}
              onClick={() => {
                onSelectPath(elementPath);
                void sendOps(
                  `move ${label}[${index}] up`,
                  reorderArrayOps(arrayPath, index, index - 1, child),
                );
              }}
            >
              ↑
            </button>
            <button
              type="button"
              title="Move down (delete + re-create: no move op on the wire)"
              disabled={index === children.length - 1}
              onClick={() => {
                onSelectPath(elementPath);
                void sendOps(
                  `move ${label}[${index}] down`,
                  reorderArrayOps(arrayPath, index, index + 1, child),
                );
              }}
            >
              ↓
            </button>
            <button
              type="button"
              title="Remove"
              onClick={() => {
                onSelectPath(elementPath);
                void sendOps(`remove ${label}[${index}]`, removeFromArrayOps(arrayPath, index), {
                  path: arrayPath,
                  value: children.filter((_, i) => i !== index),
                });
              }}
            >
              ✕
            </button>
          </div>
        );
      })}
      <AddControl
        options={options}
        verb="Add"
        onAdd={(cls) =>
          void sendOps(
            `add ${cls} to ${label}[${children.length}]`,
            addChildOps(arrayPath, children.length, cls),
            { path: arrayPath, value: [...children, { eClass: cls }] },
          )
        }
      />
    </div>
  );
}

interface AddControlProps {
  options: string[];
  verb: string;
  onAdd: (cls: string) => void;
}

/** Concrete-subtype menu + button; collapses to one button for a single option. */
export function AddControl({ options, verb, onAdd }: AddControlProps) {
  const [choice, setChoice] = useState(options[0] ?? '');
  if (options.length === 0) {
    return <p className="muted">No concrete class can be instantiated here.</p>;
  }
  if (options.length === 1) {
    return (
      <button type="button" onClick={() => onAdd(options[0])}>
        + {verb} {options[0]}
      </button>
    );
  }
  const selected = options.includes(choice) ? choice : options[0];
  return (
    <div className="add-control">
      <select value={selected} onChange={(e) => setChoice(e.target.value)}>
        {options.map((cls) => (
          <option key={cls} value={cls}>
            {cls}
          </option>
        ))}
      </select>
      <button type="button" onClick={() => onAdd(selected)}>
        + {verb}
      </button>
    </div>
  );
}

/* ---------- references ---------- */

interface ReferenceSectionProps {
  descriptor: Descriptor;
  doc: PlainJson;
  element: Record<string, PlainJson>;
  elementPath: Path;
  eClass: string;
  desc: ReferenceDesc;
  registry: FieldRegistry;
  sendOps: SendOps;
}

function ReferenceSection({ descriptor, doc, element, elementPath, eClass, desc, sendOps }: ReferenceSectionProps) {
  const label = `${eClass}.${desc.name}`;
  const instances = collectInstances(descriptor, doc, desc.target).filter((i) => i.id !== '');

  if (!desc.many) {
    const path = [...elementPath, desc.name];
    const raw = element[desc.name];
    const current = typeof raw === 'string' ? raw : '';
    return (
      <div className="form-row">
        <label>
          {desc.name} <span className="muted">→ {desc.target}</span>
        </label>
        <ReferencePicker
          instances={instances}
          current={current}
          onPick={(id) => {
            const ops = id === '' ? clearStringOps(path, current) : setStringOps(path, current, id);
            if (ops.length > 0) {
              void sendOps(`set ${label} → ${id === '' ? '(none)' : id}`, ops, { path, value: id });
            }
          }}
        />
      </div>
    );
  }

  const arrayPath = [...elementPath, desc.name];
  const raw = element[desc.name];
  const ids = Array.isArray(raw) ? raw.map((v) => (typeof v === 'string' ? v : '')) : [];
  const addable = instances.filter((i) => !ids.includes(i.id));
  return (
    <div className="form-row many">
      <label>
        {desc.name} <span className="muted">→ {desc.target} [{ids.length}]</span>
      </label>
      <div className="many-list">
        {ids.map((id, index) => (
          <div className="many-item" key={`${index}-${id}`}>
            <span>{id === '' ? '(empty)' : id}</span>
            <button
              type="button"
              title="Remove"
              onClick={() =>
                void sendOps(`remove ${label}[${index}]`, removeFromArrayOps(arrayPath, index), {
                  path: arrayPath,
                  value: ids.filter((_, i) => i !== index),
                })
              }
            >
              ✕
            </button>
          </div>
        ))}
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
          <p className="muted">No further {desc.target} instances with an id to reference.</p>
        )}
      </div>
    </div>
  );
}

interface ReferencePickerProps {
  instances: InstanceRef[];
  current: string;
  onPick: (id: string) => void;
}

function ReferencePicker({ instances, current, onPick }: ReferencePickerProps) {
  const known = instances.some((i) => i.id === current);
  return (
    <select value={current} onChange={(e) => onPick(e.target.value)}>
      <option value="">(none)</option>
      {current !== '' && !known && <option value={current}>{current} (missing)</option>}
      {instances.map((i) => (
        <option key={pathKey(i.path)} value={i.id}>
          {i.id} ({i.eClass})
        </option>
      ))}
    </select>
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
    <div className="add-control">
      <select value={selected} onChange={(e) => setChoice(e.target.value)}>
        {addable.map((i) => (
          <option key={pathKey(i.path)} value={i.id}>
            {i.id} ({i.eClass})
          </option>
        ))}
      </select>
      <button type="button" onClick={() => onAdd(selected)}>
        + Add
      </button>
    </div>
  );
}
