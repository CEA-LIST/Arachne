# Generic Network Layer

## Overview

The generic network layer (`moirai_network::generic::GenericNode`) may be used by any Arachne-generated project (or hand-written
CRDT that implements `IsLog`). 

## Architecture

```
  .ecore model
      │      
  arachne-cli generate
      │      
  ┌──────────────────────────┐
  │  Generated crate         │
  │  - Behaviortree (Op)     │ <- Serialize + Deserialize + Clone + Send
  │  - BehaviortreeLog       │ <- IsLog<Op = Behaviortree>
  └──────────┬───────────────┘
             │             
  ┌──────────────────────────┐
  │  GenericNode<L>          │
  │  - Replica<L, Tcsb<Op>>  │  one replica for each domain
  │  - TcpTransport<Op>      │  generic TCP, newline-delimited JSON
  │  - HTTP API (/api/op)    │  accepts serialized Op as JSON POST
  └──────────────────────────┘
```

## Quick Start

### 1. Generate a project

```bash
cd atraktos
cargo run -p arachne-cli -- generate examples/bt.ecore \
    -o ./generated/my_project \
    -p my_project \
    -m /path/to/Moirai
```

This automatically generates `examples/network_node.rs` — a complete network replica binary using `GenericNode<YourLog>`.

### 2. Run two replicas

The generated project includes a ready-to-use `network_node` example:

```bash
# Terminal 1
REPLICA_ID=a LISTEN_PORT=9001 HTTP_PORT=3001 PEERS=b:localhost:9002 cargo run --example network_node

# Terminal 2
REPLICA_ID=b LISTEN_PORT=9002 HTTP_PORT=3002 PEERS=a:localhost:9001 cargo run --example network_node
```

To use with more replicas, it is necessary to add all the ppers in the PEERS variable, and set up the right ports.

This was a generated network node, but an application specific node may be implemented as well.

The tcp_transport.rs implements TCP socket communication between peers.

## Creating and Editing Elements via HTTP

The HTTP API exposes a single endpoint: **`POST /api/op`**

The body is the JSON-serialized operation enum (Serde). The operation types mirror the metamodel structure exactly.

It uses server-> client communication via HTTP. It could be refactored so other communication protocols could be used, such as Websockets or HttpStream.


### Operation Structure

The generated `Behaviortree` enum (from `bt.ecore`) has this format:

```
Behaviortree
├── Root(Root)                      # operations on the root record
│   ├── Behaviortrees(NestedList)   # list of BehaviorTree instances
│   │   ├── Insert { pos, op }
│   │   ├── Update { pos, op }
│   │   └── Delete { pos }
│   ├── Main(BehaviorTree)          # single main BehaviorTree
│   └── New                         # initialize the root
├── AddReference(Refs)              # cross-reference arcs
└── RemoveReference(Refs)
```

Note that this is the Operation structure to be sent, not the metamodel written in Rust. 

### Examples

#### Create a new BehaviorTree at position 0

```bash
curl -X POST http://localhost:3001/api/op \
  -H 'Content-Type: application/json' \
  -d '{
    "Root": {
      "Behaviortrees": {
        "Insert": { "pos": 0, "op": "New" }
      }
    }
  }'
```

#### Set the ID (name) of the first BehaviorTree to "patrol"

Each character is inserted one at a time (collaborative text CRDT using EGWalker):

```bash
# Insert 'p' at position 0
curl -X POST http://localhost:3001/api/op \
  -d '{"Root":{"Behaviortrees":{"Update":{"pos":0,"op":{"Id":{"Insert":{"content":"p","pos":0}}}}}}}'

# Insert 'a' at position 1
curl -X POST http://localhost:3001/api/op \
  -d '{"Root":{"Behaviortrees":{"Update":{"pos":0,"op":{"Id":{"Insert":{"content":"a","pos":1}}}}}}}'

# Insert 't' at position 2
curl -X POST http://localhost:3001/api/op \
  -d '{"Root":{"Behaviortrees":{"Update":{"pos":0,"op":{"Id":{"Insert":{"content":"t","pos":2}}}}}}}'

# ... and so on for 'r', 'o', 'l'
```

#### String Editing with the EgWalker CRDT

String fields (like `id`, `name`, `description`) are stored as `List<char>` - a
character-level CRDT based on the Eg-walker algorithm. Each character has a
unique identity, so concurrent edits merge correctly even when two replicas
insert or delete at the same position.

The three operations are:

| Operation | JSON | Effect |
|-----------|------|--------|
| `Insert` | `{"Insert": {"content": "x", "pos": 2}}` | Insert char `x` at index 2 |
| `Delete` | `{"Delete": {"pos": 3}}` | Delete the char at index 3 |
| `DeleteRange` | `{"DeleteRange": {"start": 1, "len": 3}}` | Delete 3 chars starting at index 1 |

