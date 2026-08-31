/**
 * The containment tree, shaped by the discovered metamodel: one row per
 * element (label + eClass badge), one collapsible group per containment
 * feature. Selection drives the element form.
 */

import type { ModelNode } from '../model/instance';
import { pathKey } from '../crdt/path';

interface ModelTreeProps {
  root: ModelNode;
  selectedKey: string;
  onSelect: (node: ModelNode) => void;
}

export function ModelTree({ root, selectedKey, onSelect }: ModelTreeProps) {
  return (
    <div className="model-tree">
      <TreeNodeRow node={root} selectedKey={selectedKey} onSelect={onSelect} />
    </div>
  );
}

interface TreeNodeRowProps {
  node: ModelNode;
  selectedKey: string;
  onSelect: (node: ModelNode) => void;
}

function TreeNodeRow({ node, selectedKey, onSelect }: TreeNodeRowProps) {
  const key = pathKey(node.path);
  const nonEmpty = node.features.filter((f) => f.children.length > 0);
  return (
    <div className="tree-node">
      <button
        type="button"
        className={key === selectedKey ? 'tree-row selected' : 'tree-row'}
        onClick={() => onSelect(node)}
      >
        <span className="tree-label">{node.label}</span>
        {node.eClass !== '' && node.label !== node.eClass && (
          <span className="tree-eclass">{node.eClass}</span>
        )}
      </button>
      {nonEmpty.length > 0 && (
        <div className="tree-children">
          {nonEmpty.map((feature) => (
            <details key={feature.desc.name} open>
              <summary className="tree-feature">
                {feature.desc.name}
                {feature.desc.many && <span className="muted"> ({feature.children.length})</span>}
              </summary>
              <div className="tree-feature-children">
                {feature.children.map((child) => (
                  <TreeNodeRow
                    key={pathKey(child.path)}
                    node={child}
                    selectedKey={selectedKey}
                    onSelect={onSelect}
                  />
                ))}
              </div>
            </details>
          ))}
        </div>
      )}
    </div>
  );
}
