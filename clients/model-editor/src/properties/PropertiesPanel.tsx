/**
 * The right panel: the selected element's identity, its path as a live
 * breadcrumb, and its features in the fixed order Attributes -> Containments
 * -> References (which is flattenFeatures' own output order, supertypes first).
 *
 * Sections with no features are not rendered; a class the descriptor does not
 * declare gets a warning strip rather than a blank body.
 */

import { useState, type RefObject } from 'react';
import type { Descriptor, Path, PlainJson } from '../api/types';
import { EmptyState } from '../common/EmptyState';
import { getAtPath } from '../crdt/path';
import { removeFromArrayOps } from '../crdt/ops';
import { flattenFeatures, idAttributeOf, isPresent, labelFor } from '../model/instance';
import type { FieldRegistry } from '../sync/fieldRegistry';
import { Box, Copy, MousePointerClick, Plug, Trash2, TriangleAlert } from '../ui/icons';
import { ICON } from '../ui/iconProps';
import { AttributeRow } from './AttributeRow';
import { ContainmentBlock } from './ContainmentBlock';
import { ReferenceBlock } from './ReferenceBlock';
import type { SendOps } from './fields';

interface PropertiesPanelProps {
  descriptor: Descriptor | null;
  doc: PlainJson;
  connected: boolean;
  path: Path;
  registry: FieldRegistry;
  sendOps: SendOps;
  onSelectPath: (path: Path) => void;
  formRef: RefObject<HTMLDivElement | null>;
  idInputRef: RefObject<HTMLInputElement | null>;
  /** syncView().detail while the replica has stopped answering. */
  staleNotice: string | null;
  /** Operations are in flight: reorder is held back until they land. */
  busy: boolean;
  onOpenConnect: () => void;
}

interface SectionProps {
  title: string;
  count: number;
  children: React.ReactNode;
}

function Section({ title, count, children }: SectionProps) {
  const [open, setOpen] = useState(true);
  return (
    <section className="me-section">
      <button
        type="button"
        className="me-section__header"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
      >
        <span className="me-section__title">{title}</span>
        <span className="me-badge me-num">{count}</span>
        <span className="me-section__rule" aria-hidden="true" />
      </button>
      {open && <div className="me-section__body">{children}</div>}
    </section>
  );
}

/** The path as clickable ancestry; only segments that resolve to a present element select. */
function Breadcrumb({
  doc,
  path,
  onSelectPath,
}: {
  doc: PlainJson;
  path: Path;
  onSelectPath: (path: Path) => void;
}) {
  return (
    <nav className="me-props__crumbs me-mono" aria-label="Element path">
      <button type="button" className="me-props__crumb" onClick={() => onSelectPath([])}>
        /
      </button>
      {path.map((segment, index) => {
        const prefix = path.slice(0, index + 1);
        const selectable = isPresent(getAtPath(doc, prefix));
        return (
          <span key={index} className="me-props__crumb-wrap">
            {selectable ? (
              <button
                type="button"
                className="me-props__crumb"
                onClick={() => onSelectPath(prefix)}
              >
                {String(segment)}
              </button>
            ) : (
              <span className="me-props__crumb me-props__crumb--flat">{String(segment)}</span>
            )}
            {index < path.length - 1 && <span aria-hidden="true">/</span>}
          </span>
        );
      })}
    </nav>
  );
}

