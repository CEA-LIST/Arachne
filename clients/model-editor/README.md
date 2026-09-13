# Model Editor

A metamodel-agnostic web editor for the models a moirai node hosts. It talks only to the node's HTTP API, lists the node's models, opens any number of them in tabs, discovers each one's metamodel from the node, and shapes its UI accordingly — the same binary serves different metamodels, and the editor follows. No class name from any sample metamodel appears anywhere in `src/`.

## Run

The editor talks to interpreted replicas — `model_node`, which reads its merge rules from the metamodel descriptor at run time. Two of them, from the moirai worktree:

```sh
cd <moirai worktree>/docker/compose
./stack_interpreter.sh infra up
./stack_interpreter.sh up alice --port 8081
./stack_interpreter.sh up bob   --port 8082
```

Then in this directory:

```sh
npm install && npm run dev
```

Open the printed URL in two windows, connect one to `http://127.0.0.1:8081` and the other to `http://127.0.0.1:8082`, and in the first window create a model from the **Models** tab. Its id is listed there in full; copy it, and in the second window use **Join by id**, pasting the id and choosing the same metamodel. Both windows now edit one model on two replicas, which is how convergence is demonstrated.

**A model created on one replica is not listed on the other until that replica is asked to host it.** That is not a bug and it is the first thing that looks like one: a node lists what it hosts, there is no cluster-wide catalog, and joining by id is how a second replica comes to host a model. `./stack_interpreter.sh --help` documents the rest; `down-all` removes everything.

The editor speaks the interpreted `ModelOp` dialect and nothing else. A generated `network_node` answers every operation it sends with `400 Invalid op JSON`, so the old `rig.sh --edit` path no longer works for it.

Scripts in this directory:

```sh
npm run dev        # dev server
npm test           # unit tests (vitest), plus the live-node scenarios when the model_node binary exists
npm run e2e        # the level-4 harness: ip-editor against a running stack, and mp26 in headless Chrome (see below)
npm run lint       # oxlint
npm run build      # type-check + production bundle in dist/
```

## The UI

A four-region modelling IDE layout; one light theme, print-worthy for paper figures (`@media print` drops the chrome, so print-to-PDF gives a clean figure).

