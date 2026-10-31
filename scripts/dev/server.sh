#!/usr/bin/env bash
# The dev server of a dev instance (process-compose `server`, ADR-012 D12.1):
# build genui-dev (feature dev-stand; incremental), then run it.
#
# ANTHROPIC_API_KEY is read from the clone's .env (a worktree: the main
# clone's, through git's common dir) by app/scripts/read_env.py into this
# process's environment only, after the build and right before exec; it is
# never printed or put on a command line. Without it: no model.
#
# A failed build exits 0 so the supervisor does not rebuild in a loop: fix the
# code and run `just restart` (`just restart clean`).
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
: "${GENUI_DEV_STATE:?run through scripts/dev/dev.sh}"
cd "$root"
if ! cargo build --locked -p forma-app-platform --features dev-stand --bin genui-dev; then
  echo "genui-dev: the build failed; fix it, then run 'just restart'" >&2
  exit 0
fi
target=${CARGO_TARGET_DIR:-$root/target}

env_file() {
  if [[ -n "${GENUI_ENV_FILE:-}" ]]; then echo "$GENUI_ENV_FILE"; return; fi
  if [[ -f "$root/.env" ]]; then echo "$root/.env"; return; fi
  local common
  common=$(git -C "$root" rev-parse --path-format=absolute --git-common-dir 2>/dev/null || true)
  if [[ -n "$common" && -f "$(dirname "$common")/.env" ]]; then echo "$(dirname "$common")/.env"; fi
}
file=$(env_file)
key="" status=0
if [[ -n "$file" ]]; then
  key=$(python3 -I "$root/app/scripts/read_env.py" --file "$file" --optional --raw ANTHROPIC_API_KEY) || status=$?
  if [[ $status -ne 0 && $status -ne 3 ]]; then
    echo "genui-dev: the .env could not be read (read_env.py exit $status); no model" >&2
    key=""
  fi
fi
if [[ -n "$key" ]]; then export ANTHROPIC_API_KEY="$key"; fi
unset key
exec "$target/debug/genui-dev" serve
