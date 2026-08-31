# Model Editor

A metamodel-agnostic web editor for models hosted on a moirai replica. It talks only to the node's existing HTTP API, discovers the metamodel at load time, and shapes its UI accordingly — the same binary serves different metamodels, and the editor follows.

## Run

Start a replica that serves a metamodel descriptor (from `generated/json_crdt`):

```sh
cargo build --example network_node
REPLICA_ID=replica-a LISTEN_PORT=7101 HTTP_PORT=3101 \
METAMODEL_PATH=../../examples/bt.metamodel.json \
  ./target/debug/examples/network_node
```

Then, in this directory:

```sh
npm install
npm run dev        # dev server; open the printed URL
npm test           # unit tests (vitest)
npm run build      # type-check + production bundle in dist/
```

Enter the node's HTTP URL (e.g. `http://127.0.0.1:3101`) in the connect panel.

## Metamodel discovery

On connect the editor calls `GET /api/metamodel`. The node answers with a formatVersion-1 descriptor (classes with attributes/containments/references, root classes, enums) that `arachne-codegen` emits as `metamodel.json` beside every generated crate; `network_node` serves the file named by `METAMODEL_PATH` (default `./metamodel.json`).

If the node answers 404 (a node without a descriptor), the Metamodel tab offers the labelled fallback: load a descriptor file produced by `arachne describe <file.ecore>`.

## Editing

The Editor tab renders the model as a containment tree shaped by the descriptor (labels come from the element's id attribute — the first `isId` attribute, else one named `ID`/`name`, else the first string attribute — falling back to the class name). Selecting an element shows a typed form: text inputs for strings, number inputs (commit on blur/Enter) for int/float, checkboxes for booleans, literal dropdowns for enums; containments offer create/add with a concrete-subtype menu where the target class is abstract, plus remove and move up/down; references are pickers over the document's existing instances of the target family, stored as the target's id value. On a fresh document the editor offers root creation from the descriptor's root classes. Every action lands in the action log with its exact op payloads and the node's verdict.

## How edits reach the wire

Every edit intent is mapped by `src/crdt/ops.ts` to a sequence of `JsonKind` ops posted one at a time (`POST /api/op`); string edits are diffed into at most one `DeleteRange` plus single-character `Insert`s (the wire accepts one character per op), numbers commit as a single relative `Inc`, array/containment creation uses the insert-then-update idiom with the mandatory `eClass` field first. A single global FIFO queue serializes all batches so sequences never interleave. The UI polls `GET /api/state` (500 ms, configurable); a field being typed in is never clobbered by a refresh (focus + 500 ms typing threshold, selection restored otherwise). Refused ops (`success:false`) and HTTP errors are shown in the error banner and recorded in the exportable action log.

## Out of scope (this phase)

- No diagram/graphical representation — typed tree/forms only.
- No access control or security.
- No eOpposite maintenance: setting one side of an opposite pair does not update the other.
- Reordering a collection element is delete + full re-create at the target index (no move op on the wire).
- `Object.Remove` resets a key to its type default rather than deleting it; an object slot counts as present only while its `eClass` is non-empty.
