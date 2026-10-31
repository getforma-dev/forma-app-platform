#!/usr/bin/env bash
# The local dev stand (ADR-011 pitch): the whole product on 127.0.0.1 over a
# throwaway Docker PostgreSQL, signed in as three personas through loopback
# cookie proxies. See docs/pitch/GUIDE.md, "Local dev stand".
#
#   dev_stand.sh [start]        build, start the stand, start the proxies, print the URLs
#   dev_stand.sh stop           stop the proxies and the stand; remove its container
#   dev_stand.sh restart-server rebuild from this checkout, then restart (data is reseeded)
#   dev_stand.sh status         what is running, and the URLs
#   dev_stand.sh reset          a new stand from the last build (no rebuild): data reseeded
#
# GENUI_DEV_STAND_PROFILE=fresh (app/scripts/fresh_stand.sh) runs a second,
# separate stand: a new customer (Alex Rivera) on :4010 with the library and
# nothing else, whose browser storage is cleared after each reset.
#
# ANTHROPIC_API_KEY is read from the clone's .env (app/scripts/read_env.py)
# into the stand's environment only: never printed, never on a command line.
# Without it the stand runs with no model (rehearsed sentences only).
#
# Environment: CARGO_TARGET_DIR (default app/target), GENUI_DEV_STAND_DIR
# (state, default $TMPDIR/forma-dev-stand), GENUI_DEV_STAND_SECONDS (default
# 8 hours), GENUI_ENV_FILE (the .env to read), GENUI_BOX_PARSER_DIR (the
# sealed parser build; without it folders do not parse CSV/XLSX).
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
app=$(cd "$here/.." && pwd)
root=$(cd "$app/.." && pwd)
profile=${GENUI_DEV_STAND_PROFILE:-pitch}
if [[ "$profile" == fresh ]]; then
  state=${GENUI_DEV_STAND_DIR:-${TMPDIR:-/tmp}/forma-fresh-stand}
  personas=("alex:4010")
  proxy_flags=(--clear-on-reset)
else
  state=${GENUI_DEV_STAND_DIR:-${TMPDIR:-/tmp}/forma-dev-stand}
  personas=("diego:4001" "sarah:4002" "priya:4003")
  proxy_flags=()
fi
export GENUI_DEV_STAND_PROFILE=$profile
stand_file="$state/stand.json"
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$app/target}
test_name=storage::dev_stand::native_dev_stand

mkdir -p "$state"
chmod 700 "$state"

alive() { [[ -f "$1" ]] && kill -0 "$(cat "$1")" 2>/dev/null; }

urls() {
  if [[ "$profile" == fresh ]]; then
    echo "Forma fresh stand, a new customer (loopback only):"
    echo "  Alex Rivera      http://127.0.0.1:4010/canvas"
    echo "reset to a brand-new workspace: app/scripts/fresh_stand.sh reset"
    return
  fi
  echo "Forma dev stand (loopback only):"
  echo "  Diego Ramos      http://127.0.0.1:4001/canvas"
  echo "  Sarah Chen       http://127.0.0.1:4002/canvas"
  echo "  Priya Natarajan  http://127.0.0.1:4003/canvas"
}

