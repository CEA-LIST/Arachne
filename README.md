# Arachne

**Arachne** is a Rust-based code generator that compiles Domain-Specific Modeling Languages (DSML) designed using Ecore metamodels to Conflict-free Replicated Data Types (CRDT) using the Moirai library.

## Overview

This code generator bridges the gap between high-level domain models (defined in Ecore) and distributed, eventually consistent data structures (CRDTs). It automatically generates Rust code that leverages the Moirai library to provide conflict-free replication semantics for your domain models.

## Project structure

- `arachne-parser`: an Ecore-to-Rust parser, forked from `ecore.rs`.
- `arachne-codegen`: core component that generates a composition of CRDTs from a parsed Ecore metamodel.
- `arachne-cli`: Command Line Interface tool.
