/**
 * The model list of one node: what `GET /api/models` lists, a way to open
 * any of it in a tab, a way to create a model against one of the descriptors
 * the node holds (`GET /api/metamodels`; the node mints the id), and a way to
 * join a model by an id learned out of band.
 *
 * The list is the node's and nothing wider: there is no cluster-wide
 * catalog, so a model created on another node is reached by pasting its id
 * here, together with the metamodel it is bound to, which the node requires
 * at registration and cannot infer from an id alone.
 *
 * Beside it, the ids the node has seen go by on frames for models it does not
 * host, which spares the retyping in the common case. That list is traffic,
 * not a catalog: a model nobody has touched since this node connected has
 * sent no frame and so is not in it, which is why the join-by-id form stays.
 *
 * That form names the metamodel two ways, and the second is what makes late
 * binding reachable from the browser. A dropdown fed by this window's own
 * `GET /api/metamodels` can only offer what this replica already serves, so a
 * replica that has never seen a language could not express the join that
 * would teach it one. A digest typed straight in can; `joinTarget.ts` says
 * what is checked before it is sent, and what is left to the node.
 */

import { useState } from 'react';
import { isModelId, type HostedModel, type MetamodelId, type MetamodelListing, type ModelId } from '../api/types';
import { EmptyState } from '../common/EmptyState';
import { joinTarget } from './joinTarget';
import { DEFAULT_LOG_LABEL, packageOf } from '../ui/modelLabel';
import { Plug, RefreshCw } from '../ui/icons';
import { ICON } from '../ui/iconProps';

interface ModelsPanelProps {
  connected: boolean;
  hosted: HostedModel[] | null;
  /** Ids the node has seen traffic for and does not host; disjoint from `hosted`. */
  seen: readonly ModelId[];
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
  seen,
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
  const [seenDigest, setSeenDigest] = useState('');
  const [joinPasted, setJoinPasted] = useState('');
  const [joinSource, setJoinSource] = useState<'held' | 'digest'>('held');
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
              {/* Always a cell, empty for the default log, so the button keeps its column. */}
              <span
                className="me-subtle me-truncate me-models__ns"
                {...(model.metamodelId !== null ? { title: `digest ${model.metamodelId.digest}` } : {})}
              >
                {model.metamodelId?.nsURI ?? ''}
              </span>
              <button
                type="button"
                className={open ? 'me-btn me-btn--sm' : 'me-btn me-btn--sm me-btn--primary'}
                aria-label={`${open ? 'Show' : 'Open'} ${model.modelId}`}
                onClick={() => onOpen(model.modelId)}
              >
                {open ? 'Show' : 'Open'}
              </button>
              {/*
                All 32 characters, on a line of their own, and `user-select:
                all` so one click takes the whole id and nothing around it.
                Truncated, this was the one thing in the panel nobody could
                use: a model created on one replica is not listed on the
                other, so joining it means moving its id by hand.
              */}
              <span className="me-mono me-models__id">{model.modelId}</span>
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
          The node mints the id and opens the model&apos;s log; the model opens in a tab. Its id is listed
          above, in full: that is what another replica needs to join it.
        </p>
        <button type="submit" className="me-btn me-btn--primary" disabled={metamodels.length === 0}>
          Create model
        </button>
      </form>