- **Top bar** — app identity, then the context of the selected model tab: its id, the metamodel `package` read from its descriptor at runtime and whether it came from the node or from a file, the binding check's verdict and the store's word. On the right: the batch in flight as a determinate bar, the sync chip (the selected tab's poll), the replica id from `/api/health`, keyboard help, and Connect. **The node URL and poll interval live in the Connect popover**, not on the page.
- **Document tabs** — one per open model, under the top bar: the metamodel package and the first eight characters of the id, a status dot while it opens or when it is refused, a close button, and `Models…` to get back to the list. The panels below show the selected tab.
- **Explorer** (left) — `Models`, `Model` and `Metamodel` tabs. The models tab lists what the node hosts (`GET /api/models`), opens any of it, creates a model against one of the node's descriptors, and joins a model by id. The model tab is an ARIA tree with structural type icons, indent guides, expand/collapse, a filter that highlights matches and keeps their ancestors, an element count, and per-row add/remove actions. The metamodel tab is a searchable class browser (supertypes, abstract marker, feature counts) and hosts the descriptor-file fallback.
- **Properties** (right) — the selected element: kind icon, label, eClass badge, a clickable path breadcrumb, Copy path and Delete element; then `Attributes`, `Containments` and `References` as collapsible sections in the descriptor's own feature order. Required features carry `*`, the id attribute carries a key icon, every attribute carries a type chip.
- **Console** (bottom, `⌘/Ctrl + J`) — collapsed to a bar that still reports the entry count, the newest line and a danger dot; open on `Action log` or `Document JSON`.

Every empty surface instructs rather than sitting blank: not connected, no model open, no metamodel, empty document, no filter match, nothing selected, no operations yet.

Keyboard: `↑ ↓` move, `→ ←` expand/collapse or step in and out, `Home`/`End`, `Enter` selects and jumps into the form, `Space` selects in place, type-ahead by name, `*` expands the level, `F2` jumps to the id field, `Delete` removes, `Esc` reverts the focused field to the last synced value, `⌘/Ctrl + K` filter, `⌘/Ctrl + J` console, `⌘/Ctrl + ⇧ + E` export the log, `?` for the full map.

## Many models, in tabs

A model is its log: the node hosts any number, each under a 32-hex `ModelId`, and `GET /api/models` lists them with the `{nsURI, digest}` of the metamodel each was registered under (`null` for the node's default log, which the unscoped routes serve and which the list shows as `default log`). The Models tab is that list, refreshed on connect, after every registration and on demand, and three things can be done from it:

- **Open** a hosted model in a tab.
- **New model**: choose one of the descriptors the node holds (`GET /api/metamodels`, shown as package and nsURI) and `POST /api/models {metamodel_id}`. The node mints the id and opens the model's log with the `Install` operation that carries the descriptor; the editor never chooses an id. The model opens in a tab, and its id is listed in full in the panel, which is what another replica needs to join it.
- **Join by id**: paste an id learned out of band and choose the metamodel it is bound to, then `POST /api/models {model_id, metamodel_id}`. The node hosts the id with no history and asks its peers for the model; the log arrives from them, the `Install` operation with it. There is no cluster-wide catalog: a node lists what it hosts and nothing else, and a model's replication factor is the number of nodes that registered it.

Each open tab is one `ModelSession` (`src/sync/modelSession.ts`) addressed to its model's routes on the node it was opened against: its own document, its own descriptor (`GET /api/model/{id}/metamodel`), its own poll of `GET /api/model/{id}/state` on every interval whether or not it is the selected tab, its own op queue posting to `POST /api/model/{id}/op`, its own field registry so one tab's caret never writes into another's document, and its own binding verdict, store outcome and recorded header. The reducer (`src/state/store.ts`) holds the tabs as a map keyed by id; a session's event for a tab that has been closed is dropped. Disconnecting closes every tab, since every tab was opened against that connection.

## Metamodel discovery

When a model is opened the editor calls `GET /api/model/{id}/metamodel`, which serves the descriptor the model was registered under. The node answers with a formatVersion-2 descriptor: classes with attributes/containments/references, root classes and enums as before, and on every feature the merge rule the node routes it by. The editor reads that rule — an interpreted replica addresses an operation by slot, and both sides compute the slots from the same descriptor. `model_node` holds every `.json` descriptor found under `METAMODEL_DIR`, keyed by digest, and the file named by `METAMODEL_PATH` (default `./metamodel.json`) beside them.

If the node answers 404 (a model with no descriptor to serve), the Metamodel tab offers the labelled fallback: load a descriptor file produced by `arachne describe <file.ecore>`; the binding check then runs against that file.

## Giving a replica a language

The Metamodel tab's other action, beside that one and deliberately not the same action: **Add to this replica** posts a descriptor file's text to `POST /api/metamodels` on the node this window is connected to. The node parses it, serves it from the moment it answers, and a model registers under it on the very next request — nothing restarts and nothing is regenerated, which is the half of the claim a node generated from one metamodel cannot answer at all. The metamodel list is refreshed from that same reply, so the new language is in the New model and Join by id dropdowns immediately. The text is posted exactly as read: the node takes the metamodel's identity over what it parsed, so a re-serialization would be a descriptor nobody wrote. A descriptor the node cannot serve comes back 422 with the parser's own sentence, and that sentence is what the panel prints.

It reaches **one** replica. Bob, next door, goes on listing the metamodels he was started with, and the panel says so in as many words: *This replica serves classdiagram now; another replica learns it only when a model written in classdiagram reaches it.* The route is in-band — the descriptor travels inside the `Install` operation that opened the model's log — so joining a model written in the new language is what makes the second replica serve it too, and that is the demonstration rather than a caveat.

## Editing

The tree is the descriptor's containment structure (labels come from the element's id attribute — the first `isId` attribute, else one named `ID`/`name`, else the first string attribute — falling back to the class name). Selecting an element opens a typed form: text inputs for strings, number inputs (commit on blur/Enter) for int/float, a switch for booleans, a literal dropdown for enums. Containments offer create/add with a concrete-subtype menu where the target class is abstract, plus remove and move up/down; references are pickers over the document's existing instances of the target family, stored as the target's id value. On a fresh document the editor offers root creation from the descriptor's root classes.

Every action lands in the action log with its exact op payloads and the node's verdict. The log is append-only and has no clear button on purpose — it is the evidence. **It lives in the browser tab**: a reload starts a new one, so `Export JSON` (or `⌘⇧E`) is what makes it durable.

## How edits reach the wire

Every edit intent is mapped by `src/crdt/ops.ts` to an `EditOp` — what the user did, said in the vocabulary of the document — and `src/crdt/encode.ts` turns that into the `ModelOp`s an interpreted replica routes, posted **one at a time** (`POST /api/model/{id}/op` takes exactly one op, as the whole body, with nothing wrapping it).

An operation is a path of slots ending in one write: `Variant(class)` for every object on the path, `Field(feature)` for every step into a feature, then the collection step (`Seq`, `Opt`) and the leaf write. `src/crdt/table.ts` computes those slots from the model's own descriptor exactly as `moirai-semantics/src/parse.rs` does — classes sorted by name, enums numbered separately, and a feature's position among its class's *visible* features, own and inherited, sorted by name. Getting that wrong is silent, because an operation addressing the wrong feature is well-formed and the node applies it, so the encoder refuses rather than guesses: a class the document does not name, a feature the class cannot see, a keyed collection or a transparent class all throw at encode time, naming what could not be addressed, and the batch is refused before anything is posted.

The rule decides the write, not the JavaScript type: a text leaf is diffed into at most one `DeleteRange` plus single-character `InsertChar`s, a counter moves by `Inc`/`Dec` at the width the descriptor declares, a flag is `Enable`/`Disable`, a register or an enum is one `Write`, and a set is addressed by value and never by position. A path is encoded against the document the node last served, which is where the class of every object on it comes from; a batch advances a shadow document as it encodes, so a mint and the writes into what it made travel together. One FIFO queue per open model serializes that model's batches so sequences never interleave. Every open tab polls its `GET /api/model/{id}/state` (500 ms, configurable); a field being typed in is never clobbered by a refresh (focus + 500 ms typing threshold, selection restored otherwise). Refused ops (`success:false`) and HTTP errors are surfaced three ways — at the field, in the alert dock, and in the log, where the row names its model — and the log is exportable.

### The binding check at apply

**Against an interpreted node this check is inert, and that is open work.** An interpreted replica writes no `__model` into the document — a model's identity lives in the `Install` operation that opened its log and in the node's own registration, which `GET /api/models` answers — so every verdict is `unbound`, the document is applied under the descriptor the node serves for it, and everything keyed on `bound` below (the projection file, the conformance report) does not happen. The scenarios that assert those behaviours skip with an `E2E-SKIP` line saying exactly this. What follows describes the check as built.

A model's document carries `__model` — its id and the `{nsURI, digest}` of the metamodel it is bound to — written once by the node that created it. Before a fetched state reaches a tab, `src/model/binding.ts` hashes the descriptor the document would be rendered under (the SHA-256 over canonical JSON the node computes, `src/model/digest.ts`) and compares it with the header's digest; it also compares the header with the one it recorded at the first apply, because the header is immutable by rule. On a match the state is applied. On a mismatch nothing is: the alert dock and the action log name both pairs, the tab's model panel says so, the top bar reads `not applied`, and the edit gate holds every control with the same sentence; the tab's session refuses batches at the wire for as long as it lasts, and refuses any batch that would write `__model` regardless. The check is per tab: a refused model in one tab leaves the others editable. A log with no header — the default log of a step-1 deployment — is applied under the loaded descriptor exactly as before, and the top bar reads `unbound`. The digest is always computed over the descriptor's bytes, never read from a label the node reports, so a node that serves the wrong file under the right name is caught too; the editor keeps no descriptor cache keyed by a reported digest for the same reason, each tab fetching its own descriptor once when it opens.

### The model store as projection

After an apply the binding check lets through as `bound`, `src/model/projection.ts` writes the decoded document to the model store (`src/model/store.ts`): one file per model, `<modelId>.json`, holding the document as canonical JSON (compact, keys sorted at every level, the same text `src/model/digest.ts` hashes, no trailing newline), so the file's bytes digest to the value the top bar reports and two editors' files for one model can be compared. In the browser the store is the origin-private file system, directory `models` under the origin's root: no permission prompt, no user gesture, works in headless Chrome, private to the origin; it needs the same secure context (https, localhost, 127.0.0.1) the digest already does. The file is a projection: it is regenerated from the log at every apply and nothing reads it to build state. On connect the document comes from the node and the file is overwritten, so a stale or tampered file never survives a reconnect as rendered state, and a restart trusts the log. A refused apply writes nothing, and so does a log with no header, which has no id to file under. A store that fails or is absent never touches the view: the top bar reads `stored` with the file, its size and its digest in the title, or `not stored`, `store failed` or `no store` with the reason, and a failed write lands once in the alert dock and the action log. `src/model/fsStore.ts` is the same store over a directory, for the tests and the level-4 harness. Exporting the file to a folder of the user's choice (the File System Access API) is not built.

### The level-4 harness

`npm run e2e` runs three scenarios. **ip-editor** (`e2e/ipEditor.e2e.ts`) drives the editor's own `ModelSession` — the same encoder, queue and client a control in the browser goes through — against the two replicas of the stack above, named by `MOIRAI_LIVE_NODES` (default `:8081,:8082`): it creates a model and an element, edits a string attribute and reads the characters back in order, inserts children into an ordered containment at chosen positions and reads the order back, has the second replica's session edit the same model until both agree, and checks that the two things the editor refuses on its own — the model header, and a feature no class can see — are refused before anything is posted. Unit tests cannot make that claim: they compare operations with operations, and a wrong slot is a well-formed operation. With no stack up it prints an `E2E-SKIP` line and skips.

**add-metamodel** (`e2e/addMetamodel.e2e.ts`) runs the language-at-run-time claim in headless Chrome against the same two replicas of the stack: it builds and serves the editor, connects a page to alice, hands a descriptor file to the Metamodel tab's **Add to this replica** picker, and then reads the consequences off the nodes themselves — alice lists the language where it did not before, the New model dropdown offers it with no reload, a model is created in it and a `name` typed into the properties form, and bob, whose own `GET /api/metamodels` did not have it, joins that model by id, converges on the same document and ends up listing the language too. Last, a file that is not a descriptor comes back 422 and the panel prints the node's sentence (`not a descriptor this node can serve: …`) rather than a status. The descriptor must be one no replica holds or the first assertion is vacuous, and the image ships `examples/` as `/metamodels`, so one checked in beside the others would be preloaded everywhere: `MOIRAI_DEMO_DESCRIPTOR` names a descriptor file when there is one to hand (`arachne describe examples/class_diagram.ecore`), and without it the scenario retags a descriptor alice serves under an nsURI unique to the run, which is a different digest and so a different metamodel.

**mp26** runs the validation plan's scenario end to end (`e2e/mp26.e2e.ts`, under `vitest.e2e.config.ts`): it builds the editor with `vite build` and serves it with `vite preview` on the loopback interface (a secure context, which the store's OPFS and the digest's `crypto.subtle` need), starts two `model_node` processes peered with each other with `METAMODEL_DIR` holding both `bt.metamodel.json` and `uml.metamodel.json` (`src/testing/liveNodes.ts`, the e2e process backend's recipe), launches the system Chrome headless through `puppeteer-core` with a throwaway profile, and drives two pages in two browser contexts through the editor's own controls: connect each to its node; create a behaviour-tree model on editor-a and join it by id on editor-b; create a SimpleUML model on editor-b and join it by id on editor-a; four tabs; rename a Sequence in the behaviour-tree tab and add a Class named Door in the UML tab. It asserts per-model convergence through `GET /api/model/{id}/state` on both nodes and through what each tab renders (the console's Document JSON view), no cross-talk (the behaviour-tree document never holds a `Class` or a UML key, the UML document never a `Sequence` or a behaviour-tree key), that each tab was served the descriptor its header names, and that each browser context's store holds exactly one file per model whose bytes are the canonical JSON of that model's converged state. Nodes, Chrome and the preview server are killed however the run ends.