export function PropertiesPanel({
  descriptor,
  doc,
  connected,
  path,
  registry,
  sendOps,
  onSelectPath,
  formRef,
  idInputRef,
  staleNotice,
  busy,
  onOpenConnect,
}: PropertiesPanelProps) {
  const [copied, setCopied] = useState(false);

  if (!connected) {
    return (
      <section className="me-panel me-props" aria-label="Properties">
        <EmptyState
          icon={Plug}
          title="Not connected"
          body="Point the editor at a replica's HTTP API to load its model and metamodel."
        >
          <button type="button" className="me-btn me-btn--primary" onClick={onOpenConnect}>
            Connect…
          </button>
        </EmptyState>
      </section>
    );
  }

  const element = descriptor === null ? undefined : getAtPath(doc, path);
  if (descriptor === null || !isPresent(element)) {
    return (
      <section className="me-panel me-props" aria-label="Properties">
        <EmptyState
          icon={MousePointerClick}
          title="Select an element"
          body="Pick an element in the model tree to edit its attributes, containments and references. Arrow keys navigate; Enter jumps into the form."
        />
      </section>
    );
  }

  const eClass = element['eClass'] as string;
  const known = descriptor.classes[eClass] !== undefined;
  const flat = flattenFeatures(descriptor, eClass);
  const idAttr = idAttributeOf(descriptor, eClass);
  const arrayIndex = path[path.length - 1];
  const removable = typeof arrayIndex === 'number';

  return (
    <section className="me-panel me-props" aria-label="Properties">
      <header className="me-props__header">
        <Box {...ICON} className="me-props__icon" aria-hidden="true" />
        <h2 className="me-props__title">{labelFor(descriptor, element)}</h2>
        <span className="me-badge">{eClass}</span>
        <Breadcrumb doc={doc} path={path} onSelectPath={onSelectPath} />
        <span className="me-props__header-spacer" />
        <button
          type="button"
          className="me-btn me-btn--sm me-noprint"
          onClick={() => {
            void navigator.clipboard?.writeText(`/${path.join('/')}`).then(
              () => {
                setCopied(true);
                setTimeout(() => setCopied(false), 1200);
              },
              () => setCopied(false),
            );
          }}
        >
          <Copy {...ICON} size={13} aria-hidden="true" />
          {copied ? 'Copied' : 'Copy path'}
        </button>
        {removable && (
          <button
            type="button"
            className="me-btn me-btn--sm me-noprint"
            onClick={() => {
              const arrayPath = path.slice(0, -1);
              const siblings = getAtPath(doc, arrayPath);
              onSelectPath(arrayPath.slice(0, -1));
              void sendOps(
                `remove element at /${path.join('/')}`,
                removeFromArrayOps(arrayPath, arrayIndex),
                Array.isArray(siblings)
                  ? { path: arrayPath, value: siblings.filter((_, i) => i !== arrayIndex) }
                  : undefined,
              );
            }}
          >
            <Trash2 {...ICON} size={13} aria-hidden="true" />
            Delete element
          </button>
        )}
      </header>

      {staleNotice !== null && (
        <p className="me-props__stale">
          <TriangleAlert {...ICON} size={14} aria-hidden="true" />
          {staleNotice}
        </p>
      )}

      <div className="me-props__body" ref={formRef}>
        {!known && (
          <p className="me-props__unknown">
            <TriangleAlert {...ICON} size={14} aria-hidden="true" />
            Class <code>{eClass}</code> is not in the discovered metamodel; no features to edit.
            Load the matching descriptor to type this element.
          </p>
        )}

        {flat.attributes.length > 0 && (
          <Section title="Attributes" count={flat.attributes.length}>
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
                idInputRef={
                  idAttr !== null && attr.name === idAttr.name && !attr.many
                    ? (input) => {
                        idInputRef.current = input;
                      }
                    : undefined
                }
              />
            ))}
          </Section>
        )}

        {flat.containments.length > 0 && (
          <Section title="Containments" count={flat.containments.length}>
            {flat.containments.map((desc) => (
              <ContainmentBlock
                key={desc.name}
                descriptor={descriptor}
                element={element}
                elementPath={path}
                eClass={eClass}
                desc={desc}
                sendOps={sendOps}
                onSelectPath={onSelectPath}
                busy={busy}
              />
            ))}
          </Section>
        )}

        {flat.references.length > 0 && (
          <Section title="References" count={flat.references.length}>
            {flat.references.map((desc) => (
              <ReferenceBlock
                key={desc.name}
                descriptor={descriptor}
                doc={doc}
                element={element}
                elementPath={path}
                eClass={eClass}
                desc={desc}
                sendOps={sendOps}
                onSelectPath={onSelectPath}
              />
            ))}
          </Section>
        )}
      </div>
    </section>
  );
}
