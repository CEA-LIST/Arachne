/**
 * The decoded document as raw JSON — available with or without a metamodel,
 * which makes it the fallback inspector when a replica serves no descriptor.
 */

import type { PlainJson } from '../api/types';

interface DocumentJsonViewProps {
  doc: PlainJson;
  connected: boolean;
}

export function DocumentJsonView({ doc, connected }: DocumentJsonViewProps) {
  if (!connected) {
    return <p className="me-log__empty">Connect to a replica to inspect its state.</p>;
  }
  if (doc === null) {
    return (
      <p className="me-log__empty">
        Document is empty — the wire state is <code>Unset</code>, so no operation has ever been
        applied to this replica.
      </p>
    );
  }
  return (
    <pre className="me-json" data-testid="raw-inspector">
      {JSON.stringify(doc, null, 2)}
    </pre>
  );
}
