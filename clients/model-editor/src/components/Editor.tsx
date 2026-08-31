/**
 * The metamodel-aware editor: containment tree on the left, typed element
 * form on the right. With no root yet (fresh replica), offers root creation
 * from the descriptor's concrete root family.
 */

import { useState } from 'react';
import type { Descriptor, Path, PlainJson } from '../api/types';
import { getAtPath, pathKey } from '../crdt/path';
import { createRootOps } from '../crdt/ops';
import { buildTree, isPresent, rootCandidates } from '../model/instance';
import type { FieldRegistry } from '../sync/fieldRegistry';
import { AddControl, ElementForm } from './ElementForm';
import { ModelTree } from './ModelTree';
import type { SendOps } from './fields';

interface EditorProps {
  descriptor: Descriptor | null;
  doc: PlainJson;
  connected: boolean;
  registry: FieldRegistry;
  sendOps: SendOps;
}

export function Editor({ descriptor, doc, connected, registry, sendOps }: EditorProps) {
  const [selectedPath, setSelectedPath] = useState<Path>([]);

  // If the selected element vanished (removed, reordered away), fall back to
  // the closest present ancestor — derived at render time, no effect needed.
  let effectivePath = selectedPath;
  while (effectivePath.length > 0 && !isPresent(getAtPath(doc, effectivePath))) {
    effectivePath = effectivePath.slice(0, -1);
  }

  if (!connected) {
    return <p className="muted">Connect to a node to edit its model.</p>;
  }
  if (descriptor === null) {
    return (
      <p className="muted">
        No metamodel: this node serves no descriptor. Load one in the Metamodel tab to edit.
      </p>
    );
  }

  const tree = buildTree(descriptor, doc);
  if (tree === null) {
    const candidates = rootCandidates(descriptor);
    return (
      <div className="editor-empty">
        <p>The document is empty. Create the model root:</p>
        <AddControl
          options={candidates}
          verb="Create root"
          onAdd={(cls) =>
            void sendOps(`create root ${cls}`, createRootOps(cls), { path: [], value: { eClass: cls } })
          }
        />
      </div>
    );
  }

  return (
    <div className="editor">
      <aside className="editor-tree">
        <ModelTree root={tree} selectedKey={pathKey(effectivePath)} onSelect={(n) => setSelectedPath(n.path)} />
      </aside>
      <div className="editor-form">
        <ElementForm
          descriptor={descriptor}
          doc={doc}
          path={effectivePath}
          registry={registry}
          sendOps={sendOps}
          onSelectPath={setSelectedPath}
        />
      </div>
    </div>
  );
}
