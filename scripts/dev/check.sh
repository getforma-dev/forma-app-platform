#!/usr/bin/env bash
# just check / just ci (ADR-012 D12.3): the CI-fast set in one command.
#
#   check.sh                 every fast step; a summary names each failure
#   check.sh <step>...       only those steps
#   check.sh --list          the steps and the command each runs
#   check.sh --ci            the fast steps, then the CI campaigns
#                            (tenant-foundation.yml, pitch.yml) and the schema qualifier
#
# Each step mirrors a CI workflow command (named in the step list below).
# The steps run in order and all run even after a failure; the exit status is
# non-zero when any failed. Python steps use the per-checkout venvs that
# `just setup` makes; nothing reads .env.
set -uo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$root"
py=app/runtime/.venv/bin/python
reports="$root/.cache/reports"

# name|CI source|command (run with bash -c from the repository root)
steps=(
  "fmt|tenant-foundation.yml|cargo fmt --check -p forma-app-platform"
  "clippy|tenant-foundation.yml|cargo clippy --locked -p forma-app-platform --all-targets -- -D warnings"
  "clippy-dev|ADR-012 D12.1 (the dev binary)|cargo clippy --locked -p forma-app-platform --all-targets --features dev-stand -- -D warnings"
  "test|tenant-foundation.yml (storage::), DEPLOY-RUNBOOK 0.2|cargo test --locked -p forma-app-platform --lib"
  "msrv|tenant-foundation.yml|cargo +1.85.0 check --locked -p forma-app-platform --lib --tests"
  "catalog|ADR-012 campaign catalog|scripts/dev/campaigns.sh verify"
  "admin|tenant-foundation.yml|npm --prefix app/admin run typecheck && npm --prefix app/admin run build"
  "infra|DEPLOY-RUNBOOK 0.2|cd infra && npm run typecheck && npm test"
  "py-scripts|tenant-foundation.yml|$py -m unittest discover -s scripts -p 'test_*.py'"
  "py-runtime|tenant-foundation.yml, foundation.yml|$py -m unittest discover -s app/runtime"
  "py-qualification|tenant-foundation.yml|$py -m unittest discover -s app/qualification"
  "py-app-scripts|DEPLOY-RUNBOOK 0.2|$py -m unittest discover -s app/scripts -p 'test_*.py'"
  "py-ops|ADR-013 D13.5 (release record, fake AWS)|python3 -m unittest discover -s ops/beta -p 'test_*.py'"
  "py-infra|DEPLOY-RUNBOOK 0.2|cd infra/test && ../../$py -m unittest preflight_test window_requests_test"
  "schema-pins|tenant-foundation.yml|python3 scripts/check-schema-pins.py"
  "contracts|contracts.yml|contracts/.venv/bin/python -m unittest discover -s contracts -p 'test_*.py' && node contracts/verify_vectors.mjs && contracts/.venv/bin/python contracts/lock.py check"
  "board|planning.yml|python3 Tickets/_tools/board.py check"
  "bash-n|ADR-012 D12.3|scripts/dev/bash-n.sh"
)
# The --ci additions, after the fast steps.
ci_steps=(
  "schema-qualifier|tenant-foundation.yml|$py app/runtime/qualify_tenant_schema.py --output $reports/schema.json"
  "campaigns-ci|tenant-foundation.yml, pitch.yml|scripts/dev/campaigns.sh run-ci"
)

field() { local IFS='|'; read -r a b c <<<"$1"; case $2 in 1) echo "$a";; 2) echo "$b";; 3) echo "$c";; esac; }

if [[ "${1:-}" == --list ]]; then
  for s in "${steps[@]}" "${ci_steps[@]}"; do printf '%-17s %-40s %s\n' "$(field "$s" 1)" "($(field "$s" 2))" "$(field "$s" 3)"; done
  exit 0
fi

selected=("${steps[@]}")
if [[ "${1:-}" == --ci ]]; then
  selected=("${steps[@]}" "${ci_steps[@]}")
elif (( $# )); then
  selected=()
  for want in "$@"; do
    found=""
    for s in "${steps[@]}" "${ci_steps[@]}"; do [[ "$(field "$s" 1)" == "$want" ]] && found=$s; done
    [[ -n "$found" ]] || { echo "unknown step '$want' (check.sh --list)" >&2; exit 2; }
    selected+=("$found")
  done
fi

mkdir -p "$reports"
failed=(); passed=(); skipped=()
for s in "${selected[@]}"; do
  name=$(field "$s" 1); cmd=$(field "$s" 3)
  if [[ $name == msrv ]] && ! rustup toolchain list 2>/dev/null | grep -q '^1\.85\.0'; then
    printf '\n\033[33m--- %s: skipped (rustup toolchain install 1.85.0 --profile minimal)\033[0m\n' "$name"
    skipped+=("$name"); continue
  fi
  printf '\n\033[1m--- %s\033[0m  %s\n' "$name" "$cmd"
  start=$SECONDS
  if bash -c "set -eo pipefail; $cmd"; then
    passed+=("$name ($((SECONDS - start))s)")
  else
    failed+=("$name")
  fi
done

printf '\n\033[1m=== check summary\033[0m\n'
for p in ${passed[@]+"${passed[@]}"}; do echo "  ok    $p"; done
for p in ${skipped[@]+"${skipped[@]}"}; do echo "  skip  $p"; done
for f in ${failed[@]+"${failed[@]}"}; do echo "  FAIL  $f"; done
(( ${#failed[@]} == 0 ))
