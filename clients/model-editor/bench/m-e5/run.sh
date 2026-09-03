#!/usr/bin/env bash
#
# M-E5 — apply-to-file latency and store size against model size.
#
# Runs bench/m-e5/apply-to-file.bench.ts under vitest, which writes
# results.csv and manifest.txt beside itself. Needs the node binary for the
# calibration step (generated/json_crdt/target/debug/examples/network_node, or
# MOIRAI_E2E_NODE_BIN) and Chrome for the OPFS reading (/usr/bin/google-chrome
# or CHROME_BIN); each is skipped with a note in the manifest when absent.
#
# Usage, from this directory: ./run.sh
# Knobs: M_E5_SIZES (default 10,100,1000,10000).

set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$HERE/../.."
echo "m-e5: load average $(cut -d' ' -f1-3 /proc/loadavg)" >&2
npm run -s bench:m-e5
echo
cat "$HERE/manifest.txt"
echo
column -s, -t < "$HERE/results.csv"
