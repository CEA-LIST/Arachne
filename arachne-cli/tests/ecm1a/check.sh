#!/bin/bash
# EC.M1 piece 1, O2, the suites (spec: cea-cdrt-knowledge/spec/items/EC.M1.md, section 4).
# From this worktree, each step once:
#   S1 the parser crate, every target; S2 arachne-xmi; S3 arachne-codegen and arachne-cli, every
#      target except generated_crates (piece 3: it builds generated crates against Moirai) and
#      md04_import_sirius (piece 3: it launches nodes); every test's name and outcome must equal
#      names.txt line for line, every command exit 0;
#   B1 the workspace builds with all targets; L1 cargo fmt --all --check; L2 cargo clippy
#      --workspace --all-targets exit 0 (warnings reported, not judged).
# Exit 0 when all hold, 1 otherwise.
#
#   ECM1_SCRATCH=<dir> arachne-cli/tests/ecm1a/check.sh
set -uo pipefail
export LC_ALL=C
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0

HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../../.." && pwd)
SCRATCH=${ECM1_SCRATCH:?ECM1_SCRATCH names the work folder}
TARGET=${ECM1_TARGET:-/volatile/home/vp242103/.cache/cea-crdt-scratch/EC.M2/target}
OUT=$SCRATCH/ecm1a-check
LOCK=/volatile/home/vp242103/.cache/cea-crdt-scratch/cargo.lock

P() { (cd "$REPO" && CARGO_TARGET_DIR=$TARGET CARGO_BUILD_JOBS=2 \
    flock -w 3000 "$LOCK" nice -n 10 cargo "$@"); }

# `<suite> <target> :: <name> <ok|FAILED|ignored>`; <target> is the test binary's name without
# its hash, `doc` for doc tests (EC.M2's check.sh, plus the doc-test header).
names_of() { # suite log
    awk -v s="$1" '/^ *Running .*\(.*\/deps\/[^)]*\)$/{b=$NF; sub(/^\(.*\/deps\//,"",b); sub(/\)$/,"",b); sub(/-[0-9a-f]+$/,"",b); t=b}
        /^ *Doc-tests /{t="doc:"$2}
        /^test .* \.\.\. /{o=$NF; if ($0 ~ /\.\.\. ignored/) o="ignored"; n=$0; sub(/^test /,"",n); sub(/ \.\.\. .*$/,"",n); print s" "t" :: "n" "o}' "$2"
}

S3=(--lib --bins)
for t in ar01_regeneration ar02_descriptor_declarations ar04_sysml_registers ar05b_ecore_builtin_datatypes \
    ar06_doc_comments ed10_umlclass_metamodel ed16a_notation_diagrams md08_common md08_dependencies \
    si05_identity_reference_emitter si05_inline_replacements si10_shipped_descriptors \
    ed12_import_sirius ec01_describe_parity; do S3+=(--test "$t"); done
T="test -j 2 --offline --no-fail-fast"

rm -rf "$OUT" && mkdir -p "$OUT"
status=0
step() { # id command...
    local id=$1; shift
    "$@" >"$OUT/$id.log" 2>&1
    local code=$?
    echo "$id exit $code"
    [ "$code" = 0 ] || status=1
}
step S1 P $T -p ecore_rs
step S2 P $T -p arachne-xmi
step S3 P $T -p arachne-codegen -p arachne-cli "${S3[@]}"
step B1 P build -j 2 --offline --workspace --all-targets
step L1 bash -c "cd '$REPO' && cargo fmt --all --check"
step L2 P clippy -j 2 --offline --workspace --all-targets

for s in S1 S2 S3; do names_of "$s" "$OUT/$s.log"; done | sort >"$OUT/names.txt"
if diff "$HERE/names.txt" "$OUT/names.txt" >"$OUT/names.diff"; then
    echo "names: $(wc -l <"$OUT/names.txt") equal to names.txt"
else
    echo "names: FAIL, $(grep -c '^<' "$OUT/names.diff") expected lines missing or changed, $(grep -c '^>' "$OUT/names.diff") observed lines not expected (< expected, > observed; first 40):"
    grep '^[<>]' "$OUT/names.diff" | head -40
    status=1
fi
echo "clippy warnings: $(grep -c '^warning' "$OUT/L2.log") (reported, not judged)"
[ "$status" = 0 ] && echo PASS || echo FAIL
exit $status