All positions are 0-indexed.

**Build a string character by character:**

```bash
# Result: "patrol"
for pair in "0:p 1:a 2:t 3:r 4:o 5:l"; do
  pos=${pair%%:*}; ch=${pair#*:}
  curl -s -X POST http://localhost:3001/api/op \
    -d "{\"Root\":{\"Behaviortrees\":{\"Update\":{\"pos\":0,\"op\":{\"Id\":{\"Insert\":{\"content\":\"$ch\",\"pos\":$pos}}}}}}}"
done
```

**Delete a single character:**

```bash
# "patrol" : "patol"  (delete 'r' at index 3)
curl -X POST http://localhost:3001/api/op \
  -d '{"Root":{"Behaviortrees":{"Update":{"pos":0,"op":{"Id":{"Delete":{"pos":3}}}}}}}'
```

**Delete a range of characters:**

```bash
# "patol" : "pl"  (delete 3 chars starting at index 1: 'a','t','o')
curl -X POST http://localhost:3001/api/op \
  -d '{"Root":{"Behaviortrees":{"Update":{"pos":0,"op":{"Id":{"DeleteRange":{"start":1,"len":3}}}}}}}'
```

**Insert in the middle of an existing string:**

```bash
# "pl" : "pool"  (insert 'o' at 1, then 'o' at 2)
curl -X POST http://localhost:3001/api/op \
  -d '{"Root":{"Behaviortrees":{"Update":{"pos":0,"op":{"Id":{"Insert":{"content":"o","pos":1}}}}}}}'
curl -X POST http://localhost:3001/api/op \
  -d '{"Root":{"Behaviortrees":{"Update":{"pos":0,"op":{"Id":{"Insert":{"content":"o","pos":2}}}}}}}'
```

**Concurrent editing (two replicas editing the same string):**

```bash
# Replica a: insert 'X' at position 0    →  "Xpool"
curl -X POST http://localhost:3001/api/op \
  -d '{"Root":{"Behaviortrees":{"Update":{"pos":0,"op":{"Id":{"Insert":{"content":"X","pos":0}}}}}}}'

# Replica b: insert 'Y' at position 0    →  "Ypool"
curl -X POST http://localhost:3002/api/op \
  -d '{"Root":{"Behaviortrees":{"Update":{"pos":0,"op":{"Id":{"Insert":{"content":"Y","pos":0}}}}}}}'

# After sync, both converge to either "XYpool" or "YXpool"
```

**Same pattern in Rust:**

```rust
use my_project::classifiers::*;
use my_project::package::Behaviortree;
use moirai_crdt::list::eg_walker::List;
use moirai_crdt::list::nested_list::NestedList;

// Write "hello" into the Id field of BehaviorTree at position 0
for (i, ch) in "hello".chars().enumerate() {
    node.apply_op(Behaviortree::Root(Root::Behaviortrees(
        NestedList::Update {
            pos: 0,
            op: BehaviorTree::Id(List::Insert { content: ch, pos: i }),
        },
    )));
}

// Delete character at position 1  ("hello" → "hllo")
node.apply_op(Behaviortree::Root(Root::Behaviortrees(
    NestedList::Update {
        pos: 0,
        op: BehaviorTree::Id(List::Delete { pos: 1 }),
    },
)));

// Delete 2 characters starting at position 0  ("hllo" → "lo")
node.apply_op(Behaviortree::Root(Root::Behaviortrees(
    NestedList::Update {
        pos: 0,
        op: BehaviorTree::Id(List::DeleteRange { start: 0, len: 2 }),
    },
)));
```

#### Create a child Sequence node for the first BehaviorTree

```bash
# Set the child to a ControlNode > Sequence
curl -X POST http://localhost:3001/api/op \
  -d '{
    "Root": {
      "Behaviortrees": {
        "Update": {
          "pos": 0,
          "op": {
            "Child": {
              "ControlNode": {
                "Sequence": "New"
              }
            }
          }
        }
      }
    }
  }'
```

#### Add a BlackboardEntry

```bash
curl -X POST http://localhost:3001/api/op \
  -d '{
    "Root": {
      "Behaviortrees": {
        "Update": {
          "pos": 0,
          "op": {
            "Blackboard": {
              "Entries": {
                "Insert": { "pos": 0, "op": "New" }
              }
            }
          }
        }
      }
    }
  }'
```

#### Add children to a ControlNode (Sequence)