It needs the interpreted node binary (`moirai-model-plane/target/debug/examples/model_node`, or `MOIRAI_E2E_NODE_BIN`) and Chrome (`/usr/bin/google-chrome`, or `CHROME_BIN`); without either it prints an `E2E-SKIP` line and skips, the discipline the e2e suite uses. `npm test` follows the same rule for mp27 (a model opened on a node whose descriptor file was edited is refused, both pairs shown, editing held, nothing stored), mp28 (the store file equals the converged model, and a restart after the file is tampered trusts the log) and mp37 (two replicas each accept one half of a duplicate and both report it with editing left enabled), which run the sync path against two live nodes with the directory store and no browser. All three, and mp26 with them, currently skip on the header above: they need a model's identity to reach the editor from its registration rather than from the document.

### The edit gate, and why edits are held

Measured on the rig: the replica answers a single-character `POST /api/op` in ~400 ms, and one reorder is ~133 ops — so a structural edit can take the better part of a minute. Two consequences the UI has to be honest about.

**The wait is shown, not hidden.** A batch in flight has a determinate bar in the top bar, in the collapsed console summary, and at the head of the action log — because a log row is only appended when the whole batch resolves, and a panel that stays blank for a minute reads as a broken app.

**Structural edits wait for a quiet queue.** Add, remove, reorder, unset, delete and reference writes all compute their ops from the client's *current* view of the document — an index, an array length, or (for a reorder, since the wire has no move op) the whole child subtree that gets deleted and re-created. Issued while a batch is draining, they are computed from the replica's half-applied state. This was reproduced twice against a live rig, destroying elements while every log row reported `ok`. So `src/ui/editGate.ts` holds them until the queue is empty *and* one poll has read the replica back; value fields are held for the shorter window in which a structural batch is in flight (never for the user's own typing, which would lock a field mid-word). Every held control says why in its title, and the form carries a strip naming the batch and its progress.

This is a guard, not a cure. **The fix belongs on the wire** — a move op, so a reorder stops being delete + re-create — and is open work.

## Dependencies

`react`, `react-dom`, and `lucide-react` (pinned exact at 1.38.0: one package, zero transitive dependencies, no install hooks, no network at runtime, ISC). All icons are re-exported from `src/ui/icons.tsx`, so the set can be swapped for inline SVGs in one file if that audit ever sours. `puppeteer-core` is a devDependency for the harness only: it drives the Chrome already on the machine and downloads no browser. `npm audit`: 0 vulnerabilities.

Measured production bundle: **JS 299.4 kB raw / 90.9 kB gzip, CSS 29.8 kB / 5.8 kB gzip** (`dist` 344 kB). React and react-dom are ~68 kB gzip of that; lucide-react is ~2.8 kB for the glyphs used; the rest is this app.

## Out of scope (this phase)

- No diagram/graphical representation — typed tree/forms only.
- No access control or security.
- No eOpposite maintenance: setting one side of an opposite pair does not update the other.
- Reordering a collection element is delete + full re-create at the target index (no move op on the wire) — see the edit gate above.
- No keyed (`uw-map`) collections and no transparent classes: the encoder refuses an operation that would address one, naming the feature, rather than misrouting it.
- No way to empty a single-valued containment: the interpreted node has no operation that does it, so the control refuses naming the feature. An optional one unsets normally.
- No `__model` header on the interpreted path, so the binding check answers `unbound`, no projection file is written and no conformance report is produced — see the binding check above.
- The action log is per-browser-tab client state; it does not survive a reload and is not stored on the replica.
- No cluster-wide model catalog: a node lists what it hosts, and a model created elsewhere is joined by an id learned out of band, together with the metamodel it is bound to, which the node requires at registration.
