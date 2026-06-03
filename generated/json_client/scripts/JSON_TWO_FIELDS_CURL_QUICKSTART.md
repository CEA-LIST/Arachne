# JSON CRDT Quickstart: Create Two Fields via curl

This quickstart shows the smallest end-to-end flow for the generated `json_crdt` project.

Target JSON state:

```json
{
  "name": "Bob",
  "age": 30
}
```

## 1. Start one node

From `atraktos/generated/json_crdt`:

```bash
REPLICA_ID=a LISTEN_PORT=9001 HTTP_PORT=8081 cargo run --example network_node
```

## 2. Optional health check

```bash
curl http://localhost:8081/api/health
```

## 3. Create `name` and insert first character

`name` is a JSON string backed by a char-level CRDT (`List<char>`), so characters are inserted one by one.

```bash
curl -X POST http://localhost:8081/api/op \
  -H 'Content-Type: application/json' \
  -d '{"JsonKind":{"Object":{"Update":["name",{"String":{"Insert":{"content":"B","pos":0}}}]}}}'
```

## 4. Complete `name = "Bob"`

```bash
curl -X POST http://localhost:8081/api/op \
  -H 'Content-Type: application/json' \
  -d '{"JsonKind":{"Object":{"Update":["name",{"String":{"Insert":{"content":"o","pos":1}}}]}}}'

curl -X POST http://localhost:8081/api/op \
  -H 'Content-Type: application/json' \
  -d '{"JsonKind":{"Object":{"Update":["name",{"String":{"Insert":{"content":"b","pos":2}}}]}}}'
```

## 5. Add `age = 30`

`age` uses the counter CRDT (`Number`), so this is a single operation.

```bash
curl -X POST http://localhost:8081/api/op \
  -H 'Content-Type: application/json' \
  -d '{"JsonKind":{"Object":{"Update":["age",{"Number":{"Inc":30}}]}}}'
```

## 6. Read current state

```bash
curl -s http://localhost:8081/api/state | jq .
```

You should see a state equivalent to JSON with two fields: `name = "Bob"` and `age = 30`.

---

## Alternative: Using CLI Helper Scripts

The generated `json_crdt` project includes CLI helper scripts for easier manipulation:

```bash
cd atraktos/generated/json_crdt

# Add fields using helper scripts
./scripts/json_add.py name '"Bob"'
./scripts/json_add.py age 30

# Get plain JSON state
./scripts/state_plain.py
```

See `atraktos/generated/json_crdt/scripts/README.md` for the complete CLI reference including:
- `json_add.py` — add fields and array elements
- `json_delete.py` — delete fields and array elements
- `json_update.py` — update field values
- `json_string.py` — string operations (append, insert, delete, replace, clear)
- `init_from_json.py` — initialize from a full JSON file
- `state_plain.py` — decode CRDT state into plain JSON