```bash
# Add a first child (Action > OpenDoor) to the Sequence's children list
curl -X POST http://localhost:3001/api/op \
  -d '{
    "Root": {
      "Behaviortrees": {
        "Update": {
          "pos": 0,
          "op": {
            "Child": {
              "ControlNode": {
                "Sequence": {
                  "ControlNodeSuper": {
                    "Children": {
                      "Insert": {
                        "pos": 0,
                        "op": {
                          "ExecutionNode": {
                            "Action": {
                              "OpenDoor": "New"
                            }
                          }
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        }
      }
    }
  }'
```

#### Delete the BehaviorTree at position 0

```bash
curl -X POST http://localhost:3001/api/op \
  -d '{"Root":{"Behaviortrees":{"Delete":{"pos":0}}}}'
```

### Using from Rust directly

```rust
use my_project::classifiers::*;
use my_project::package::Behaviortree;
use moirai_crdt::list::nested_list::NestedList;
use moirai_crdt::list::eg_walker::List;

// Create a BehaviorTree at position 0
let op = Behaviortree::Root(Root::Behaviortrees(
    NestedList::Insert { pos: 0, op: BehaviorTree::New }
));
node.apply_op(op);

// Set its name character by character
for (i, c) in "patrol".chars().enumerate() {
    let op = Behaviortree::Root(Root::Behaviortrees(
        NestedList::Update { pos: 0, op: BehaviorTree::Id(
            List::Insert { content: c, pos: i }
        )}
    ));
    node.apply_op(op);
}

// Add a Sequence child
let op = Behaviortree::Root(Root::Behaviortrees(
    NestedList::Update { pos: 0, op: BehaviorTree::Child(
        Box::new(TreeNodeKind::ControlNode(
            ControlNodeKind::Sequence(Sequence::New)
        ))
    )}
));
node.apply_op(op);
```

## How It Works

1. **`node.apply_op(op)`** calls `Replica::send(op)` which:
   - Checks `is_enabled` on the log
   - Creates an `Event<Op>` with causal metadata (EventId, Lamport clock, version vector)
   - Applies the effect locally
   - Returns an `EventMessage<Op>`

2. The `EventMessage` is wrapped in `TransportMessage::Event` and broadcast to all peers
   via `TcpTransport` (newline-delimited JSON over TCP).

3. Peers receive the `TransportMessage::Event`, call `Replica::receive()` which:
   - Internalizes the remote event (translates replica indices)
   - Buffers it until causally ready
   - Delivers and applies the effect when all dependencies are satisfied

4. On reconnection, peers exchange `SyncRequest`/`Batch` messages to catch up on
   missed operations (delta-based sync using version vectors).

## Simulating Network Partitions

The generic layer supports pausing and resuming peer connections via HTTP,
which lets you simulate disconnections, network partitions, and offline editing
without stopping any replica.

### API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/pause/{peer_id}` | POST | Pause a single peer  |
| `/api/resume/{peer_id}` | POST | Resume a paused peer  |
| `/api/pause-all` | POST | Pause all peers  |
| `/api/resume-all` | POST | Resume all peers |
| `/api/peers` | GET | List peers with status (`Connected` / `Paused`) and buffered message count |

### Example: Simulate Disconnect and Reconnect

Start two replicas:

```bash
# Terminal 1
REPLICA_ID=a LISTEN_PORT=9001 HTTP_PORT=3001 PEERS=b:localhost:9002 cargo run --example network_node

# Terminal 2
REPLICA_ID=b LISTEN_PORT=9002 HTTP_PORT=3002 PEERS=a:localhost:9001 cargo run --example network_node
```

**Step 1 - Pause replica `b` from replica `a`'s perspective:**

```bash
curl -X POST http://localhost:3001/api/pause/b
# {"success":true,"message":"Paused peer 'b'"}
```

**Step 2 - Make edits on both sides while disconnected:**

```bash
# On replica a - create a BehaviorTree
curl -X POST http://localhost:3001/api/op \
  -d '{"Root":{"Behaviortrees":{"Insert":{"pos":0,"op":"New"}}}}'

# On replica b - create a different BehaviorTree
curl -X POST http://localhost:3002/api/op \
  -d '{"Root":{"Behaviortrees":{"Insert":{"pos":0,"op":"New"}}}}'
```

**Step 3 - Check peer status (optional):**

```bash
curl http://localhost:3001/api/peers
# {"peers":[{"id":"b","status":"Paused","buffered":1}]}
```

**Step 4 - Resume:**

```bash
curl -X POST http://localhost:3001/api/resume/b
# {"success":true,"message":"Resumed peer 'b', delivered 1 buffered msgs, requested sync"}
```

After resuming, `a` delivers any messages that were buffered during the pause,
then sends a `SyncRequest` with its version vector. `b` responds with a `Batch`
containing all events `a` hasn't seen. The CRDT merge ensures both replicas converge.