build() {
  # Always rebuild the canvas bundle from this checkout: a stale dist served an
  # old canvas (2026-10-31).
  echo "building the canvas bundle (npm run build) ..."
  (cd "$app/admin" && npm run build >"$state/admin-build.log" 2>&1) || {
    echo "admin build failed; see $state/admin-build.log" >&2
    exit 1
  }
  [[ -f "$app/admin/dist/manifest.json" ]] || {
    echo "admin/dist is not built: cd $app/admin && npm ci && npm run build" >&2
    exit 1
  }
  echo "building the stand (cargo test --no-run, CARGO_TARGET_DIR=$CARGO_TARGET_DIR) ..."
  local exe
  exe=$(cd "$app" && cargo test --locked --lib --no-run --message-format=json 2>"$state/build.log" |
    python3 -I -c 'import json,sys
for line in sys.stdin:
    try: m = json.loads(line)
    except ValueError: continue
    if m.get("reason") == "compiler-artifact" and m.get("executable") and m["target"]["name"] == "genui" and "lib" in m["target"]["kind"]:
        print(m["executable"])') || true
  if [[ -z "$exe" || ! -x "$exe" ]]; then
    echo "build failed; see $state/build.log" >&2
    tail -n 30 "$state/build.log" >&2 || true
    exit 1
  fi
  # A private copy, so builds of other checkouts in a shared target
  # directory never replace the running stand's executable.
  cp "$exe" "$state/dev-stand-bin.new"
  mv "$state/dev-stand-bin.new" "$state/dev-stand-bin"
}

env_file() {
  if [[ -n "${GENUI_ENV_FILE:-}" ]]; then echo "$GENUI_ENV_FILE"; return; fi
  if [[ -f "$root/.env" ]]; then echo "$root/.env"; return; fi
  # A worktree: the main clone's .env.
  local common
  common=$(git -C "$root" rev-parse --path-format=absolute --git-common-dir 2>/dev/null || true)
  if [[ -n "$common" && -f "$(dirname "$common")/.env" ]]; then echo "$(dirname "$common")/.env"; fi
}

start_stand() {
  rm -f "$stand_file" "$stand_file.stop" "$state/container" "$state/scratch" "$state/stand.log"
  local file key="" status=0
  file=$(env_file)
  if [[ -n "$file" ]]; then
    key=$(python3 -I "$here/read_env.py" --file "$file" --optional --raw ANTHROPIC_API_KEY) || status=$?
    if [[ $status -ne 0 && $status -ne 3 ]]; then
      echo "the .env could not be read (read_env.py exit $status); starting without a model" >&2
      key=""
    fi
  fi
  if [[ -n "$key" ]]; then echo "model: ANTHROPIC_API_KEY from $file"; else echo "model: none (no ANTHROPIC_API_KEY in the .env)"; fi
  (
    cd "$app"
    if [[ -n "$key" ]]; then export ANTHROPIC_API_KEY="$key"; fi
    export GENUI_DEV_STAND_FILE="$stand_file"
    # The operator validates library bundles with contracts/validate.py
    # (jsonschema lives in the contracts environment).
    if [[ -z "${PYTHONPATH:-}" ]]; then
      for site in "$root"/contracts/.venv/lib/python*/site-packages; do
        if [[ -d "$site" ]]; then export PYTHONPATH="$site"; break; fi
      done
    fi
    export GENUI_DEV_STAND_SECONDS="${GENUI_DEV_STAND_SECONDS:-28800}"
    exec nohup "$state/dev-stand-bin" "$test_name" --exact --ignored --nocapture --test-threads=1
  ) >"$state/stand.log" 2>&1 &
  echo $! >"$state/stand.pid"
  key=""
  echo "seeding the stand (Docker PostgreSQL, 20-app library, personas) ..."
  local waited=0
  until [[ -f "$stand_file" ]]; do
    if ! alive "$state/stand.pid"; then
      echo "the stand exited; last lines of $state/stand.log:" >&2
      tail -n 40 "$state/stand.log" >&2
      cleanup_leftovers
      exit 1
    fi
    if (( waited >= 600 )); then
      echo "the stand did not come up in 10 minutes; see $state/stand.log" >&2
      stop_all
      exit 1
    fi
    sleep 1; waited=$((waited + 1))
  done
  grep -o -m1 'DEV_STAND container genui-native-[0-9a-f]*' "$state/stand.log" | awk '{print $3}' >"$state/container" || true
  grep -o 'DEV_STAND scratch [^ ]*' "$state/stand.log" | awk '{print $3}' >"$state/scratch" || true
  grep -o 'DEV_STAND ready .*' "$state/stand.log" || true
}

start_proxies() {
  local entry persona port
  for entry in "${personas[@]}"; do
    persona=${entry%%:*}; port=${entry##*:}
    if alive "$state/proxy-$persona.pid"; then continue; fi
    nohup python3 -I "$here/dev_stand_proxy.py" "$stand_file" "$persona" "$port" ${proxy_flags[@]+"${proxy_flags[@]}"} \
      >"$state/proxy-$persona.log" 2>&1 &
    echo $! >"$state/proxy-$persona.pid"
  done
  for entry in "${personas[@]}"; do
    port=${entry##*:}
    local tries=0
    until curl -fsS -o /dev/null "http://127.0.0.1:$port/health/live" 2>/dev/null; do
      tries=$((tries + 1))
      if (( tries > 50 )); then
        echo "proxy on :$port did not answer; see $state/proxy-*.log" >&2
        exit 1
      fi
      sleep 0.2
    done
  done
}

stop_proxies() {
  local entry persona
  for entry in "${personas[@]}"; do
    persona=${entry%%:*}
    if alive "$state/proxy-$persona.pid"; then kill "$(cat "$state/proxy-$persona.pid")" 2>/dev/null || true; fi
    rm -f "$state/proxy-$persona.pid"
  done
}

# Only what this stand recorded: its own container and private directories.
cleanup_leftovers() {
  if [[ -f "$state/stand.log" ]]; then
    [[ -s "$state/container" ]] || grep -o -m1 'DEV_STAND container genui-native-[0-9a-f]*' "$state/stand.log" | awk '{print $3}' >"$state/container" || true
    [[ -s "$state/scratch" ]] || grep -o 'DEV_STAND scratch [^ ]*' "$state/stand.log" | awk '{print $3}' >"$state/scratch" || true
  fi
  if [[ -s "$state/container" ]]; then
    local name
    name=$(cat "$state/container")
    if [[ "$name" =~ ^genui-native-[0-9a-f]+$ ]]; then docker rm --force --volumes "$name" >/dev/null 2>&1 || true; fi
  fi
  if [[ -s "$state/scratch" ]]; then
    local dir
    while read -r dir; do
      if [[ "$(basename "$dir")" =~ ^genui-(native|app012-dev-stand)-[0-9a-f]+$ && -d "$dir" ]]; then rm -rf "$dir"; fi
    done <"$state/scratch"
  fi
  rm -f "$state/container" "$state/scratch" "$stand_file" "$stand_file.stop" "$stand_file.new"
}

stop_stand() {
  if alive "$state/stand.pid"; then
    local pid
    pid=$(cat "$state/stand.pid")
    # Graceful: the stand sees the stop file, removes its container and exits.
    touch "$stand_file.stop"
    local waited=0
    while kill -0 "$pid" 2>/dev/null && (( waited < 60 )); do sleep 0.5; waited=$((waited + 1)); done
    if kill -0 "$pid" 2>/dev/null; then
      kill "$pid" 2>/dev/null || true
      sleep 2
      kill -9 "$pid" 2>/dev/null || true
    fi
  fi
  rm -f "$state/stand.pid"
  cleanup_leftovers
}

stop_all() {
  stop_proxies
  stop_stand
}

case "${1:-start}" in
  start)
    if alive "$state/stand.pid" && [[ -f "$stand_file" ]]; then
      start_proxies
      echo "already running"; urls; exit 0
    fi
    stop_all
    build
    start_stand
    start_proxies
    urls
    if [[ "$profile" == fresh ]]; then self="$here/fresh_stand.sh"; else self="$here/dev_stand.sh"; fi
    echo "logs: $state/stand.log; stop: $self stop"
    ;;
  stop)
    stop_all
    echo "stopped"
    ;;
  restart-server)
    # Build first, while the old stand still serves; then a fresh stand
    # (the database is reseeded: browser state on the canvas is kept by the
    # browser, server data is not). The proxies stay up and follow the new
    # stand file.
    build
    stop_stand
    start_stand
    start_proxies
    urls
    ;;
  reset)
    # A new stand from the last build: the database is reseeded (and, on the
    # fresh stand, the browser's storage is cleared on the next page load).
    [[ -x "$state/dev-stand-bin" ]] || build
    stop_stand
    start_stand
    start_proxies
    urls
    ;;
  status)
    if alive "$state/stand.pid" && [[ -f "$stand_file" ]]; then
      echo "stand: running (pid $(cat "$state/stand.pid"))"
      for entry in "${personas[@]}"; do
        persona=${entry%%:*}
        if alive "$state/proxy-$persona.pid"; then echo "proxy $persona: running"; else echo "proxy $persona: stopped"; fi
      done
      urls
    else
      echo "stand: not running"
    fi
    ;;
  *)
    echo "usage: $0 [start|stop|restart-server|reset|status]" >&2
    exit 2
    ;;
esac
