# Arachne CLI

A Command Line Interface (CLI) for the Arachne code generator.

## Usage

```bash
RUST_LOG=debug cargo run generate -vv -o <WHERE_TO_GENERATE_PROJECT> <PATH_TO_ECORE_METAMODEL>
```

## Importing a Sirius Web / SysON project

```bash
cargo run -- import-sirius project.json --descriptor sysml.json > ops.json
curl -X POST -H 'Content-Type: application/json' \
  -d '{"metamodel_id": "<digest>"}' http://<node>/api/models
curl -X POST --data-binary @ops.json http://<node>/api/model/<model_id>/ops
```

`import-sirius` prints `{"ops": [...]}`, the operations that build the project's model on an empty model registered under that descriptor, and posts nothing. A reference whose target is an object of the project is rewritten from the Sirius id to the target's id attribute (`elementId` in SysML). Any other reference value, such as a standard-library URI, is written as the source wrote it and reported on stderr as `unresolved: <holder> <feature> <value>`, followed by `references: <R> resolved, <U> unresolved`. stderr also names the descriptor's digest. A class or feature the descriptor cannot carry stops the command, and the message names the element.
