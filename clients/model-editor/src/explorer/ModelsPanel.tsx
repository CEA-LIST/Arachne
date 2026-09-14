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
 *
 * # Which of the two leads, and why it changed
 *
 * The digest does. It used to be the other way round — the dropdown first,
 * in force from the start, and reading an unanswered choice as the listing's
 * first entry — and rehearsing the demo on 2026-09-14 that cost a model.
 * Alice held `library` and hosted a Library model; bob did not hold
 * `library`, so bob's dropdown could not offer it, and `behaviortree` was
 * picked from the dropdown instead. Nothing objected: bob holds
 * `behaviortree`, so the binding was taken at once, and bob went on serving
 * the behaviortree descriptor for a Library document until the replica was
 * restarted, because there is no un-join.
 *
 * So the two controls are ordered by what they can be wrong about. A digest
 * is the model's identity and comes from whoever gave you the id, and the
 * node checks it against the language the model turns out to carry — a wrong
 * digest is *found* wrong. The dropdown offers this replica's own listing,
 * which says what this replica can serve and nothing whatever about the
 * model, so every entry in it is a plausible answer and at most one of them
 * is true. It stays, because joining a model in a language you hold is the
 * common case and is one choice away; what it no longer does is answer for
 * you, or go out unremarked. `UncheckedJoin` below is the question it asks,
 * and it carries the digest so that it can be read against the one you were
 * given.
 */

import { useRef, useState } from 'react';
import { isModelId, type HostedModel, type MetamodelId, type MetamodelListing, type ModelId } from '../api/types';
import { EmptyState } from '../common/EmptyState';
import { joinTarget, seenJoin } from './joinTarget';
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

/**
 * A descriptor choice: the package first, the nsURI after it, one option per
 * digest.
 *
 * `placeholder` is what makes a join's dropdown different from a create's. A
 * create is written from a descriptor this node holds, so showing the first
 * one and meaning it is right. A join is a claim about someone else's model,
 * so an unanswered dropdown must stay unanswered, and the placeholder is the
 * option that says so and carries the empty digest.
 */
function MetamodelSelect({
  label,
  metamodels,
  value,
  onChange,
  placeholder,
}: {
  label: string;
  metamodels: MetamodelListing[];
  value: string;
  onChange: (digest: string) => void;
  placeholder?: string;
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
      {metamodels.length > 0 && placeholder !== undefined && <option value="">{placeholder}</option>}
      {metamodels.map((entry) => (
        <option key={entry.digest} value={entry.digest}>
          {entry.package.length > 0 ? `${entry.package} — ${entry.nsURI}` : entry.nsURI}
        </option>
      ))}
    </select>
  );
}

/** What to call a language in a sentence: its package, or its nsURI if it has none. */
function nameOf(entry: MetamodelListing): string {
  return entry.package.length > 0 ? entry.package : entry.nsURI;
}

/**
 * A join named from this replica's own listing, held back until it is
 * confirmed.
 *
 * The replica has received nothing it can read about the model — that is what
 * joining means — so it cannot say whether the language chosen for it is the
 * right one, and after the join it is too late: the node takes a binding to a
 * language it holds without a word, and there is no un-join.
 */
interface UncheckedJoin {
  /** Which block raised it, so the question is asked where the click was. */
  from: 'seen' | 'byId';
  id: ModelId;
  metamodelId: MetamodelId;
  /** The language as the question names it. */
  name: string;
}

/**
 * The question a dropdown-named join is asked before it goes out.
 *
 * It prints the digest, which is the point of it rather than a detail: the
 * person joining was given one with the model id, and two digests side by
 * side is the only check available to anybody here. The sentences say what
 * cannot be taken back, because nothing else in the UI will get the chance
 * to — a wrong binding does not fail, it succeeds quietly and serves the
 * wrong language afterwards.
 */
