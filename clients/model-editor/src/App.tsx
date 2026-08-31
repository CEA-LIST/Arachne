import { useState } from 'react';
import './App.css';
import { ActionLog } from './components/ActionLog';
import { ConnectPanel } from './components/ConnectPanel';
import { ErrorBanner } from './components/ErrorBanner';
import { MetamodelPanel } from './components/MetamodelPanel';
import { RawInspector } from './components/RawInspector';
import { SyncStatus } from './components/SyncStatus';
import { useSync } from './sync/useSync';

type Tab = 'editor' | 'metamodel' | 'inspector';

export default function App() {
  const sync = useSync();
  const [tab, setTab] = useState<Tab>('inspector');
  const { state } = sync;
  const connected = state.connection.status === 'connected';

  return (
    <div className="app">
      <header className="app-header">
        <h1>Model Editor</h1>
        <ConnectPanel
          connection={state.connection}
          pollMs={sync.pollMs}
          setPollMs={sync.setPollMs}
          setUrl={sync.setUrl}
          connect={() => void sync.connect()}
          disconnect={sync.disconnect}
        />
        <SyncStatus state={state} />
      </header>
      <ErrorBanner message={state.banner} onDismiss={sync.clearBanner} />
      <nav className="tabs">
        {(['editor', 'metamodel', 'inspector'] as const).map((t) => (
          <button
            key={t}
            type="button"
            className={tab === t ? 'tab active' : 'tab'}
            onClick={() => setTab(t)}
          >
            {t === 'editor' ? 'Editor' : t === 'metamodel' ? 'Metamodel' : 'State inspector'}
          </button>
        ))}
      </nav>
      <main className="app-main">
        {tab === 'editor' && (
          <p className="muted">
            The typed model editor (tree + forms shaped by the discovered metamodel) plugs in here.
          </p>
        )}
        {tab === 'metamodel' && (
          <MetamodelPanel
            metamodel={state.metamodel}
            source={state.metamodelSource}
            connected={connected}
            loadDescriptorFile={sync.loadDescriptorFile}
          />
        )}
        {tab === 'inspector' && <RawInspector doc={state.doc} connected={connected} />}
      </main>
      <footer className="app-footer">
        <ActionLog log={state.log} />
      </footer>
    </div>
  );
}
