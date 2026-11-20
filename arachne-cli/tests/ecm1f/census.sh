#!/usr/bin/env bash
# EC.M1f census (a report, not a pass condition): `describe` on the 133 ModelSet files our build
# refused with Léo's forward-package line in EC.M1 a2/o4 his.csv. Prints `read` for each file it
# reads and the first stderr line (names masked) for the rest, counted.
# Usage: census.sh <arachne binary> [corpus dir]
set -u
export LC_ALL=C
BIN=$1
CORPUS=${2:-/volatile/home/vp242103/datasets/modelset/unpacked/modelset/raw-data/repo-ecore-all/data}
HERE=$(cd "$(dirname "$0")" && pwd)
while IFS= read -r id; do
  if err=$("$BIN" describe "$CORPUS/${id#modelset/}" 2>&1 >/dev/null); then echo read; else
    printf '%s\n' "$err" | head -n 1 | sed -e 's/\x1b\[[0-9;]*m//g' -e 's/`[^`]*`/`_`/g'
  fi
done < "$HERE/ids.txt" | sort | uniq -c | sort -rn
