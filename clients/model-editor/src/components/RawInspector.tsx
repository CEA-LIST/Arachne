import type { PlainJson } from '../api/types';

interface RawInspectorProps {
  doc: PlainJson;
  connected: boolean;
}

/**
 * Always-available view of the decoded document — works with or without a
 * metamodel. A fresh replica ("Unset" on the wire) shows as an empty document.
 */
export function RawInspector({ doc, connected }: RawInspectorProps) {
  if (!connected) {
    return <p className="muted">Connect to a node to inspect its state.</p>;
  }
  if (doc === null) {
    return <p className="muted">Document is empty (state "Unset" — no operation applied yet).</p>;
  }
  return (
    <pre className="raw-inspector" data-testid="raw-inspector">
      {JSON.stringify(doc, null, 2)}
    </pre>
  );
}
