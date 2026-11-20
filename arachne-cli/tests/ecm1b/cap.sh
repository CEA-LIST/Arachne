#!/bin/sh
# EC.M1b: runs $ECM1B_REAL with EC.M1's census caps (3 GiB of address space, no core files) and its
# arguments unchanged, so EC.01's driver calls every build, his CLI's `generate --diagnostics-json` included,
# the same way.
ulimit -v 3145728
ulimit -c 0
exec "${ECM1B_REAL:?ECM1B_REAL names the arachne binary}" "$@"
