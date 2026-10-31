# shellcheck shell=bash
# Shared by ops/beta/*.sh (ADR-012 D12.6). Sourced, never run.
#
# Paths come from this checkout, never from another clone or a scratchpad:
#   REPO    the checkout these scripts live in
#   PRIV    private operator state, default <checkout>/.cache/ops/beta (0700,
#           gitignored); GENUI_OPS_PRIV overrides it
#   OPVENV  the operator venv `just setup` makes (app/operator/.venv)
# The stack's identifiers (region, stack, registry, cluster, ...) are read from
# $PRIV/env.sh, which ops/beta/env.example.sh documents by name; values never
# enter Git and the scripts never print them.

_OPS_REPO=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
REPO=$_OPS_REPO
PRIV=${GENUI_OPS_PRIV:-$REPO/.cache/ops/beta}
OPVENV=$REPO/app/operator/.venv
PY=$REPO/app/runtime/.venv/bin/python
DRY_RUN=${DRY_RUN:-0}
APPROVED=${APPROVED:-}

die() { echo "error: $*" >&2; exit 1; }

# Private directories are 0700 and files 0600 (DEPLOY-RUNBOOK 0.4).
umask 077

priv_ready() {
  [[ -d "$PRIV" ]] || die "no private directory at $PRIV (DEPLOY-RUNBOOK.md, 'The private directory'; ops/beta/import-legacy-priv.sh)"
  [[ "$(stat -f %Lp "$PRIV" 2>/dev/null || stat -c %a "$PRIV")" == 700 ]] || die "$PRIV must be mode 0700"
}

load_env() {
  priv_ready
  [[ -f "$PRIV/env.sh" ]] || die "missing $PRIV/env.sh (copy ops/beta/env.example.sh and fill it in privately)"
  # shellcheck disable=SC1091
  source "$PRIV/env.sh"
  # Never let a private env file point the scripts at another checkout.
  REPO=$_OPS_REPO
  PRIV=${GENUI_OPS_PRIV:-$REPO/.cache/ops/beta}
  OPVENV=$REPO/app/operator/.venv
  unset CAND CANDIDATE WINDOW WINDOW2
  export AWS_REGION
}

# window_dir <id>: the window's private directory (DEPLOY-RUNBOOK 0.4).
window_dir() {
  local id=$1
  [[ "$id" =~ ^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$ ]] || die "window id must be a lowercase UUIDv4: $id"
  W=$PRIV/windows/$id
}
need_window_dir() {
  [[ -d "$W" ]] || die "no window directory $W (just deploy new ...)"
  [[ -f "$W/release.json" ]] || die "no $W/release.json (windows are made by 'just deploy new')"
}

# release_set <jq filter> [jq args...]: update $W/release.json in place
# (schema: ops/beta/release.py; the file stays 0600 under umask 077).
release_set() {
  local filter=$1; shift
  local tmp="$W/release.json.new"
  jq "$@" "$filter | .updated_at_utc = (now | floor | todate)" "$W/release.json" > "$tmp" && mv "$tmp" "$W/release.json"
}
release_get() { jq -r "$1" "$W/release.json"; }

# shown <code>: a read or local step. The code is printed exactly as it runs
# (variables by name), then evaluated; --dry-run only prints it.
shown() {
  printf '%s\n' "$1" | sed '/^$/d; s/^/+ /'
  (( DRY_RUN )) && return 0
  eval "$1"
}

# prepare <code>: local or read work that only serves the mutation after it.
# It runs only when that mutation is approved (or prints with --dry-run), so
# a refused step leaves nothing behind.
prepare() {
  if [[ -n "$APPROVED" ]] || (( DRY_RUN )); then shown "$1"; return; fi
  printf '%s\n' "$1" | sed '/^$/d; s/^/+ (runs once approved) /'
}

# mutate <step> <shell code>: an AWS mutation (DEPLOY-RUNBOOK [MUTATION]).
# The code is printed exactly as it will run (variables from $PRIV/env.sh are
# shown by name, never expanded into the output). Without --approved it
# refuses. With --approved, Victor's words are appended to $PRIV/approvals.md
# first, then the code runs. --dry-run prints and records nothing.
mutate() {
  local step=$1 code=$2
  echo "[MUTATION] $step:"
  printf '%s\n' "$code" | sed 's/^/    /'
  if [[ -z "$APPROVED" ]]; then
    echo "refused: '$step' changes AWS. Re-run with --approved \"<Victor's verbatim words for this action>\"." >&2
    exit 3
  fi
  if (( DRY_RUN )); then
    echo "(dry run: would append the approval to $PRIV/approvals.md, then run the above)"
    return 0
  fi
  record_approval "$step"
  eval "$code"
}

record_approval() {
  local step=$1
  priv_ready
  [[ -f "$PRIV/approvals.md" ]] || printf "# Victor's approvals (private window record)\n\n" > "$PRIV/approvals.md"
  chmod 600 "$PRIV/approvals.md"
  local at
  at=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  printf -- '- %s UTC, `just deploy %s`, window %s: "%s"\n' \
    "${at%Z}" "$step" "${WINDOW_ID:--}" "$APPROVED" >> "$PRIV/approvals.md"
  # ADR-013 D13.5: the window's release record points at the approval (its
  # line in approvals.md), so each mutation in release.json has its word.
  if [[ -n "${W:-}" && -f "$W/release.json" ]]; then
    release_set '.approvals += [{step: $s, at_utc: $a, approvals_md_line: ($l | tonumber)}]' \
      --arg s "$step" --arg a "$at" --arg l "$(wc -l < "$PRIV/approvals.md" | tr -d ' ')"
  fi
}

# ADR-013 D13.5: the public origin the beta answers on (verify, the health
# wait of activate). Never an AWS endpoint.
BETA_ORIGIN=${GENUI_BETA_ORIGIN:-https://beta.getforma.dev}

# release_py <args>: ops/beta/release.py (stdlib only, isolated).
release_py() { python3 -I "$REPO/ops/beta/release.py" "$@"; }
