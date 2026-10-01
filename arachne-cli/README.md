# Arachne CLI

A Command Line Interface (CLI) for the Arachne code generator.

## Usage

```bash
cargo run --locked -p arachne-cli -- generate INPUT.ecore --output OUTPUT_DIRECTORY [--project-name NAME] [--moirai-path DIRECTORY] [-vv]
```

`--moirai-path` selects a local Moirai workspace and writes absolute dependency
paths into the generated manifest. Without it, projects use the pinned Moirai
Git revision documented in the repository README.
