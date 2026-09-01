/**
 * One containment feature as a card: its children as selectable rows with
 * reorder/remove, and the add control at the foot.
 *
 * The op call sites are the tested contract with crdt/ops.ts and are carried
 * over unchanged — including the two honest caveats the wire forces on us:
 * a reorder is delete + full re-create (there is no move op), and "unset"
 * resets a slot to defaults rather than deleting the key.
 *
 * EVERY control in this card carries a third caveat, found by driving the real
 * app against a real replica: its ops are computed from the client's current
 * view — an index, an array length, or, for a reorder, the whole child subtree
 * that gets deleted and re-created. Issued while the queue is draining, they
 * are computed from the replica's half-applied state. Measured on the rig:
 * two reorders queued behind a pending string edit turned a populated element
 * into a bare `{}` and dropped its child subtree, with both log rows reporting
 * `ok`; a later sweep destroyed an element using add/remove alone during the
 * same window. The identical sequences with the queue drained first round-trip
 * every element. So the card is gated (ui/editGate.ts): add / remove / unset
 * wait for no structural batch to be in flight, and the two reorder buttons —
 * which copy a whole subtree out of the local document — wait for a completely
 * silent queue. The defect itself is below this layer — there is no move op on
 * the wire — and is reported rather than papered over; this only stops the UI
 * from firing the gun while the queue is moving.
 */

import type { ContainmentDesc, Descriptor, Path, PlainJson } from '../api/types';
import { pathKey } from '../crdt/path';
import {
  addChildOps,
  createSingleContainmentOps,
  removeFromArrayOps,
  reorderArrayOps,
  unsetFeatureOps,
} from '../crdt/ops';
import { concreteSubtypes, isPresent, labelFor } from '../model/instance';
import { ArrowDown, ArrowUp, Box, Folder, Trash2, TriangleAlert, Unlink } from '../ui/icons';
import { ICON } from '../ui/iconProps';
import { AddControl } from './AddControl';
import type { EditControls } from './fields';

interface ContainmentBlockProps {
  descriptor: Descriptor;
  element: Record<string, PlainJson>;
  elementPath: Path;
  eClass: string;
  desc: ContainmentDesc;
  onSelectPath: (path: Path) => void;
  /** Senders and the edit gate: every control here is structural. */
  edit: EditControls;
}

