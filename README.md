# Arachne

**Arachne** is a Rust code generator that compiles Domain-Specific Modeling
Languages (DSMLs) defined by Ecore metamodels into Rust scaffold projects. A
generated project implements the metamodel as a local-first replicated model
API using Conflict-free Replicated Data Types (CRDTs) from the
[Moirai library](https://github.com/CEA-LIST/Moirai).

The generated source is the implementation of the input metamodel. It becomes
part of a running modeling application after it is compiled and integrated
with an application transport and user interface.

> **Supported Ecore subset:** Arachne does not support the complete Ecore
> metalanguage. See the
> [mapping and supported features](./arachne-codegen/README.md) before using a
> metamodel outside the supplied examples.

## Overview

### Motivation

Expressing instances of a recurring problem as sentences in a small, Domain-Specific Language (DSL) is a well-established software engineering technique for simplifying the development and maintenance of business applications.

In the Model-Driven Engineering (MDE) community, metamodels are used to specify the abstract syntax of DSLs in terms of domain concepts, properties, and relationships. Although modern language workbenches can generate much of the language infrastructure (e.g., APIs, (de)serialization, and editors) from a metamodel, support for collaborative model editing is typically delegated to external services, requiring additional integration effort from language engineers. In particular, local-first collaborative modeling, where modelers can seamlessly alternate between online and offline editing, with automatic reconciliation of changes upon reconnection, is poorly supported.

This lack of support is particularly problematic for DSMLs that are used in distributed systems, where modelers may be geographically dispersed and have limited or intermittent network connectivity. In such scenarios, local-first collaborative modeling is essential for enabling effective collaboration and ensuring that all modelers can contribute to the development of the system, regardless of their location or network conditions.

### Our approach

Arachne addresses this problem by automatically generating local-first,
decentralized implementations of modeling languages from their metamodels.
The generated implementations are built on CRDTs, which provide a principled
approach to eventual consistency in distributed systems. When integrated into
an application, they allow modelers to update local replicas independently and
automatically reconcile those replicas without a centralized merge service.

An Ecore metamodel is compiled into a composition of
[pure operation-based CRDTs](https://arxiv.org/abs/1710.04469) that mirrors its
structure. The generated code supports the documented subset of classifiers,
attributes, containment hierarchies, references, multiplicities, and
subtyping.

![Approach overview](./images/approach_overview.png)

### Generated implementation capabilities

The generated implementation provides an API for creating, reading, updating,
and deleting model elements through CRDT update and read queries. Moirai
provides the replication infrastructure, including causal event delivery.
Persistence, network transport, and graphical editing are outside the generated
project and must be provided by the surrounding application.

Moirai implements a
[Reliable Causal Broadcast (RCB) protocol](https://courses.edx.org/asset-v1:KTHx+ID2203.2x+2016T4+type@asset+block/Lecture_6_Causal_Broadcast.pdf). Arachne does not generate a network
layer. The generated send and receive API can be connected to a transport
selected by the application developer.

#### Usage example

You can programmatically create one or several replicas of a generated CRDT, and then perform operations on the replicas. Schematically, the usage of a generated CRDT can be represented as follows:

```rust
// Create two replicas of the generated CRDT, each with a unique identifier and a list of replicas it can communicate with.
let mut replica_a = Replica::<MyGeneratedCRDT>::bootstrap("a", &["a", "b"]);
let mut replica_b = Replica::<MyGeneratedCRDT>::bootstrap("b", &["a", "b"]);

// Perform an operation on replica A
let event_a = replica_a.send(MyGeneratedCRDT::SomeOperation { /* ... */ });
// Read the current state of replica A
let returned_value_a = replica_a.query(Read::new());

// B receives the operation from A and applies it to its local state
replica_b.receive(event_a);
// Read the current state of replica B
let returned_value_b = replica_b.query(Read::new());

// A and B have delivered the same operations in a causally consistent order, and thus have converged to the same state.
assert_eq!(returned_value_a, returned_value_b);
```

## Repository organization

- `arachne-parser`: Ecore parser, forked from `ecore.rs`.
  See the [parser README](./arachne-parser/README.md).
- `arachne-codegen`: mapping analysis and Rust code generation.
  See the [codegen README](./arachne-codegen/README.md).
- `arachne-cli`: command-line interface exposed as `arachne`.
  See the [CLI README](./arachne-cli/README.md).
- `examples`: input Ecore metamodels used to exercise the generator.
- `modelset_coverage.py`: ModelSet parser, generation, and compilation
  coverage runner.
- `.devcontainer`: reproducible Linux development environment for Windows,
  Linux, and macOS hosts.

Canonical source repositories:

- Arachne: <https://github.com/CEA-LIST/Arachne>
- Moirai: <https://github.com/CEA-LIST/Moirai>

## Running the generator

From the repository root, build and run the CLI with Cargo. The `generate` command accepts an input `.ecore` file, an output directory, and
an optional Cargo project name:

```sh
arachne generate INPUT.ecore --output OUTPUT_DIRECTORY [--project-name NAME]
```

The generated directory is a Rust project containing `Cargo.toml` and the
generated modules under `src/`. Inside the Dev Container, the `arachne` binary
is already installed, so `arachne generate ...` can be used directly.

### Generating the examples

Generate the three metamodel examples:

```sh
cargo run --locked --release -p arachne-cli -- generate examples/class_hierarchy.ecore --output generated/class-hierarchy --project-name class-hierarchy
cargo run --locked --release -p arachne-cli -- generate examples/behavior_tree.ecore --output generated/behavior-tree --project-name behavior-tree
cargo run --locked --release -p arachne-cli -- generate examples/json.ecore --output generated/json --project-name json-model
```

Compile the generated projects:

```sh
cargo check --manifest-path generated/class-hierarchy/Cargo.toml
cargo check --manifest-path generated/behavior-tree/Cargo.toml
cargo check --manifest-path generated/json/Cargo.toml
```

## ModelSet coverage

Download and extract
[ModelSet v0.9.4](https://github.com/modelset/modelset-dataset/releases/tag/v0.9.4),
then run a small sample:

```sh
python3 modelset_coverage.py PATH_TO_MODELSET --limit 5 --csv modelset-results/modelset_coverage_smoke.csv
```

Run the complete dataset by omitting `--limit`. Be aware that the complete run can take a long time and requires a some gigabytes of disk space for the generated projects.

```sh
python3 modelset_coverage.py PATH_TO_MODELSET --csv modelset-results/modelset_coverage_reproduced.csv
```

On Windows, `py -3` can be used instead of `python3`. The script builds Arachne
in release mode and records whether each metamodel can be parsed, generated,
and compiled. Unsupported metamodels are expected and are reported in the
`error` column.

Reference results are available in the
[coverage report](./modelset-results/MODELSET_COVERAGE_REPORT.md) and the
[per-model CSV](./modelset-results/modelset_coverage.csv).

## License

Arachne is distributed under the Apache License 2.0. See
[`LICENSE`](./LICENSE).
