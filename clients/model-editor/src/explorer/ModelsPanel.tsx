/**
 * The model list of one node: what `GET /api/models` lists, a way to open
 * any of it in a tab, a way to create a model against one of the descriptors
 * the node holds (`GET /api/metamodels`; the node mints the id and writes the
 * header), and a way to join a model by an id learned out of band.
 *
 * The list is the node's and nothing wider: there is no cluster-wide
 * catalog, so a model created on another node is reached by pasting its id
 * here, together with the metamodel it is bound to, which the node requires
 * at registration and cannot infer from an id alone.
 */

import { useState } from 'react';
import { isModelId, type HostedModel, type MetamodelId, type MetamodelListing, type ModelId } from '../api/types';
import { EmptyState } from '../common/EmptyState';
import { DEFAULT_LOG_LABEL, packageOf, shortId } from '../ui/modelLabel';
import { Plug, RefreshCw } from '../ui/icons';
import { ICON } from '../ui/iconProps';

interface ModelsPanelProps {
  connected: boolean;
  hosted: HostedModel[] | null;
  metamodels: MetamodelListing[];
  openIds: readonly ModelId[];
  selected: ModelId | null;
  onRefresh: () => void;
  onCreate: (metamodelId: MetamodelId) => void;
  onJoin: (id: ModelId, metamodelId: MetamodelId) => void;
  onOpen: (id: ModelId) => void;
}

/** A descriptor choice: the package first, the nsURI after it, one option per digest. */
function MetamodelSelect({
  label,
  metamodels,
  value,
  onChange,
}: {
  label: string;
  metamodels: MetamodelListing[];
  value: string;
  onChange: (digest: string) => void;
}) {
  return (
    <select
      className="me-select me-models__select"
      aria-label={label}
      value={value}
      onChange={(event) => onChange(event.target.value)}
      disabled={metamodels.length === 0}
    >
      {metamodels.length === 0 && <option value="">no descriptor on this node</option>}
      {metamodels.map((entry) => (
        <option key={entry.digest} value={entry.digest}>
          {entry.package.length > 0 ? `${entry.package} — ${entry.nsURI}` : entry.nsURI}
        </option>
      ))}
    </select>
  );
}

export function ModelsPanel({
  connected,
  hosted,
  metamodels,
  openIds,
  selected,
  onRefresh,
  onCreate,
  onJoin,
  onOpen,
}: ModelsPanelProps) {
  const [createDigest, setCreateDigest] = useState('');
  const [joinDigest, setJoinDigest] = useState('');
  const [joinId, setJoinId] = useState('');
  const [joinError, setJoinError] = useState<string | null>(null);

  const pick = (digest: string): MetamodelId | null => {
    const entry = metamodels.find((m) => m.digest === (digest.length > 0 ? digest : metamodels[0]?.digest));
    return entry === undefined ? null : { nsURI: entry.nsURI, digest: entry.digest };
  };

  if (!connected) {
    return (
      <EmptyState icon={Plug} title="Not connected" body="Connect to a node to list the models it hosts." />
    );
  }

  return (
    <div className="me-models">
      <div className="me-panel__toolbar me-noprint">
        <span className="me-subtle">
          {hosted === null ? 'listing…' : `${hosted.length} hosted model${hosted.length === 1 ? '' : 's'}`}
        </span>
        <span className="me-log__spacer" />
        <button type="button" className="me-iconbtn" aria-label="Refresh model list" title="Refresh" onClick={onRefresh}>
          <RefreshCw {...ICON} size={14} aria-hidden="true" />
        </button>
      </div>

      <ul className="me-models__list" aria-label="Hosted models">
        {hosted !== null && hosted.length === 0 && (
          <li className="me-models__none me-muted">This node hosts no model yet.</li>
        )}
        {(hosted ?? []).map((model) => {
          const pkg = model.metamodelId === null ? DEFAULT_LOG_LABEL : (packageOf(model.metamodelId, metamodels) ?? 'model');
          const open = openIds.includes(model.modelId);
          return (
            <li
              key={model.modelId}
              className={model.modelId === selected ? 'me-models__row me-models__row--selected' : 'me-models__row'}
              data-model-id={model.modelId}
            >
              <span className="me-models__pkg">{pkg}</span>
              <span className="me-mono me-models__id" title={model.modelId}>
                {shortId(model.modelId)}…
              </span>
              {model.metamodelId !== null && (
                <span className="me-subtle me-truncate me-models__ns" title={`digest ${model.metamodelId.digest}`}>
                  {model.metamodelId.nsURI}
                </span>
              )}
              <button
                type="button"
                className={open ? 'me-btn me-btn--sm' : 'me-btn me-btn--sm me-btn--primary'}
                aria-label={`${open ? 'Show' : 'Open'} ${model.modelId}`}
                onClick={() => onOpen(model.modelId)}
              >
                {open ? 'Show' : 'Open'}
              </button>
            </li>
          );
        })}
      </ul>

      <form
        className="me-models__form"
        aria-label="New model"
        onSubmit={(event) => {
          event.preventDefault();
          const id = pick(createDigest);
          if (id !== null) onCreate(id);
        }}
      >
        <h3 className="me-models__heading">New model</h3>
        <label className="me-connect__field">
          <span className="me-connect__label">Metamodel</span>
          <MetamodelSelect
            label="Metamodel for the new model"
            metamodels={metamodels}
            value={createDigest.length > 0 ? createDigest : (metamodels[0]?.digest ?? '')}
            onChange={setCreateDigest}
          />
        </label>
        <p className="me-connect__hint">
          The node mints the id and writes the <code>__model</code> header; the model opens in a tab.
        </p>
        <button type="submit" className="me-btn me-btn--primary" disabled={metamodels.length === 0}>
          Create model
        </button>
      </form>

      <form
        className="me-models__form"
        aria-label="Join model by id"
        onSubmit={(event) => {
          event.preventDefault();
          const id = joinId.trim();
          if (!isModelId(id)) {
            setJoinError('a model id is 32 lowercase hex characters');
            return;
          }
          const metamodelId = pick(joinDigest);
          if (metamodelId === null) {
            setJoinError('choose the metamodel the model is bound to');
            return;
          }
          setJoinError(null);
          onJoin(id, metamodelId);
        }}
      >
        <h3 className="me-models__heading">Join by id</h3>
        <label className="me-connect__field">
          <span className="me-connect__label">Model id</span>
          <input
            className="me-input me-mono"
            type="text"
            aria-label="Model id"
            value={joinId}
            placeholder="32 hex characters, from whoever created it"
            spellCheck={false}
            onChange={(event) => {
              setJoinId(event.target.value);
              setJoinError(null);
            }}
          />
        </label>
        <label className="me-connect__field">
          <span className="me-connect__label">Bound to</span>
          <MetamodelSelect
            label="Metamodel of the model to join"
            metamodels={metamodels}
            value={joinDigest.length > 0 ? joinDigest : (metamodels[0]?.digest ?? '')}
            onChange={setJoinDigest}
          />
        </label>
        {joinError !== null && <p className="me-connect__error">{joinError}</p>}
        <p className="me-connect__hint">
          This node hosts the id with no history and asks its peers for the model; the header arrives with it.
        </p>
        <button type="submit" className="me-btn" disabled={metamodels.length === 0}>
          Join model
        </button>
      </form>
    </div>
  );
}
