# JSON → JSON CRDT Type Mapping

How standard JSON values are represented as conflict-free replicated data types.

## Type Mapping

| JSON type | CRDT type | Log type | Semantics |
|-----------|-----------|----------|-----------|
| `"hello"` | `List<char>` | `EventGraph<List<char>>` | Ordered sequence of characters with positional insert/delete (Yjs-style EGWalker) |
| `42` | `Counter<f64>` | `VecLog<Counter<f64>>` | Resettable increment/decrement counter |
| `true` / `false` | `EWFlag` | `VecLog<EWFlag>` | Enable-Wins flag (concurrent enable + disable → enabled) |
| `{ … }` | `UWMap<String, Box<Json>>` | `UWMapLog<String, JsonLog>` | Update-Wins map (concurrent update + remove → value kept) |
| `[ … ]` | `NestedList<Box<Json>>` | `NestedListLog<JsonLog>` | Ordered list of nested CRDT elements with positional addressing |

## Union definition

From `moirai-crdt/src/json/mod.rs`:

```rust
union! {
    Json = Number(Counter<f64>,              VecLog<Counter<f64>>)
         | Boolean(EWFlag,                   VecLog<EWFlag>)
         | String(List<char>,                EventGraph<List<char>>)
         | Object(UWMap<String, Box<Json>>,  UWMapLog<String, JsonLog>)
         | Array(NestedList<Box<Json>>,      NestedListLog<JsonLog>)
}
```

The `union!` macro generates:

- **`Json`** — the operation enum (variants: `Number`, `Boolean`, `String`, `Object`, `Array`)
- **`JsonLog`** — the log type that stores per-variant state
- **`JsonValue`** — the evaluated value type

## Operations per type

### String — `List<char>` (EGWalker)

```text
Insert { content: char, pos: usize }   — insert character at position
Delete { pos: usize }                  — delete character at position
DeleteRange { start: usize, len: usize } — delete a range
```

One operation per character. `"Alice"` → 5 ops.

Concurrent inserts at the same position are resolved by the EGWalker (Yjs/Fugue) algorithm, preserving user intent.

### Number — `Counter<f64>`

```text
Inc(value)  — increment by value
Dec(value)  — decrement by value
Reset       — reset to 0
```

Concurrent increments are **additive**: replica A does `Inc(5)`, replica B does `Inc(3)` → result is `8`.

### Boolean — `EWFlag`

```text
Enable   — set to true
Disable  — set to false
Clear    — remove the flag
```

Enable-Wins: if replica A enables and replica B disables concurrently, the flag is **true**.

### Object — `UWMap<String, Box<Json>>`

```text
Update(key, nested_op)  — insert or update a field
Remove(key)             — remove a field
Clear                   — remove all fields
```

Update-Wins: if replica A updates key `"x"` and replica B removes `"x"` concurrently, the **update wins** and the key is preserved.

Nested operations are wrapped recursively:

```rust
// Set root.user.name = insert 'A' at pos 0
Json::Object(UWMap::Update(
    "user",
    Box::new(Json::Object(UWMap::Update(
        "name",
        Box::new(Json::String(EGList::Insert { content: 'A', pos: 0 }))
    )))
))
```

### Array — `NestedList<Box<Json>>`

```text
Insert { pos, value: Json_op }  — insert element at position with initial op
Update { pos, value: Json_op }  — apply op to existing element
Delete { pos }                  — remove element at position
```

Elements are full `Json` CRDTs, so each array slot can be a string, number, boolean, object, or nested array.

## Example: full document

Given this JSON:

```json
{
  "name": "Alice",
  "age": 30,
  "active": true,
  "tags": ["admin", "dev"]
}
```

### Operations generated

| # | Operation | Type |
|---|-----------|------|
| 1–5 | `Object(Update("name", String(Insert('A',0))))` … `Insert('e',4)` | String chars |
| 6 | `Object(Update("age", Number(Inc(30))))` | Counter |
| 7 | `Object(Update("active", Boolean(Enable)))` | Flag |
| 8 | `Object(Update("tags", Array(Insert{pos:0, Number(Inc(0))})))` | Array init |
| 9 | `Object(Update("tags", Array(Delete{pos:0})))` | Array clear placeholder |
| 10 | `Object(Update("tags", Array(Insert{pos:0, String(Insert('a',0))})))` | 1st element |
| 11–14 | `Object(Update("tags", Array(Update{pos:0, String(Insert(…))})))` | "dmin" chars |
| 15 | `Object(Update("tags", Array(Insert{pos:1, String(Insert('d',0))})))` | 2nd element |
| 16–17 | `Object(Update("tags", Array(Update{pos:1, String(Insert(…))})))` | "ev" chars |

**Total: 17 operations**, each independently replicable.

## Conflict resolution summary

| Scenario | Winner | Rationale |
|----------|--------|-----------|
| Two replicas insert at same string position | Both kept, deterministic order | EGWalker (Yjs algorithm) |
| Two replicas set different values for same key | Both updates applied | UWMap keeps all concurrent updates |
| One replica updates key, other removes it | Update wins | Update-Wins Map semantics |
| One replica enables flag, other disables | Enabled | Enable-Wins Flag semantics |
| Two replicas increment counter | Sum of both | Counter is additive |
| Two replicas insert at same array index | Both kept, deterministic order | NestedList uses EGWalker |

## Limitations

- **Null** — no CRDT backing; silently skipped during load.
- **Floating-point precision** — numbers are stored as `f64` counters but loaded via `isize` cast in the legacy helper; the `json_crdt` loader uses `f64` natively.
- **Object nesting** — the CRDT and the `json_crdt` load helpers are fully recursive (unbounded depth). Only the legacy `moirai-network` loader was limited to 2 levels.
- **Arrays of arrays** — the CRDT supports it; the load helpers handle arrays containing objects but not arrays containing arrays yet.

## See also

- [JSON_LOAD_MAPPING.md](JSON_LOAD_MAPPING.md) — detailed operation sequences during `load`
