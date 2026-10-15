# Arachne CLI

A Command Line Interface (CLI) for the Arachne code generator.

## Usage

```bash
cargo run --locked -p arachne-cli -- generate INPUT.ecore --output OUTPUT_DIRECTORY [--project-name NAME] [--moirai-path DIRECTORY] [-vv]
```

`--moirai-path` selects a local Moirai workspace and writes absolute dependency
paths into the generated manifest. Without it, projects use the pinned Moirai
Git revision documented in the repository README.

For automated consumers, add `--diagnostics-json` to `parse` or `generate`.
Stdout becomes newline-delimited JSON: `arachne-diagnostic` events contain a
level, code, and message; the final `arachne-result` contains `schema_version`,
`stage`, `ok`, and an optional error with code, kind, and message. Parser and
generator warnings are emitted even with `--quiet` or `RUST_LOG=off`. Generated
feature warnings have stable `arachne_codegen::warning::*` codes. Parser log
codes identify their emitting module; parser errors currently use a broad
`parse_error` code. Panics and process termination can prevent the final result;
consumers must also inspect exit status and stderr.

```sh
arachne parse INPUT.ecore --quiet --diagnostics-json
arachne generate INPUT.ecore -o OUTPUT_DIRECTORY --diagnostics-json
```
