#!/bin/sh
# Runs the frozen tests and prints the score: passing / total, then each failing unit.
cd "$(dirname "$0")" || exit 1
out=$(cargo test -p ootlebook_escrow_tests --test escrow -- --test-threads 4 2>&1)
pass=$(echo "$out" | grep -c '^test .* ok$')
total=$(echo "$out" | grep -c '^test .* \(ok\|FAILED\)$')
[ "$total" -gt 0 ] || { echo "$out" | tail -30; exit 1; }
echo "$pass / $total passing"
# Everything but the one file builders change is frozen; compare this with the hash posted with the breakdown.
echo "frozen files sha256: $(git ls-files | grep -vx template/src/lib.rs | LC_ALL=C sort | xargs sha256sum | sha256sum | cut -c1-64)"
echo "$out" | sed -n 's/^test \(.*\) \.\.\. FAILED$/  failing: \1/p'
[ "$pass" -eq "$total" ]
