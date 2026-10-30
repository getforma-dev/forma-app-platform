#!/bin/sh
# APP-093: a one-shot local smoke of the composer's model path against the
# real endpoint, with the cheapest setting (the Haiku model for the router
# and for generation). The key is loaded from the repository's gitignored
# `.env` (`ANTHROPIC_API_KEY=...`) into this process's environment only; it is
# never printed, logged, passed on argv or written anywhere. Without that
# file (or with no `ANTHROPIC_API_KEY` line in it) the smoke is skipped.
# The service itself never reads a file: it takes the key from its
# environment alone (the ECS secret in deployment).
set -eu
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH= cd -- "$here/../.." && pwd)
env_file=${GENUI_ENV_FILE:-$root/.env}
if [ ! -f "$env_file" ]; then
  echo "composer smoke skipped: no $env_file" >&2
  exit 0
fi
# Only the one variable, only from that file, through the one .env reader
# (app/scripts/read_env.py, APP-113: strips quotes, CR and `export`; refuses a
# duplicate line). Exit 3 means missing or empty: skip.
status=0
key=$(python3 -I "$here/read_env.py" --file "$env_file" --optional --raw ANTHROPIC_API_KEY) || status=$?
if [ "$status" -eq 3 ]; then
  echo "composer smoke skipped: no ANTHROPIC_API_KEY line in $env_file" >&2
  exit 0
elif [ "$status" -ne 0 ]; then
  exit "$status"
fi
export ANTHROPIC_API_KEY="$key"
unset key
export GENUI_COMPOSER_ROUTER_MODEL=${GENUI_COMPOSER_ROUTER_MODEL:-claude-haiku-4-5-20251001}
export GENUI_COMPOSER_GENERATION_MODEL=${GENUI_COMPOSER_GENERATION_MODEL:-claude-haiku-4-5-20251001}
cd "$root/app"
# APP-101: `composer_smoke.sh transform` runs the live-transform smoke
# (the pitch sentences, decisions and latencies only) instead.
case "${1:-composer}" in
  transform) test=composer::transform::smoke::real_haiku_transform_smoke ;;
  *) test=composer::tests::real_model_smoke ;;
esac
exec cargo +1.93.1 test --locked --lib "$test" -- --ignored --exact --nocapture
