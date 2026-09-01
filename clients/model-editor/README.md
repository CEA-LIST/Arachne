# Model Editor

A metamodel-agnostic web editor for models hosted on a moirai replica. It talks only to the node's existing HTTP API, discovers the metamodel at load time, and shapes its UI accordingly — the same binary serves different metamodels, and the editor follows. No class name from any sample metamodel appears anywhere in `src/`.

## Run

The usual way is the Compose rig, which starts a whole cluster and publishes two replicas for browsers:

```
cd ../../../moirai-modelsward/docker && ./rig.sh --edit --no-load
npm install && npm run dev      # in this directory
```

Then open the printed URL and connect to `http://127.0.0.1:8081` (or `:8082` for the second replica — two browser windows on the two replicas is how convergence is demonstrated).

`editor-a` and `editor-b` join the same cluster as the scaled replicas and serve the baked-in metamodel descriptor on `/api/metamodel` (`METAMODEL_PATH` overrides which one). `--no-load` keeps the load driver's random writes out of the document during a session with people. `./rig.sh down` removes everything.

A single replica without Docker, when that is all you need (from `generated/json_crdt`):

```sh
cargo build --example network_node
REPLICA_ID=replica-a LISTEN_PORT=7101 HTTP_PORT=3101 \
METAMODEL_PATH=../../examples/bt.metamodel.json \
  ./target/debug/examples/network_node
```

Scripts in this directory:

```sh
npm run dev        # dev server
npm test           # unit tests (vitest)
npm run lint       # oxlint
npm run build      # type-check + production bundle in dist/
```

## The UI

A four-region modelling IDE layout; one light theme, print-worthy for paper figures (`@media print` drops the chrome, so print-to-PDF gives a clean figure).

- **Top bar** — app identity, then the document context read from the descriptor at runtime (`package`, and whether it came from the node or from a file). On the right: the batch in flight as a determinate bar, the sync chip, the replica id from `/api/health`, keyboard help, and Connect. **The node URL and poll interval live in the Connect popover**, not on the page.
- **Explorer** (left) — `Model` and `Metamodel` tabs. The model tab is an ARIA tree with structural type icons, indent guides, expand/collapse, a filter that highlights matches and keeps their ancestors, an element count, and per-row add/remove actions. The metamodel tab is a searchable class browser (supertypes, abstract marker, feature counts) and hosts the descriptor-file fallback.
- **Properties** (right) — the selected element: kind icon, label, eClass badge, a clickable path breadcrumb, Copy path and Delete element; then `Attributes`, `Containments` and `References` as collapsible sections in the descriptor's own feature order. Required features carry `*`, the id attribute carries a key icon, every attribute carries a type chip.
- **Console** (bottom, `⌘/Ctrl + J`) — collapsed to a bar that still reports the entry count, the newest line and a danger dot; open on `Action log` or `Document JSON`.

Every empty surface instructs rather than sitting blank: not connected, no metamodel, empty document, no filter match, nothing selected, no operations yet.

Keyboard: `↑ ↓` move, `→ ←` expand/collapse or step in and out, `Home`/`End`, `Enter` selects and jumps into the form, `Space` selects in place, type-ahead by name, `*` expands the level, `F2` jumps to the id field, `Delete` removes, `Esc` reverts the focused field to the last synced value, `⌘/Ctrl + K` filter, `⌘/Ctrl + J` console, `⌘/Ctrl + ⇧ + E` export the log, `?` for the full map.

## Metamodel discovery

On connect the editor calls `GET /api/metamodel`. The node answers with a formatVersion-1 descriptor (classes with attributes/containments/references, root classes, enums) that `arachne-codegen` emits as `metamodel.json` beside every generated crate; `network_node` serves the file named by `METAMODEL_PATH` (default `./metamodel.json`).

If the node answers 404 (a node without a descriptor), the Metamodel tab offers the labelled fallback: load a descriptor file produced by `arachne describe <file.ecore>`.

## Editing

