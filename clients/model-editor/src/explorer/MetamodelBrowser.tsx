/**
 * The discovered metamodel, browsable — and the two things a person can do
 * with a descriptor file, which are not the same thing.
 *
 * **Load into this editor** parses the file, validates it, and hands it to the
 * open model's session for display and typing. Nothing leaves the browser.
 * That is the labelled degraded mode for a node that serves no descriptor
 * (GET /api/metamodel -> 404), and it is what this tab did and only did.
 *
 * **Add to this replica** posts the file's text to `POST /api/metamodels` on
 * the node this window is connected to, which serves it from the moment it
 * answers: a model registers under it on the very next request, with nothing
 * restarted and nothing regenerated. The reach is exactly one replica, which
 * the success line says out loud — see `servedHereLine` in
 * `addMetamodel.ts`, which is where both actions' behaviour lives.
 */

import { useEffect, useMemo, useState } from 'react';
import { validateDescriptor } from '../api/client';
import type { Descriptor } from '../api/types';
import { EmptyState } from '../common/EmptyState';
import type { ClassDesc } from '../api/types';
import type { MetamodelAddOutcome } from '../sync/useSync';
import { addDescriptorFile, type AddState } from './addMetamodel';
import { metamodelDigest } from '../model/digest';
import { FileWarning, Plug, Search, Upload } from '../ui/icons';
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

/**
 * The digest of the descriptor in hand, once it has been computed.
 *
 * Computed here rather than looked up in the replica's listing, and the
 * difference is not a convenience. A digest is taken over the descriptor's
 * own bytes, and two descriptors can share an `nsURI` and a package name and
 * still be two metamodels — that is what `ip25` is about — so matching this
 * descriptor to a listing entry by name would sometimes show the digest of a
 * language this is not. It also answers for a descriptor loaded from a file,
 * which is in no listing at all and whose digest is exactly what a person
 * needs in order to ask another replica for models written in it.
 *
 * `null` while it is being computed and if it cannot be: `crypto.subtle` is
 * exposed in secure contexts only, which https, localhost and 127.0.0.1 are
 * and a bare LAN address is not. A row that says the digest is unavailable
 * is honest; a row showing a wrong one would not be.
 */
function useDescriptorDigest(descriptor: Descriptor | null): string | null {
  // The descriptor it was computed for is kept beside it, so a stale digest
  // is filtered out during render rather than cleared by a second one.
  const [computed, setComputed] = useState<{ of: Descriptor; digest: string } | null>(null);
  useEffect(() => {
    if (descriptor === null) return;
    let current = true;
    metamodelDigest(descriptor)
      .then((digest) => {
        if (current) setComputed({ of: descriptor, digest });
      })
      .catch(() => {
        // No digest is the honest answer where `crypto.subtle` is absent.
      });
    return () => {
      current = false;
    };
  }, [descriptor]);
  return computed !== null && computed.of === descriptor ? computed.digest : null;
}

interface MetamodelBrowserProps {
  metamodel: Descriptor | null;
  source: 'node' | 'file' | null;
  connected: boolean;
  loadDescriptorFile: (descriptor: Descriptor) => void;
  /** Post a descriptor's text to the connected node; raises the banner and logs on its own. */
  addMetamodel: (text: string, fileName: string) => Promise<MetamodelAddOutcome>;
}

export function MetamodelBrowser({
  metamodel,
  source,
  connected,
  loadDescriptorFile,
  addMetamodel,
}: MetamodelBrowserProps) {
  const [fileError, setFileError] = useState<string | null>(null);
  const [addState, setAddState] = useState<AddState>({ phase: 'idle' });
  const [query, setQuery] = useState('');
  const digest = useDescriptorDigest(metamodel);

  const onFile = (file: File | undefined) => {
    if (file === undefined) return;
    setFileError(null);
    file
      .text()
      .then((text) => loadDescriptorFile(validateDescriptor(JSON.parse(text))))
      .catch((err) => setFileError(err instanceof Error ? err.message : String(err)));
  };

  const onAddFile = (file: File | undefined) => {
    if (file === undefined) return;
    setAddState({ phase: 'sending', file: file.name });
    void addDescriptorFile(file, addMetamodel).then(setAddState);
  };

  /*
   * Both pickers clear their input after a choice. Without it the second
   * choice of the SAME file fires no change event, which is exactly the
   * sequence a refusal puts a person in: pick the wrong file, read why, fix
   * it on disk, pick it again — and nothing happens.
   */
  const actions = (
    <div className="me-meta__actions">
      {connected && (
        <label className="me-btn me-btn--primary">
          <Upload {...ICON} size={14} aria-hidden="true" />
          Add to this replica…
          <input
            type="file"
            className="me-sr-only"
            data-testid="add-metamodel-file"
            accept=".json,application/json"
            disabled={addState.phase === 'sending'}
            onChange={(event) => {
              onAddFile(event.target.files?.[0]);
              event.target.value = '';
            }}
          />
        </label>
      )}
      <label className="me-btn">
        Load into this editor…
        <input
          type="file"
          className="me-sr-only"
          data-testid="load-descriptor-file"
          accept=".json,application/json"
          onChange={(event) => {
            onFile(event.target.files?.[0]);
            event.target.value = '';
          }}
        />
      </label>
    </div>
  );

  const notes = (
    <>
      {addState.phase === 'sending' && (
        <p className="me-meta__note me-subtle">sending {addState.file} to this replica…</p>
      )}
      {addState.phase === 'served' && (
        <p className="me-meta__note me-meta__note--ok" role="status">
          {addState.line}
        </p>
      )}
      {addState.phase === 'refused' && (
        <p className="me-meta__note me-meta__note--bad" role="alert">
          {addState.detail}
        </p>
      )}
      {fileError !== null && <p className="me-form__error">descriptor rejected: {fileError}</p>}
    </>
  );

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
            <code>GET /api/metamodel</code> returned 404. Give this replica a descriptor so models can
            be created in that language, or load one into the editor to read a document with types.
          </>
        }
        tone="warn"
      >
        {actions}
        <p className="me-well">arachne describe &lt;file.ecore&gt;</p>
        {notes}
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
        {/*
          The identity, and the one thing on this tab another window needs.
          A model is joined by id *and* digest, and until now the digest was
          nowhere a person could read it — a tooltip on a model row and the
          invisible `value` of a dropdown option. `user-select: all` so one
          click takes the whole 64 characters, wrapped or not.
        */}
        <dt>digest</dt>
        <dd className="me-mono me-meta__digest">
          {digest ?? <span className="me-subtle">computing…</span>}
        </dd>
        <dt>root classes</dt>
        <dd className="me-mono">
          {metamodel.rootClasses.length > 0 ? metamodel.rootClasses.join(', ') : '—'}
        </dd>
        <dt>source</dt>
        <dd>{source === 'node' ? 'served by the node' : 'loaded from file'}</dd>
      </dl>

      {/* Under the summary rather than under the class list: a control at the
          foot of a hundred rows is a control nobody finds. */}
      {actions}
      {notes}

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
