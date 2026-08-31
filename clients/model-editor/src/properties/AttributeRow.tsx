/**
 * One attribute of the selected element: a left-aligned label cell carrying
 * the feature's name, its required marker, its id marker and a type chip, and
 * a control cell holding the right live-synced primitive for its kind.
 *
 * The kind -> control mapping is descriptor-driven; nothing here knows a class
 * name from any metamodel.
 */

import type { AttributeDesc, Descriptor, Path, PlainJson } from '../api/types';
import { pathKey } from '../crdt/path';
import { insertIntoArrayOps, removeFromArrayOps } from '../crdt/ops';
import { attributeValue, defaultForKind } from '../model/instance';
import type { FieldRegistry } from '../sync/fieldRegistry';
import { Key, Plus, Trash2 } from '../ui/icons';
import { ICON } from '../ui/iconProps';
import { BoolInput, EnumSelect, NumberInput, SyncedTextInput, type SendOps } from './fields';

/** The 11px chip after a feature name: its declared type, in wire terms. */
function typeChip(attr: AttributeDesc): string {
  const base = attr.kind === 'enum' ? `enum ${attr.enum ?? '?'}` : attr.kind;
  return attr.many ? `${base}[ ]` : base;
}

interface AttributeRowProps {
  descriptor: Descriptor;
  element: Record<string, PlainJson>;
  elementPath: Path;
  eClass: string;
  attr: AttributeDesc;
  registry: FieldRegistry;
  sendOps: SendOps;
  /** Set on the element's id/name attribute so F2 can reach it. */
  idInputRef?: (element: HTMLInputElement | null) => void;
}

export function AttributeRow({
  descriptor,
  element,
  elementPath,
  eClass,
  attr,
  registry,
  sendOps,
  idInputRef,
}: AttributeRowProps) {
  const label = `${eClass}.${attr.name}`;

  return (
    <div className="me-field">
      <span className="me-field__label">
        {attr.isId && (
          <span
            className="me-field__id"
            title="identifies this element for references"
            aria-label="identifies this element for references"
          >
            <Key {...ICON} size={12} aria-hidden="true" />
          </span>
        )}
        <span>{attr.name}</span>
        {attr.required && (
          <span className="me-req" title="required feature">
            *
          </span>
        )}
        <span className="me-typechip">{typeChip(attr)}</span>
      </span>
      <span className="me-field__control">
        {attr.many ? (
          <ManyAttributeValues
            descriptor={descriptor}
            element={element}
            elementPath={elementPath}
            label={label}
            attr={attr}
            registry={registry}
            sendOps={sendOps}
          />
        ) : (
          <AttributeInput
            descriptor={descriptor}
            path={[...elementPath, attr.name]}
            label={label}
            attr={attr}
            value={attributeValue(element, attr)}
            registry={registry}
            sendOps={sendOps}
            inputRef={idInputRef}
          />
        )}
      </span>
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
  inputRef?: (element: HTMLInputElement | null) => void;
}

function AttributeInput({
  descriptor,
  path,
  label,
  attr,
  value,
  registry,
  sendOps,
  inputRef,
}: AttributeInputProps) {
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
      return <BoolInput path={path} label={label} remoteValue={value === true} sendOps={sendOps} />;
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
          inputRef={inputRef}
        />
      );
  }
}

interface ManyAttributeValuesProps {
  descriptor: Descriptor;
  element: Record<string, PlainJson>;
  elementPath: Path;
  label: string;
  attr: AttributeDesc;
  registry: FieldRegistry;
  sendOps: SendOps;
}

function ManyAttributeValues({
  descriptor,
  element,
  elementPath,
  label,
  attr,
  registry,
  sendOps,
}: ManyAttributeValuesProps) {
  const arrayPath = [...elementPath, attr.name];
  const raw = element[attr.name];
  const values = Array.isArray(raw) ? raw : [];

  const addEntry = () =>
    void sendOps(
      `add ${label}[${values.length}]`,
      insertIntoArrayOps(arrayPath, values.length, defaultForKind(attr.kind)),
      { path: arrayPath, value: [...values, defaultForKind(attr.kind)] },
    );

  return (
    <div className="me-many">
      {values.map((entry, index) => (
        <div className="me-many__item" key={pathKey([...arrayPath, index])}>
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
            className="me-iconbtn me-iconbtn--danger"
            aria-label={`Remove ${label}[${index}]`}
            title="Remove"
            onClick={() =>
              void sendOps(`remove ${label}[${index}]`, removeFromArrayOps(arrayPath, index), {
                path: arrayPath,
                value: values.filter((_, i) => i !== index),
              })
            }
          >
            <Trash2 {...ICON} size={14} aria-hidden="true" />
          </button>
        </div>
      ))}
      <button
        type="button"
        className="me-btn me-btn--sm me-many__add"
        onKeyDown={(event) => {
          if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') addEntry();
        }}
        onClick={addEntry}
      >
        <Plus {...ICON} size={13} aria-hidden="true" />
        Add
      </button>
    </div>
  );
}
