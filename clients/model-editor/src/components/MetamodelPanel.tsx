import { useState } from 'react';
import { validateDescriptor } from '../api/client';
import type { Descriptor } from '../api/types';

interface MetamodelPanelProps {
  metamodel: Descriptor | null;
  source: 'node' | 'file' | null;
  connected: boolean;
  loadDescriptorFile: (descriptor: Descriptor) => void;
}

/**
 * Shows the discovered metamodel. When the node serves none (GET
 * /api/metamodel → 404), offers the clearly-labelled degraded mode: load a
 * descriptor file produced by `arachne describe <file.ecore>`.
 */
export function MetamodelPanel({ metamodel, source, connected, loadDescriptorFile }: MetamodelPanelProps) {
  const [fileError, setFileError] = useState<string | null>(null);

  const onFile = (file: File | undefined) => {
    if (file === undefined) return;
    setFileError(null);
    file
      .text()
      .then((text) => loadDescriptorFile(validateDescriptor(JSON.parse(text))))
      .catch((err) => setFileError(err instanceof Error ? err.message : String(err)));
  };

  if (metamodel === null) {
    return (
      <div className="metamodel-panel">
        {connected ? (
          <>
            <p className="muted">
              This node serves no metamodel descriptor (GET /api/metamodel returned 404).
            </p>
            <p>
              Fallback: load a descriptor file (produced by <code>arachne describe &lt;file.ecore&gt;</code>):
            </p>
            <input
              type="file"
              accept=".json,application/json"
              onChange={(e) => onFile(e.target.files?.[0])}
            />
            {fileError !== null && <p className="connect-error">descriptor rejected: {fileError}</p>}
          </>
        ) : (
          <p className="muted">Connect to a node to discover its metamodel.</p>
        )}
      </div>
    );
  }

  return (
    <div className="metamodel-panel">
      <table className="meta-summary">
        <tbody>
          <tr>
            <th>package</th>
            <td>{metamodel.package}</td>
          </tr>
          <tr>
            <th>nsURI</th>
            <td>{metamodel.nsURI}</td>
          </tr>
          <tr>
            <th>root classes</th>
            <td>{metamodel.rootClasses.join(', ')}</td>
          </tr>
          <tr>
            <th>source</th>
            <td>{source === 'node' ? 'served by the node (/api/metamodel)' : 'loaded from file'}</td>
          </tr>
        </tbody>
      </table>
      <h3>Classes ({Object.keys(metamodel.classes).length})</h3>
      <ul className="class-list">
        {Object.entries(metamodel.classes).map(([name, cls]) => (
          <li key={name}>
            <strong>{name}</strong>
            {cls.abstract && <em> (abstract)</em>}
            {cls.superTypes.length > 0 && <span> : {cls.superTypes.join(', ')}</span>}
            <span className="muted">
              {' '}
              — {cls.attributes.length} attr, {cls.containments.length} containment
              {cls.containments.length === 1 ? '' : 's'}, {cls.references.length} ref
              {cls.references.length === 1 ? '' : 's'}
            </span>
          </li>
        ))}
      </ul>
      {Object.keys(metamodel.enums).length > 0 && (
        <>
          <h3>Enums</h3>
          <ul className="class-list">
            {Object.entries(metamodel.enums).map(([name, literals]) => (
              <li key={name}>
                <strong>{name}</strong>: {literals.join(' | ')}
              </li>
            ))}
          </ul>
        </>
      )}
    </div>
  );
}
