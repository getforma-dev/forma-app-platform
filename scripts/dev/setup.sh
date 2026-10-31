#!/usr/bin/env bash
# just setup (ADR-012 D12.3, D12.4): make this checkout self-sufficient.
#
#   - Rust: the toolchain named by rust-toolchain.toml (rustup installs it).
#   - Python: the interpreter named by .python-version, through uv, and one venv
#     per lock set, created in this checkout from the existing hashed locks
#     (--require-hashes, wheels only). No venv is borrowed from another checkout.
#   - Node: npm ci in app/admin and infra (skipped when the lock is unchanged
#     since the last successful install; --force reinstalls).
#   - Docker: reachable or not (reported; the campaigns and images need it).
#
# Idempotent. Never reads .env or any secret.
#
#   scripts/dev/setup.sh [--force]
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$root"
force=0
[[ "${1:-}" == --force ]] && force=1

say() { printf '\033[1m==> %s\033[0m\n' "$*"; }
warn() { printf '\033[33mwarning:\033[0m %s\n' "$*" >&2; }

command -v uv >/dev/null || { echo "uv is missing: brew install uv (version in tools.versions)" >&2; exit 1; }

# --- Rust -------------------------------------------------------------------
say "Rust toolchain from rust-toolchain.toml"
# rustup >= 1.28 installs the active toolchain (with its components and
# targets) on demand; `rustup toolchain install` with no name reads the file.
rustup toolchain install --no-self-update >/dev/null 2>&1 || rustup toolchain install --no-self-update
(cd app && rustc --version)

# --- Python -----------------------------------------------------------------
pyver=$(tr -d '[:space:]' < .python-version)
say "Python $pyver (uv-managed)"
uv python install --quiet "$pyver"
python=$(uv python find "$pyver")

# venv <dir> <lock>...: create or refresh <dir>/.venv with exactly the locks.
venv() {
  local dir=$1; shift
  local path="$dir/.venv"
  if [[ -L "$path" ]]; then
    # A symlink into another checkout (the old committed contracts/.venv):
    # remove the link only, never its target.
    warn "$path is a symlink to $(readlink "$path"); replacing it with a local venv"
    rm "$path"
  fi
  local have=""
  [[ -x "$path/bin/python" ]] && have=$("$path/bin/python" -c 'import platform;print(platform.python_version())' 2>/dev/null || true)
  if [[ "$have" != "$pyver" ]]; then
    uv venv --quiet --clear --python "$python" "$path"
  fi
  uv pip sync --quiet --python "$path/bin/python" --require-hashes --only-binary :all: "$@"
  printf '  %-24s %s (%s)\n' "$path" "$pyver" "$*"
}
say "venvs from the hashed locks"
# The runtime venv carries both locks, exactly as CI installs it
# (tenant-foundation.yml); campaigns and the dev stand use it.
venv app/runtime app/runtime/requirements.lock contracts/requirements.lock
venv contracts contracts/requirements.lock
# The operator lock is the union of the three (app/operator/requirements.lock:2-4);
# image checks and operator steps use this venv (replaces the other clone's opvenv).
venv app/operator app/operator/requirements.lock
# Fixture and query qualification (fixtures.yml, query.yml).
venv spikes/APP-005 contracts/requirements.lock spikes/APP-005/requirements.lock app/runtime/requirements.lock

# --- Node -------------------------------------------------------------------
want_node=$(tr -d '[:space:]' < .node-version)
have_node=$(node --version 2>/dev/null || echo none)
[[ "$have_node" == "v$want_node" ]] || warn "node is $have_node, .node-version says $want_node (nvm use / fnm use)"
npm_ci() {
  local dir=$1 stamp="$1/node_modules/.genui-lock.sha256"
  local sum
  sum=$(shasum -a 256 "$dir/package-lock.json" | cut -d' ' -f1)
  if [[ $force == 0 && -f "$stamp" && "$(cat "$stamp")" == "$sum" ]]; then
    printf '  %-24s up to date\n' "$dir"
    return
  fi
  (cd "$dir" && npm ci --no-audit --no-fund --loglevel=error)
  echo "$sum" > "$stamp"
  printf '  %-24s installed\n' "$dir"
}
say "npm ci"
npm_ci app/admin
npm_ci infra

# --- Docker -----------------------------------------------------------------
say "Docker"
if docker info --format '{{.ServerVersion}}' >/dev/null 2>&1; then
  echo "  engine reachable"
else
  warn "Docker engine not reachable: campaigns, the qualifier and image builds need it"
fi

say "setup done; run 'just doctor' to compare versions with tools.versions"
