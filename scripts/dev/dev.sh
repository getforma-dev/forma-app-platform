#!/usr/bin/env bash
# just dev / restart / status (ADR-012 D12.3), wrapping the existing stand
# scripts until plan step 4 replaces them with the genui-dev binary and
# process-compose.
#
#   dev.sh [clean] [start|stop|reset|status|restart-server] [-d]
#
# `clean` is the clean-slate instance (today app/scripts/fresh_stand.sh, :4010);
# without it, the pitch instance (app/scripts/dev_stand.sh, :4001-4003).
# -d is accepted for the step-4 interface; the current stand always detaches.
#
# Until step 4 a stand's state lives machine-wide in $TMPDIR, not per checkout,
# so a stand started from one checkout looks like "the" stand to every other.
# This wrapper refuses to start, stop, reset or restart a stand that another
# checkout started (`status` is always allowed).
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
instance=pitch
if [[ "${1:-}" == clean ]]; then instance=clean; shift; fi
args=()
for a in ${@+"$@"}; do [[ "$a" == -d ]] || args+=("$a"); done
cmd=${args[0]:-start}

case "$cmd" in
  start|stop|reset|status|restart-server) ;;
  *) echo "usage: just dev [clean] [start|stop|reset|status]; just restart [clean]; just status [clean]" >&2; exit 2 ;;
esac

if [[ $instance == clean ]]; then
  script="$root/app/scripts/fresh_stand.sh"
  state=${GENUI_DEV_STAND_DIR:-${TMPDIR:-/tmp}/forma-fresh-stand}
else
  script="$root/app/scripts/dev_stand.sh"
  state=${GENUI_DEV_STAND_DIR:-${TMPDIR:-/tmp}/forma-dev-stand}
fi

if [[ $cmd != status && -f "$state/stand.pid" ]]; then
  pid=$(cat "$state/stand.pid")
  if kill -0 "$pid" 2>/dev/null; then
    owner=$(lsof -a -p "$pid" -d cwd -Fn 2>/dev/null | sed -n 's/^n//p' | head -1)
    if [[ -n "$owner" && "$owner" != "$root/app" ]]; then
      echo "refused: the $instance stand (pid $pid) was started from ${owner%/app}, not this checkout." >&2
      echo "Its state is machine-wide until ADR-012 step 4; run this from that checkout." >&2
      exit 3
    fi
  fi
fi

exec "$script" ${args[@]+"${args[@]}"}
