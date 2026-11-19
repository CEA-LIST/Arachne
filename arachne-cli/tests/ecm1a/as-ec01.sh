#!/bin/sh
# EC.M1 piece 1: lets EC.01's driver (`ec01.py`, unchanged) run our `arachne`. The driver calls
# `<bin> generate --diagnostics-json -o OUT -p NAME FILE`; our CLI has no `--diagnostics-json` and
# refuses a Moirai root that does not exist, so the flag is dropped and every run writes its
# Moirai paths to the same existing folder, absolute, which keeps the project bytes comparable.
# Every other command passes through unchanged.
real=${ECM1A_REAL:?ECM1A_REAL names the arachne binary}
stub=${ECM1A_STUB:?ECM1A_STUB names an existing folder}
if [ "$1" = generate ]; then
    shift
    [ "$1" = --diagnostics-json ] && shift
    exec "$real" generate -m "$stub" --moirai-path-style absolute "$@"
fi
exec "$real" "$@"