      {/*
        The fourth block: what this node has *heard from*, which is not the
        same as what exists. The heading says "since connecting" and the hint
        says a quiet model is missing, so a reader who knows of a model and
        does not find it here is told why before they look for a bug.
      */}
      <section className="me-models__form" aria-label="Seen since connecting">
        <h3 className="me-models__heading">Seen since connecting</h3>
        <p className="me-connect__hint">
          Models this node has received traffic for and does not host. Not a list of the session: a model
          nobody has touched since this node connected has sent nothing and is not here — join that one by
          id below.
        </p>
        {seen.length === 0 ? (
          <p className="me-muted me-models__none">Nothing seen yet.</p>
        ) : (
          <>
            <label className="me-connect__field">
              <span className="me-connect__label">Bound to</span>
              <MetamodelSelect
                label="Metamodel of the seen model to join"
                metamodels={metamodels}
                value={seenDigest.length > 0 ? seenDigest : (metamodels[0]?.digest ?? '')}
                onChange={setSeenDigest}
              />
            </label>
            <ul className="me-models__list me-models__seen" aria-label="Models seen since connecting">
              {seen.map((id) => (
                <li key={id} className="me-models__row" data-seen-id={id}>
                  <span className="me-mono me-models__id">{id}</span>
                  <button
                    type="button"
                    className="me-btn me-btn--sm"
                    aria-label={`Join ${id}`}
                    disabled={metamodels.length === 0}
                    onClick={() => {
                      const metamodelId = pick(seenDigest);
                      if (metamodelId !== null) onJoin(id, metamodelId);
                    }}
                  >
                    Join
                  </button>
                </li>
              ))}
            </ul>
            <p className="me-connect__hint">
              The node cannot say which metamodel a model it does not host is bound to, so choose it above;
              the wrong one is refused by the node, naming the digest it expected.
            </p>
          </>
        )}
      </section>

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
          const target = joinTarget(
            joinSource === 'held'
              ? { source: 'held', digest: joinDigest }
              : { source: 'digest', digest: joinPasted },
            metamodels,
          );
          if (!target.ok) {
            setJoinError(target.error);
            return;
          }
          setJoinError(null);
          onJoin(id, target.metamodelId);
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
        {/*
          Two ways of naming the metamodel, and the second one is not a
          convenience. The dropdown is fed by this window's own
          /api/metamodels, so it cannot express a digest this replica does not
          hold — which is exactly the digest a replica needs when it joins a
          model written in a language it has never seen. Choosing in either
          control puts that control in force, so the dropdown path is still
          one click and nothing about it changed.
        */}
        <fieldset className="me-models__bound">
          <legend className="me-connect__label">Bound to</legend>
          <label className="me-models__choice">
            <input
              type="radio"
              name="join-bound-to"
              value="held"
              checked={joinSource === 'held'}
              onChange={() => {
                setJoinSource('held');
                setJoinError(null);
              }}
            />
            <span>a metamodel this replica serves</span>
          </label>
          <MetamodelSelect
            label="Metamodel of the model to join"
            metamodels={metamodels}
            value={joinDigest.length > 0 ? joinDigest : (metamodels[0]?.digest ?? '')}
            onChange={(digest) => {
              setJoinDigest(digest);
              setJoinSource('held');
              setJoinError(null);
            }}
          />
          <label className="me-models__choice">
            <input
              type="radio"
              name="join-bound-to"
              value="digest"
              checked={joinSource === 'digest'}
              onChange={() => {
                setJoinSource('digest');
                setJoinError(null);
              }}
            />
            <span>a digest, typed or pasted</span>
          </label>
          <input
            className="me-input me-mono me-models__digest"
            type="text"
            aria-label="Metamodel digest"
            value={joinPasted}
            placeholder="64 hex characters, the digest of the metamodel"
            spellCheck={false}
            onChange={(event) => {
              setJoinPasted(event.target.value);
              setJoinSource('digest');
              setJoinError(null);
            }}
          />
          <p className="me-connect__hint">
            A model can be joined under a language this replica does not hold yet, which is why a digest can
            be given here and not only chosen above: the node hosts the model with its binding pending and
            the language arrives with the model&apos;s own history. Until it does, the node refuses every
            write into that model, and says so in its own words.
          </p>
        </fieldset>
        {joinError !== null && <p className="me-connect__error">{joinError}</p>}
        <p className="me-connect__hint">
          This node hosts the id with no history and asks its peers for the model; the log arrives from them.
        </p>
        {/*
          Never disabled on an empty listing, unlike Create above. A create
          needs a descriptor in hand and there is nothing to write the opening
          operation from; a join needs only a digest, and a replica serving no
          descriptor at all is precisely the one that has to type one. An
          empty dropdown with the dropdown in force is answered on submit,
          beside the model id's own answer, rather than by a dead button.
        */}
        <button type="submit" className="me-btn">
          Join model
        </button>
      </form>
    </div>
  );
}
