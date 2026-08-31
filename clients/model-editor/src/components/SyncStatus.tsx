import type { AppState } from '../state/store';

interface SyncStatusProps {
  state: AppState;
}

export function SyncStatus({ state }: SyncStatusProps) {
  const { connection, lastSyncAt, pendingOps, metamodel, metamodelSource } = state;
  return (
    <div className="sync-status">
      <span className={`status-dot status-${connection.status}`} aria-hidden="true" />
      <span>{connection.status}</span>
      {connection.replicaId !== null && <span>replica {connection.replicaId}</span>}
      {lastSyncAt !== null && (
        <span>last sync {new Date(lastSyncAt).toLocaleTimeString()}</span>
      )}
      <span>{pendingOps} pending op{pendingOps === 1 ? '' : 's'}</span>
      <span>
        {metamodel !== null
          ? `metamodel: ${metamodel.package} (${metamodelSource === 'node' ? 'from node' : 'from file'})`
          : 'no metamodel'}
      </span>
    </div>
  );
}
