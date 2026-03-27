# Arachne

**Arachne** is a Rust-based code generator that compiles Domain-Specific Modeling Languages (DSMLs) defined with Ecore metamodels into Conflict-free Replicated Data Types (CRDTs), leveraging the Moirai library.

## Overview

This code generator bridges the gap between high-level domain models (defined in Ecore) and distributed, eventually consistent data structures (CRDTs). It automatically generates Rust code that leverages the Moirai library to provide conflict-free replication semantics for your domain models.

## Project Organization

- `arachne-parser`: an Ecore-to-Rust parser, forked from `ecore.rs`.
- `arachne-codegen`: core component that generates a composition of CRDTs from a parsed Ecore metamodel.
- `arachne-cli`: Command Line Interface tool.

## License

See the [LICENSE](LICENSE) file for details.

---

## Acknowledgments

Developed at [CEA LIST](https://list.cea.fr/en/), the French Alternative Energies and Atomic Energy Commission.

**Authors:**

- Léo Olivier ([@leo-olivier](https://github.com/leo-olivier))