function UncheckedJoinPrompt({
  join,
  onCancel,
  onConfirm,
}: {
  join: UncheckedJoin;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  return (
    <div className="me-models__unchecked" role="group" aria-label="Confirm a join this replica cannot check">
      <p className="me-models__unchecked-ask">
        This replica has received nothing it can read about <span className="me-mono">{join.id}</span>, so it
        cannot tell what language that model is written in. Join it as <strong>{join.name}</strong>, under this
        digest?
      </p>
      <p className="me-mono me-models__unchecked-digest">{join.metamodelId.digest}</p>
      <p className="me-connect__hint">
        Read it against the digest you were given with the model id. A wrong one is not refused: this replica
        holds that language, so the binding is taken, and there is no un-join — the model stays bound to the
        wrong language here until this replica restarts. If the two do not match, cancel and paste the digest
        you were given into the field below instead.
      </p>
      <div className="me-models__unchecked-actions">
        <button type="button" className="me-btn" onClick={onCancel}>
          Cancel
        </button>
        <button type="button" className="me-btn" onClick={onConfirm}>
          Join as {join.name}
        </button>
      </div>
    </div>
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
  // The digest leads: it is the control that can name the model's actual
  // language, and the one the node can later find wrong.
  const [joinSource, setJoinSource] = useState<'held' | 'digest'>('digest');
  const [joinId, setJoinId] = useState('');
  const [joinError, setJoinError] = useState<string | null>(null);
  const [unchecked, setUnchecked] = useState<UncheckedJoin | null>(null);
  // Where a seen row hands its id to, and what the person types next.
  const joinFormRef = useRef<HTMLFormElement>(null);
  const joinDigestRef = useRef<HTMLInputElement>(null);

  /**
   * The listing entry behind a digest. Only a create falls back to the first
   * one: a join must not, or an unanswered dropdown becomes an answer.
   */
  const pick = (digest: string): MetamodelId | null => {
    const entry = metamodels.find((m) => m.digest === (digest.length > 0 ? digest : metamodels[0]?.digest));
    return entry === undefined ? null : { nsURI: entry.nsURI, digest: entry.digest };
  };

  /** Ask before a dropdown-named join goes out, and forget any older question. */
  const ask = (join: UncheckedJoin) => setUnchecked(join);

  /**
   * Carry a seen model's id down to the Join by id form and leave the cursor
   * in the digest field.
   *
   * The scroll is not a flourish. The two blocks are far enough apart in a
   * panel this narrow that a click whose whole effect happened below the
   * fold reads as a click that did nothing, and the person's next move would
   * be to click it again. The form is brought to where the eye is, and the
   * focus is set without a second scroll fighting the first.
   */
  const handOff = (id: ModelId) => {
    setJoinId(id);
    setJoinSource('digest');
    setJoinError(null);
    setUnchecked(null);
    joinFormRef.current?.scrollIntoView({ block: 'nearest' });
    joinDigestRef.current?.focus({ preventScroll: true });
  };
  const confirmed = () => {
    if (unchecked === null) return;
    const join = unchecked;
    setUnchecked(null);
    onJoin(join.id, join.metamodelId);
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
                {...(model.metamodelId !== null ? { title: model.metamodelId.nsURI } : {})}
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
              {/*
                The digest, beside the id, for the same reason the id is here
                in full: these are the two things a person carries to another
                window, and until now this one was a `title` tooltip — visible
                to a mouse, and untypable. A join asks for both, so they are
                shown together, and the value is its own element so that one
                click takes the digest and nothing around it.
              */}
              {model.metamodelId !== null && (
                <span className="me-models__digest-line">
                  <span className="me-subtle">metamodel </span>
                  <span className="me-mono me-models__digest-value">{model.metamodelId.digest}</span>
                </span>
              )}
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
                value={seenDigest}
                placeholder="choose the language it is written in"
                onChange={(digest) => {
                  setSeenDigest(digest);
                  setUnchecked(null);
                }}
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
                    onClick={() => {
                      // Never disabled, and least of all on a replica that
                      // serves nothing: that is the one whose only way in is
                      // the digest, and the hand-off is how it gets there.
                      const next = seenJoin(seenDigest, metamodels);
                      if (next.kind === 'handOff') {
                        handOff(id);
                        return;
                      }
                      const entry = metamodels.find((m) => m.digest === next.metamodelId.digest);
                      ask({
                        from: 'seen',
                        id,
                        metamodelId: next.metamodelId,
                        name: entry === undefined ? next.metamodelId.digest : nameOf(entry),
                      });
                    }}
                  >
                    Join
                  </button>
                  {/*
                    What this replica knows about the row, which is the id and
                    nothing else. It is here rather than only in the hint
                    below because the dropdown sits directly above the list
                    and reads as though it held the answer.
                  */}
                  <span className="me-subtle me-models__unknown">language unknown here</span>
                </li>
              ))}
            </ul>
            {unchecked !== null && unchecked.from === 'seen' && (
              <UncheckedJoinPrompt
                join={unchecked}
                onCancel={() => setUnchecked(null)}
                onConfirm={confirmed}
              />
            )}
            {/*
              This used to say the node refuses the wrong one, naming the
              digest it expected. It does not, and cannot: a seen id is a
              model this replica has heard frames for and could not read, so
              the node has nothing to check a language against until the
              model's own history reaches it. Choosing the wrong one here is
              taken, and only found out afterwards.
            */}
            <p className="me-connect__hint">
              This replica knows the ids it has heard and not what they are written in, and the list above is
              its own — what it can serve, not what these models are. So a choice there is a guess unless you
              were told. Leave it unanswered and Join carries the id down to Join by id with the cursor in the
              digest field: paste the digest you were given and submit, and that is the naming the node can
              check against the model itself.
            </p>
          </>
        )}
      </section>

      <form
        ref={joinFormRef}
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
          // A typed digest goes straight out. It is the model's identity,
          // it came from whoever gave you the id, and the node holds it
          // against the descriptor the model turns out to carry, so a wrong
          // one is found wrong. A dropdown choice is none of those things:
          // it is this replica's own listing, every entry of which is
          // plausible and at most one of which is true, so it is asked.
          if (joinSource === 'held') {
            const entry = metamodels.find((m) => m.digest === target.metamodelId.digest);
            ask({
              from: 'byId',
              id,
              metamodelId: target.metamodelId,
              name: entry === undefined ? target.metamodelId.digest : nameOf(entry),
            });
            return;
          }
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
              setUnchecked(null);
            }}
          />
        </label>
        {/*
          Two ways of naming the metamodel, the digest first because it is the
          one that can be right about someone else's model. The dropdown is
          fed by this window's own /api/metamodels, so it cannot express a
          digest this replica does not hold — which is exactly the digest a
          replica needs when it joins a model written in a language it has
          never seen — and what it can express is a list of languages this
          replica happens to serve, none of which it has any reason to think
          the model is written in. Choosing in either control puts that
          control in force, so the dropdown is still one choice away; what
          follows it is a question rather than a join.
        */}
        <fieldset className="me-models__bound">
          <legend className="me-connect__label">Bound to</legend>
          <label className="me-models__choice">
            <input
              type="radio"
              name="join-bound-to"
              value="digest"
              checked={joinSource === 'digest'}
              onChange={() => {
                setJoinSource('digest');
                setJoinError(null);
                setUnchecked(null);
              }}
            />
            <span>the digest given to you with the model id</span>
          </label>
          <input
            ref={joinDigestRef}
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
              setUnchecked(null);
            }}
          />
          <p className="me-connect__hint">
            A model can be joined under a language this replica does not hold yet, which is why this field
            leads and the dropdown does not: the node hosts the model with its binding pending and the
            language arrives with the model&apos;s own history. Until it does, the node refuses every write
            into that model, and says so in its own words. The digest is also the model&apos;s identity, so
            the node holds it against what the model turns out to carry — this is the naming that can be
            found wrong instead of merely believed.
          </p>
          <label className="me-models__choice">
            <input
              type="radio"
              name="join-bound-to"
              value="held"
              checked={joinSource === 'held'}
              onChange={() => {
                setJoinSource('held');
                setJoinError(null);
                setUnchecked(null);
              }}
            />
            <span>or a metamodel this replica already serves</span>
          </label>
          <MetamodelSelect
            label="Metamodel of the model to join"
            metamodels={metamodels}
            value={joinDigest}
            placeholder="choose a language this replica serves"
            onChange={(digest) => {
              setJoinDigest(digest);
              setJoinSource('held');
              setJoinError(null);
              setUnchecked(null);
            }}
          />
          <p className="me-connect__hint">
            This listing is what this replica can serve, not what the model is written in, so a choice here
            is confirmed before it is sent.
          </p>
        </fieldset>
        {joinError !== null && <p className="me-connect__error">{joinError}</p>}
        {unchecked !== null && unchecked.from === 'byId' && (
          <UncheckedJoinPrompt join={unchecked} onCancel={() => setUnchecked(null)} onConfirm={confirmed} />
        )}
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
