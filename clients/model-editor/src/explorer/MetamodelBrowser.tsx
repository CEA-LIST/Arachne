/**
 * The discovered metamodel, browsable.
 *
 * When the node serves no descriptor (GET /api/metamodel -> 404) this tab
 * hosts the clearly-labelled degraded mode: load a descriptor file produced by
 * `arachne describe <file.ecore>`. Rejection shows the validator's own message.
 */

import { useMemo, useState } from 'react';
import { validateDescriptor } from '../api/client';
import type { Descriptor } from '../api/types';
import { EmptyState } from '../common/EmptyState';
import type { ClassDesc } from '../api/types';
import { FileWarning, Plug, Search } from '../ui/icons';
import { ICON } from '../ui/iconProps';

/**
 * Feature counts, zeros omitted.
 *
 * Most classes in a real metamodel declare features in one or two of the three
 * kinds, so `0 attr · 0 cont · 0 ref` spent a quarter of the panel's width
 * saying nothing — and it was the CLASS NAME that got the ellipsis for it. The
 * absence of a part is the zero; the full sentence stays in the row's title.
 */
function countParts(cls: ClassDesc): string {
  const parts: string[] = [];
  if (cls.attributes.length > 0) parts.push(`${cls.attributes.length} attr`);
  if (cls.containments.length > 0) parts.push(`${cls.containments.length} cont`);
  if (cls.references.length > 0) parts.push(`${cls.references.length} ref`);
  return parts.join(' · ');
}

interface MetamodelBrowserProps {
  metamodel: Descriptor | null;
  source: 'node' | 'file' | null;
  connected: boolean;
  loadDescriptorFile: (descriptor: Descriptor) => void;
}

export function MetamodelBrowser({
  metamodel,
  source,
  connected,
  loadDescriptorFile,
}: MetamodelBrowserProps) {
  const [fileError, setFileError] = useState<string | null>(null);
  const [query, setQuery] = useState('');

  const onFile = (file: File | undefined) => {
    if (file === undefined) return;
    setFileError(null);
    file
      .text()
      .then((text) => loadDescriptorFile(validateDescriptor(JSON.parse(text))))
      .catch((err) => setFileError(err instanceof Error ? err.message : String(err)));
  };

  const classes = useMemo(() => {
    if (metamodel === null) return [];
    const needle = query.trim().toLowerCase();
    return Object.entries(metamodel.classes).filter(
      ([name]) => needle.length === 0 || name.toLowerCase().includes(needle),
    );
  }, [metamodel, query]);

  if (metamodel === null) {
    if (!connected) {
      return (
        <EmptyState
          icon={Plug}
          title="Not connected"
          body="Connect to a replica to discover the metamodel it serves."
        />
      );
    }
    return (
      <EmptyState
        icon={FileWarning}
        title="This replica serves no metamodel"
        body={
          <>
            <code>GET /api/metamodel</code> returned 404. Load a descriptor file to edit with types,
            or use the Document JSON tab in the console to inspect the raw state.
          </>
        }
        tone="warn"
      >
        <label className="me-btn">
          Load descriptor…
          <input
            type="file"
            className="me-sr-only"
            accept=".json,application/json"
            onChange={(e) => onFile(e.target.files?.[0])}
          />
        </label>
        <p className="me-well">arachne describe &lt;file.ecore&gt;</p>
        {fileError !== null && <p className="me-form__error">descriptor rejected: {fileError}</p>}
      </EmptyState>
    );
  }

  return (
    <div className="me-meta">
      <dl className="me-meta__summary">
        <dt>package</dt>
        <dd className="me-mono">{metamodel.package}</dd>
        <dt>nsURI</dt>
        <dd className="me-mono me-truncate" title={metamodel.nsURI}>
          {metamodel.nsURI}
        </dd>
        <dt>root classes</dt>
        <dd className="me-mono">
          {metamodel.rootClasses.length > 0 ? metamodel.rootClasses.join(', ') : '—'}
        </dd>
        <dt>source</dt>
        <dd>{source === 'node' ? 'served by the node' : 'loaded from file'}</dd>
      </dl>

      <div className="me-panel__toolbar">
        <span className="me-panel__search">
          <Search {...ICON} size={14} className="me-panel__search-icon" aria-hidden="true" />
          <input
            className="me-input me-panel__search-input"
            type="search"
            value={query}
            placeholder="Filter classes…"
            aria-label="Filter classes"
            onChange={(e) => setQuery(e.target.value)}
          />
        </span>
        <span className="me-subtle me-num">
          {classes.length} class{classes.length === 1 ? '' : 'es'}
        </span>
      </div>

      <ul className="me-meta__list">
        {classes.map(([name, cls]) => (
          <li key={name} className="me-meta__class">
            <span className="me-meta__name me-mono" title={name}>
              {name}
            </span>
            {cls.abstract && <span className="me-badge">abstract</span>}
            {cls.superTypes.length > 0 && (
              <span className="me-meta__super me-subtle me-mono" title={cls.superTypes.join(', ')}>
                : {cls.superTypes.join(', ')}
              </span>
            )}
            <span
              className="me-meta__counts me-subtle me-num"
              title={`${cls.attributes.length} attributes · ${cls.containments.length} containments · ${cls.references.length} references`}
            >
              {countParts(cls)}
            </span>
          </li>
        ))}
        {classes.length === 0 && (
          <li className="me-meta__none">No class matches “{query}”.</li>
        )}
      </ul>

      {Object.keys(metamodel.enums).length > 0 && (
        <>
          <h3 className="me-section__title">Enums</h3>
          <ul className="me-meta__list">
            {Object.entries(metamodel.enums).map(([name, literals]) => (
              <li key={name} className="me-meta__class">
                <span className="me-meta__name me-mono">{name}</span>
                <span className="me-subtle me-mono">{literals.join(' | ')}</span>
              </li>
            ))}
          </ul>
        </>
      )}
    </div>
  );
}