The tree is the descriptor's containment structure (labels come from the element's id attribute — the first `isId` attribute, else one named `ID`/`name`, else the first string attribute — falling back to the class name). Selecting an element opens a typed form: text inputs for strings, number inputs (commit on blur/Enter) for int/float, a switch for booleans, a literal dropdown for enums. Containments offer create/add with a concrete-subtype menu where the target class is abstract, plus remove and move up/down; references are pickers over the document's existing instances of the target family, stored as the target's id value. On a fresh document the editor offers root creation from the descriptor's root classes.

Every action lands in the action log with its exact op payloads and the node's verdict. The log is append-only and has no clear button on purpose — it is the evidence. **It lives in the browser tab**: a reload starts a new one, so `Export JSON` (or `⌘⇧E`) is what makes it durable.

## How edits reach the wire

Every edit intent is mapped by `src/crdt/ops.ts` to a sequence of `JsonKind` ops posted **one at a time** (`POST /api/op` takes exactly one op — `moirai-network/src/http_api.rs`); string edits are diffed into at most one `DeleteRange` plus single-character `Insert`s (the wire accepts one character per op), numbers commit as a single relative `Inc`, array/containment creation uses the insert-then-update idiom with the mandatory `eClass` field first. A single global FIFO queue serializes all batches so sequences never interleave. The UI polls `GET /api/state` (500 ms, configurable); a field being typed in is never clobbered by a refresh (focus + 500 ms typing threshold, selection restored otherwise). Refused ops (`success:false`) and HTTP errors are surfaced three ways — at the field, in the alert dock, and in the log — and the log is exportable.

### The edit gate, and why edits are held

Measured on the rig: the replica answers a single-character `POST /api/op` in ~400 ms, and one reorder is ~133 ops — so a structural edit can take the better part of a minute. Two consequences the UI has to be honest about.

**The wait is shown, not hidden.** A batch in flight has a determinate bar in the top bar, in the collapsed console summary, and at the head of the action log — because a log row is only appended when the whole batch resolves, and a panel that stays blank for a minute reads as a broken app.

**Structural edits wait for a quiet queue.** Add, remove, reorder, unset, delete and reference writes all compute their ops from the client's *current* view of the document — an index, an array length, or (for a reorder, since the wire has no move op) the whole child subtree that gets deleted and re-created. Issued while a batch is draining, they are computed from the replica's half-applied state. This was reproduced twice against a live rig, destroying elements while every log row reported `ok`. So `src/ui/editGate.ts` holds them until the queue is empty *and* one poll has read the replica back; value fields are held for the shorter window in which a structural batch is in flight (never for the user's own typing, which would lock a field mid-word). Every held control says why in its title, and the form carries a strip naming the batch and its progress.

This is a guard, not a cure. **The fix belongs on the wire** — a move op, so a reorder stops being delete + re-create — and is open work.

## Dependencies

`react`, `react-dom`, and `lucide-react` (pinned exact at 1.38.0: one package, zero transitive dependencies, no install hooks, no network at runtime, ISC). All icons are re-exported from `src/ui/icons.tsx`, so the set can be swapped for inline SVGs in one file if that audit ever sours. `npm audit`: 0 vulnerabilities.

Measured production bundle: **JS 273.6 kB raw / 83.6 kB gzip, CSS 27.5 kB / 5.5 kB gzip** (`dist` 302 kB). React and react-dom are ~68 kB gzip of that; lucide-react is ~2.7 kB for the 30 glyphs used; the rest is this app.

## Out of scope (this phase)

- No diagram/graphical representation — typed tree/forms only.
- No access control or security.
- No eOpposite maintenance: setting one side of an opposite pair does not update the other.
- Reordering a collection element is delete + full re-create at the target index (no move op on the wire) — see the edit gate above.
- `Object.Remove` resets a key to its type default rather than deleting it; an object slot counts as present only while its `eClass` is non-empty.
- The action log is per-tab client state; it does not survive a reload and is not stored on the replica.