export function ContainmentBlock({
  descriptor,
  element,
  elementPath,
  eClass,
  desc,
  onSelectPath,
  edit,
}: ContainmentBlockProps) {
  const label = `${eClass}.${desc.name}`;
  const options = concreteSubtypes(descriptor, desc.target);
  const raw = element[desc.name];
  const children = desc.many && Array.isArray(raw) ? raw : [];

  const header = (count: number | null) => (
    <div className="me-block__header">
      <Folder {...ICON} size={14} aria-hidden="true" />
      <span className="me-block__name">{desc.name}</span>
      <span className="me-subtle me-mono">: {desc.target}</span>
      {desc.required && (
        <span className="me-req" title="required feature">
          *
        </span>
      )}
      {count !== null && <span className="me-badge me-num">{count}</span>}
    </div>
  );

  if (!desc.many) {
    const childPath = [...elementPath, desc.name];
    const child = raw;
    return (
      <div className="me-block me-card">
        {header(null)}
        <div className="me-block__body">
          {isPresent(child) ? (
            <div className="me-block__row">
              <button
                type="button"
                className="me-block__link"
                onClick={() => onSelectPath(childPath)}
              >
                <Box {...ICON} size={14} aria-hidden="true" />
                <span>{labelFor(descriptor, child)}</span>
                <span className="me-badge">{child['eClass'] as string}</span>
              </button>
              <button
                type="button"
                className={edit.structureEnabled ? 'me-iconbtn' : 'me-iconbtn me-held'}
                aria-label={`Unset ${desc.name}`}
                disabled={!edit.structureEnabled}
                title={
                  edit.structureReason ??
                  'Unset — resets the slot to defaults; the key remains in the serialized state'
                }
                onClick={() => {
                  onSelectPath(elementPath);
                  void edit.runStructural(`unset ${label}`, unsetFeatureOps(elementPath, desc.name), {
                    path: childPath,
                    value: { eClass: '' },
                  });
                }}
              >
                <Unlink {...ICON} size={14} aria-hidden="true" />
              </button>
            </div>
          ) : (
            /* Placeholder and action on one line: an empty feature does not
               deserve three stacked rows of furniture. */
            <div className="me-block__empty">
              <p className="me-block__placeholder">No {desc.target} yet</p>
              <AddControl
                options={options}
                verb="Create"
                disabled={!edit.structureEnabled}
                disabledReason={edit.structureReason}
                onAdd={(cls) =>
                  void edit.runStructural(
                    `create ${cls} in ${label}`,
                    createSingleContainmentOps(elementPath, desc.name, cls),
                    { path: childPath, value: { eClass: cls } },
                  )
                }
              />
            </div>
          )}
        </div>
      </div>
    );
  }

  const arrayPath = [...elementPath, desc.name];
  return (
    <div className="me-block me-card">
      {header(children.length)}
      <div className="me-block__body">
        {children.map((child, index) => {
          const childPath = [...arrayPath, index];
          const present = isPresent(child);
          return (
            <div className="me-block__row" key={pathKey(childPath)}>
              <button
                type="button"
                className="me-block__link"
                disabled={!present}
                onClick={() => onSelectPath(childPath)}
              >
                {present ? (
                  <Box {...ICON} size={14} aria-hidden="true" />
                ) : (
                  <TriangleAlert {...ICON} size={14} className="me-tree__icon--warn" aria-hidden="true" />
                )}
                <span>{present ? labelFor(descriptor, child) : '(invalid element)'}</span>
                {present && <span className="me-badge">{child['eClass'] as string}</span>}
              </button>
              <button
                type="button"
                className={edit.reorderEnabled ? 'me-iconbtn' : 'me-iconbtn me-held'}
                aria-label={`Move ${label}[${index}] up`}
                title={
                  edit.reorderReason ??
                  'Move up — sent as delete + re-create: the wire has no move op'
                }
                disabled={index === 0 || !edit.reorderEnabled}
                onClick={() => {
                  onSelectPath(elementPath);
                  void edit.runStructural(
                    `move ${label}[${index}] up`,
                    reorderArrayOps(arrayPath, index, index - 1, child),
                  );
                }}
              >
                <ArrowUp {...ICON} size={14} aria-hidden="true" />
              </button>
              <button
                type="button"
                className={edit.reorderEnabled ? 'me-iconbtn' : 'me-iconbtn me-held'}
                aria-label={`Move ${label}[${index}] down`}
                title={
                  edit.reorderReason ??
                  'Move down — sent as delete + re-create: the wire has no move op'
                }
                disabled={index === children.length - 1 || !edit.reorderEnabled}
                onClick={() => {
                  onSelectPath(elementPath);
                  void edit.runStructural(
                    `move ${label}[${index}] down`,
                    reorderArrayOps(arrayPath, index, index + 1, child),
                  );
                }}
              >
                <ArrowDown {...ICON} size={14} aria-hidden="true" />
              </button>
              <button
                type="button"
                className={
                  edit.structureEnabled
                    ? 'me-iconbtn me-iconbtn--danger'
                    : 'me-iconbtn me-iconbtn--danger me-held'
                }
                aria-label={`Remove ${label}[${index}]`}
                disabled={!edit.structureEnabled}
                title={edit.structureReason ?? 'Remove'}
                onClick={() => {
                  onSelectPath(elementPath);
                  void edit.runStructural(`remove ${label}[${index}]`, removeFromArrayOps(arrayPath, index), {
                    path: arrayPath,
                    value: children.filter((_, i) => i !== index),
                  });
                }}
              >
                <Trash2 {...ICON} size={14} aria-hidden="true" />
              </button>
            </div>
          );
        })}
        <div className="me-block__empty">
          {children.length === 0 && <p className="me-block__placeholder">No {desc.target} yet</p>}
          <AddControl
            options={options}
            verb="Add"
            disabled={!edit.structureEnabled}
            disabledReason={edit.structureReason}
            onAdd={(cls) =>
              void edit.runStructural(
                `add ${cls} to ${label}[${children.length}]`,
                addChildOps(arrayPath, children.length, cls),
                { path: arrayPath, value: [...children, { eClass: cls }] },
              )
            }
          />
        </div>
      </div>
    </div>
  );
}
