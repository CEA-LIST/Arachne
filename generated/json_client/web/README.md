# Experiment Editor Web Application

A collaborative web application for editing experiment data using the JSON CRDT backend.

## Features

- **Real-time collaborative editing** - Multiple users can edit the same document simultaneously
- **Auto-refresh** - Automatically syncs changes from other users every 500ms
- **Concurrent string editing** - Long text fields (description, notes) support concurrent edits with CRDT merge
- **Visual feedback** - Highlights fields when they change from remote edits
- **Auto-save** - Changes are automatically saved as you type (debounced)
- **Connection status** - Shows connection health and last sync time

## Quick Start

### 1. Start the JSON CRDT node

```bash
cd atraktos/generated/json_crdt
REPLICA_ID=a LISTEN_PORT=9001 HTTP_PORT=8081 cargo run --example network_node
```

### 2. Handle CORS (required for local development)

The node needs to accept browser requests. You have two options:

**Option A: Use a CORS proxy (simplest for testing)**

```bash
# Install cors-anywhere or similar
npx cors-anywhere
```

Then update `API_BASE` in `experiment_editor.html` to use the proxy.

**Option B: Add CORS headers to the node (recommended)**

The node already serves HTTP, but you may need to enable CORS. If you see CORS errors in the browser console, you'll need to modify the HTTP server in `moirai-network` to add CORS headers.

**Option C: Serve from the same origin**

Serve the HTML file through the node's HTTP server (requires adding a static file handler).

### 3. Open the web application

```bash
cd atraktos/generated/json_crdt/web
python3 -m http.server 8080
```

Then open in your browser:
```
http://localhost:8080/experiment_editor.html
```

**Important**: If the API is on port 8081, you may encounter CORS issues. See the CORS section below.

## Usage

### Fields

- **Experiment ID**: Unique identifier for the experiment
- **Version**: Integer version number
- **Timestamp**: ISO 8601 timestamp (e.g., `2026-05-11T10:30:00Z`)
- **Description**: Long-form text, supports concurrent editing
- **Notes**: Extended documentation, supports concurrent editing

### Operations

- **Type in any field**: Changes auto-save after 500ms of inactivity
- **Auto-refresh toggle**: Enable/disable automatic syncing from other users
- **Refresh Now button**: Manually pull latest state
- **Clear All button**: Deletes all data (confirms before executing)

### Collaborative Editing

1. Open the app in multiple browser windows/tabs or on different machines
2. Edit text in one window
3. Watch changes appear in the other windows automatically
4. Concurrent edits to the same field will merge using CRDT semantics

## CORS Configuration

If you encounter CORS errors, modify the HTTP server in `moirai-network/src/generic.rs`:

Add CORS headers to the HTTP response:

```rust
use actix_cors::Cors;

// In the HTTP server setup:
HttpServer::new(move || {
    App::new()
        .wrap(
            Cors::default()
                .allow_any_origin()
                .allow_any_method()
                .allow_any_header()
        )
        // ... rest of routes
})
```

Add to `Cargo.toml`:
```toml
actix-cors = "0.7"
```

## Architecture

```
Browser (experiment_editor.html)
    |
    | POST /api/op (CRDT operations)
    | GET /api/state (current state)
    |
    v
JSON CRDT Node (port 8081)
    |
    | CRDT operations
    |
    v
Replica State (in-memory CRDT)
```

## Concurrent Editing Details

### String Fields (description, notes)

The app calculates minimal character-level diffs and sends CRDT operations:

```javascript
// User types "Hello" -> "Hello World"
// Sends operations:
Insert 'W' at pos 6
Insert 'o' at pos 7
Insert 'r' at pos 8
Insert 'l' at pos 9
Insert 'd' at pos 10
```

When another user edits concurrently, the CRDT ensures both changes merge correctly.

### Simple Fields (experiment_id, timestamp, version)

These use last-writer-wins or counter semantics:
- Strings: replaced character-by-character
- Numbers: incremented
- Changes sync on blur or 500ms after typing stops

## Troubleshooting

### "Failed to save changes"
- Check that the node is running on port 8081
- Verify CORS is configured correctly
- Check browser console for specific errors

### Changes not appearing
- Ensure auto-refresh is enabled
- Check the "Last sync" time in the status bar
- Click "Refresh Now" to manually sync

### Cursor jumps while typing
- This shouldn't happen - the app only updates fields that aren't currently focused
- If it does, there may be a race condition; stop auto-refresh while typing

## Advanced: Multi-Node Setup

To test true distributed editing:

```bash
# Terminal 1 - Node A
REPLICA_ID=a LISTEN_PORT=9001 HTTP_PORT=8081 PEERS=b:localhost:9002 cargo run --example network_node

# Terminal 2 - Node B  
REPLICA_ID=b LISTEN_PORT=9002 HTTP_PORT=8082 PEERS=a:localhost:9001 cargo run --example network_node
```

Now you can:
- Point one browser to `localhost:8081` 
- Point another to `localhost:8082`
- Edit simultaneously and watch CRDT convergence in action
