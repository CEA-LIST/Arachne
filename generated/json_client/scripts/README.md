# json_crdt helper scripts

## Quick Reference

| Script | Purpose |
|--------|---------|
| `state_plain.py` | Get plain JSON state (decode CRDT wrappers) |
| `init_from_json.py` | Initialize from a full JSON file |
| `json_add.py` | Add a field or array element |
| `json_delete.py` | Delete a field or array element |
| `json_update.py` | Update a field value |
| `json_string.py` | String operations (append, insert, delete, replace, clear) |

---

## 1) Get plain JSON state

```bash
cd atraktos/generated/json_crdt
./scripts/state_plain.py --url http://localhost:8081/api/state
```

Default URL is already `http://localhost:8081/api/state`, so this also works:

```bash
./scripts/state_plain.py
```

---

## 2) Initialize from a full JSON file

```bash
cd atraktos/generated/json_crdt
./scripts/init_from_json.py /path/to/input.json --url http://localhost:8081
```

Optional flags:

- `--clear-root`: clears root object before loading
- `--dry-run`: prints generated `/api/op` payloads without sending

Example:

```bash
./scripts/init_from_json.py ../../data/sample.json --url http://localhost:8081 --clear-root
```

---

## 3) Add a field or array element

Add a string field:
```bash
./scripts/json_add.py user.name '"Alice"'
```

Add a number field:
```bash
./scripts/json_add.py user.age 25
```

Add a boolean field:
```bash
./scripts/json_add.py user.active true
```

Add an element to an array at position 0:
```bash
./scripts/json_add.py items --array-pos 0 '"item1"'
```

Add an element at position 2:
```bash
./scripts/json_add.py tags --array-pos 2 '"rust"'
```

---

## 4) Delete a field or array element

Delete a field:
```bash
./scripts/json_delete.py user.name
```

Delete an array element at position 1:
```bash
./scripts/json_delete.py items[1]
```

Or using the flag:
```bash
./scripts/json_delete.py items --array-pos 1
```

---

## 5) Update a field value

Update a string (replaces with new value):
```bash
./scripts/json_update.py user.name '"Bob"'
```

Update a number (increments):
```bash
./scripts/json_update.py user.age 30
```

Update a boolean:
```bash
./scripts/json_update.py user.active false
```

---

## 6) String-specific operations

### Append text to a string
```bash
./scripts/json_string.py append user.name ' Smith'
```

### Insert text at a position
```bash
./scripts/json_string.py insert user.name 'Mr. ' --pos 0
```

### Delete a single character
```bash
./scripts/json_string.py delete user.name --pos 0
```

### Delete a range of characters
```bash
./scripts/json_string.py delete user.name --pos 0 --len 4
```

### Replace text (delete + insert)
```bash
./scripts/json_string.py replace user.name 'Alice' --pos 0 --len 3
```

### Clear entire string
```bash
./scripts/json_string.py clear user.name
```

---

## Complete Workflow Example

Start the node:
```bash
cd atraktos/generated/json_crdt
REPLICA_ID=a LISTEN_PORT=9001 HTTP_PORT=8081 cargo run --example network_node &
```

Build a JSON object step by step:
```bash
# Add user fields
./scripts/json_add.py user.name '"Alice"'
./scripts/json_add.py user.age 30
./scripts/json_add.py user.active true

# Add nested address
./scripts/json_add.py user.address.city '"Portland"'
./scripts/json_add.py user.address.zip '"97201"'

# Add array of tags
./scripts/json_add.py tags --array-pos 0 '"developer"'
./scripts/json_add.py tags --array-pos 1 '"rust"'

# Modify a string
./scripts/json_string.py append user.name ' Smith'

# Check result
./scripts/state_plain.py
```

Result:
```json
{
  "user": {
    "name": "Alice Smith",
    "age": 30,
    "active": true,
    "address": {
      "city": "Portland",
      "zip": "97201"
    }
  },
  "tags": ["developer", "rust"]
}
```

---

## Notes

- Supports objects, arrays, strings, numbers, booleans.
- `null` values are skipped during initialization.
- Empty strings are represented by an insert-then-delete sequence.
- All scripts default to `http://localhost:8081` but accept `--url` to override.
- Paths use dot notation: `user.profile.name` or `items[0].title`
