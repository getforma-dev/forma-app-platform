#!/usr/bin/env bash
# just dev / restart / status (ADR-012 D12.1, D12.2): this checkout's dev
# instances, supervised by process-compose (process-compose.yaml) over a
# Compose PostgreSQL (compose.dev.yaml), served by the genui-dev binary.
#
#   dev.sh [clean] [start] [-d]   start (foreground: the TUI on a terminal,
#                                 plain logs otherwise; -d detaches and waits
#                                 until the personas answer)
#   dev.sh [clean] stop           stop every process; removes the container,
#                                 keeps the data volume
#   dev.sh [clean] reset          stop, drop the volume and the state, start
#                                 again (-d as for start)
#   dev.sh [clean] restart        rebuild and restart the server only (data
#                                 kept); also `restart-server`
#   dev.sh [clean] status         what runs, and the URLs
#   dev.sh [clean] logs           follow the server's log (all processes'
#                                 logs: .cache/dev/<profile>/logs/)
#   dev.sh clean seed-library     install the template apps for the clean
#                                 customer (operator seed-library +
#                                 grant-library-access, audited)
#
# Without `clean`: the pitch instance (Northstar, Diego/Sarah/Priya on port
# base +1..+3). `clean`: a new customer, Alex Rivera, on port base +10, no
# library. Ports, Compose project and state are this checkout's own
# (scripts/dev/env.sh), so other checkouts' instances are never touched.
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
profile=pitch
if [[ "${1:-}" == clean ]]; then profile=clean; shift; fi
detach=0 cmd=""
for a in "$@"; do
  case $a in
    -d|--detach) detach=1 ;;
    start|stop|reset|restart|restart-server|status|logs|seed-library)
      [[ -z "$cmd" ]] || { echo "one command at a time" >&2; exit 2; }
      cmd=$a ;;
    *) echo "usage: just dev [clean] [start|stop|reset|status|logs|seed-library] [-d]; just restart [clean]; just status [clean]" >&2; exit 2 ;;
  esac
done
cmd=${cmd:-start}

suffix=""
[[ $profile == clean ]] && suffix=" clean"
# shellcheck source=scripts/dev/env.sh
source "$here/env.sh" "$profile"
root=$GENUI_DEV_ROOT
cd "$root"

need() { command -v "$1" >/dev/null || { echo "$1 is missing: $2 (version in tools.versions)" >&2; exit 1; }; }
need process-compose "brew install f1bonacc1/tap/process-compose"
need docker "Docker Desktop"

# process-compose logs a debug line about its (absent) user config on every
# call; it is noise, so it is dropped (other stderr passes through).
quiet() { grep -v 'process compose config home' >&2 || true; }
pc() { process-compose "$@" --port "$GENUI_DEV_PC_PORT" 2> >(quiet); }
# `up` keeps its own streams: a detached supervisor must not hold a pipe of
# ours (it would keep the caller's pipeline open for its whole life).
pc_up() { process-compose up -f "$root/process-compose.yaml" --port "$GENUI_DEV_PC_PORT" "$@"; }
running() { pc process list >/dev/null 2>&1; }
compose() { docker compose -f "$root/compose.dev.yaml" "$@"; }

urls() {
  local p name port
  echo "genui-dev $profile instance (loopback only; port base $GENUI_DEV_PORT_BASE):"
  for p in "${GENUI_DEV_PERSONAS[@]}"; do
    name=${p#*:}; name=${name%:*}; port=${p##*:}
    printf '  %-16s http://127.0.0.1:%s/canvas\n' "$name" "$port"
  done
}

# Refuse to start on ports something else holds (another checkout with the
# same base, or an old app/scripts/dev_stand.sh stand), naming the holder.
guard_ports() {
  local port holder busy=0
  for port in ${GENUI_DEV_PERSONA_PORTS//,/ } "$GENUI_DEV_DB_PORT" "$GENUI_DEV_PC_PORT"; do
    holder=$(lsof -nP -iTCP:"$port" -sTCP:LISTEN -Fcp 2>/dev/null | tr '\n' ' ' || true)
    if [[ -n "$holder" ]]; then
      echo "refused: port $port is in use ($holder); it is not this checkout's $profile instance." >&2
      busy=1
    fi
  done
  if (( busy )); then
    echo "Stop that process from the checkout that started it, or set another GENUI_DEV_PORT_BASE in $root/.env.dev." >&2
    exit 3
  fi
}

wait_ready() {
  local waited=0 port
  echo "waiting for the server (the first build of a checkout takes minutes; log: $GENUI_DEV_STATE/logs/server.log) ..."
  for port in ${GENUI_DEV_PERSONA_PORTS//,/ }; do
    until curl -fsS -o /dev/null "http://127.0.0.1:$port/health/live" 2>/dev/null; do
      if ! running; then echo "the supervisor stopped; see $GENUI_DEV_STATE/logs/" >&2; exit 1; fi
      if (( waited >= 1800 )); then echo "not ready after 30 minutes; see $GENUI_DEV_STATE/logs/server.log" >&2; exit 1; fi
      sleep 1; waited=$((waited + 1))
    done
  done
}

start() {
  if running; then echo "already running"; urls; return; fi
  guard_ports
  mkdir -p "$GENUI_DEV_STATE/logs"
  chmod 700 "$root/.cache/dev" "$GENUI_DEV_STATE"
  if (( detach )); then
    pc_up --detached >/dev/null 2>&1
    wait_ready
    urls
    echo "stop: just dev$suffix stop"
  else
    urls
    if [[ -t 1 ]]; then
      pc_up
    else
      pc_up --tui=false
    fi
  fi
}

stop() {
  if running; then pc down >/dev/null 2>&1 || true; fi
  local waited=0
  while running && (( waited < 60 )); do sleep 1; waited=$((waited + 1)); done
  # Whatever the supervisor left (a crash): this instance's container only.
  compose down >/dev/null 2>&1 || true
}

case $cmd in
  start) start ;;
  stop) stop; echo "stopped ($profile; the data volume is kept)" ;;
  reset)
    stop
    compose down --volumes >/dev/null 2>&1 || true
    rm -rf "${GENUI_DEV_STATE:?}"
    echo "reset: the $profile database and state are gone; starting a new one"
    start
    ;;
  restart|restart-server)
    running || { echo "not running: just dev$suffix" >&2; exit 1; }
    pc process restart server >/dev/null
    echo "server restarting (rebuilds if the Rust code changed; data kept)"
    ;;
  status)
    if running; then
      pc process list --output wide
      urls
    else
      echo "$profile: not running"
      if docker volume inspect "${GENUI_DEV_PROJECT}_pgdata" >/dev/null 2>&1; then
        echo "  data volume ${GENUI_DEV_PROJECT}_pgdata is kept"
      fi
    fi
    ;;
  logs)
    running || { echo "not running" >&2; exit 1; }
    pc process logs server --follow
    ;;
  seed-library)
    running || { echo "not running: start it first (just dev$suffix -d)" >&2; exit 1; }
    (cd "$root/app" && cargo build --locked --features dev-stand --bin genui-dev)
    "${CARGO_TARGET_DIR:-$root/app/target}/debug/genui-dev" seed-library
    ;;
esac
