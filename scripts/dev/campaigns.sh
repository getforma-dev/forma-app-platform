#!/usr/bin/env bash
# just campaigns / just campaign <name> (ADR-012 D12.3): the ignored-test
# catalog in scripts/dev/campaigns.tsv.
#
#   campaigns.sh list [--ci|--all]  the catalog (default: campaigns and probes)
#   campaigns.sh run <name> [args]  one test, exactly as CI runs it:
#                                   cargo test --locked --lib <path> -- --ignored --exact --nocapture [args]
#                                   with its report at .cache/reports/<name>.json
#   campaigns.sh run-ci             every campaign CI runs, in order (just ci)
#   campaigns.sh verify             the catalog equals cargo's list of ignored tests
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
catalog="$root/scripts/dev/campaigns.tsv"
reports="$root/.cache/reports"

rows() { grep -v -e '^#' -e '^name	' "$catalog"; }
row() { rows | awk -F'\t' -v n="$1" '$1==n'; }

list() {
  local filter=${1:-}
  printf '%-22s %-9s %-18s %s\n' NAME KIND CI PATH
  rows | while IFS=$'\t' read -r name path kind ci report env needs; do
    case $filter in
      --ci) [[ $ci != - ]] || continue ;;
      --all) ;;
      *) [[ $kind == campaign || $kind == probe ]] || continue ;;
    esac
    printf '%-22s %-9s %-18s %s\n' "$name" "$kind" "$ci" "$path"
  done
  local total ci_count
  total=$(rows | wc -l | tr -d ' ')
  ci_count=$(rows | awk -F'\t' '$4!="-"' | wc -l | tr -d ' ')
  echo
  echo "$total ignored tests; $ci_count run in CI. 'just campaigns --all' adds smokes, the stand and helpers;"
  echo "'just campaign <name>' runs one (needs: column 'needs' in scripts/dev/campaigns.tsv)."
}

run() {
  local name=${1:-}; shift || true
  local line; line=$(row "$name")
  [[ -n "$line" ]] || { echo "unknown campaign '$name' (just campaigns --all)" >&2; exit 2; }
  local path kind ci report env needs
  IFS=$'\t' read -r _ path kind ci report env needs <<<"$line"
  case $kind in
    stand) echo "'$name' is the dev stand: use 'just dev'" >&2; exit 2 ;;
    helper) echo "'$name' is a helper ($needs); it is not run on its own" >&2; exit 2 ;;
  esac
  mkdir -p "$reports"
  local vars=()
  [[ $report == - ]] || vars+=("$report=$reports/$name.json")
  if [[ $env != - ]]; then
    local e
    for e in ${env//,/ }; do vars+=("${e//@VENV@/$root/app/runtime/.venv/bin/python}"); done
  fi
  echo "campaign $name ($kind; needs: $needs)"
  [[ $report == - ]] || echo "report: $reports/$name.json"
  cd "$root/app"
  set -x
  env ${vars[@]+"${vars[@]}"} cargo test --locked --lib "$path" -- --ignored --exact --nocapture "$@"
}

run_ci() {
  local failed=() name
  for name in $(rows | awk -F'\t' '$4!="-" {print $1}'); do
    if ! (run "$name"); then failed+=("$name"); fi
  done
  if (( ${#failed[@]} )); then echo "CI campaigns failed: ${failed[*]}" >&2; exit 1; fi
  echo "CI campaigns: all passed"
}

verify() {
  local want have
  want=$(rows | cut -f2 | sort)
  have=$(cd "$root/app" && cargo test --locked --lib -q -- --list --ignored 2>/dev/null | sed -n 's/: test$//p' | sort)
  if [[ -z "$have" ]]; then echo "could not list the ignored tests (does 'cargo test --lib' build?)" >&2; exit 1; fi
  if [[ "$want" == "$have" ]]; then
    echo "campaign catalog matches cargo: $(echo "$have" | wc -l | tr -d ' ') ignored tests"
  else
    echo "campaign catalog differs from cargo's ignored tests (< catalog, > cargo):" >&2
    diff <(echo "$want") <(echo "$have") >&2 || true
    exit 1
  fi
}

case "${1:-list}" in
  list) shift || true; list "$@" ;;
  run) shift; run "$@" ;;
  run-ci) run_ci ;;
  verify) verify ;;
  *) echo "usage: campaigns.sh list [--ci|--all] | run <name> [args] | run-ci | verify" >&2; exit 2 ;;
esac
