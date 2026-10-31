#!/usr/bin/env bash
# just deploy verify (DEPLOY-RUNBOOK 6.2): the public HTTPS surface, checked
# with plain curl (no -k, no AWS API, no credentials).
#
#   verify.sh [--origin https://beta.getforma.dev] [--dry-run]
#
#   /health/live            200
#   /canvas signed out      the sign-in page (200 "Sign in", or a redirect to /signin)
#   /v1/mcp without bearer  the uniform 404
#   /v1/slack/commands      401 unsigned (Slack on) or 404 (Slack off)
set -euo pipefail

origin=https://beta.getforma.dev dry=0
while (( $# )); do
  case $1 in
    --origin) origin=${2:?}; shift 2 ;;
    --dry-run) dry=1; shift ;;
    -h|--help) sed -n '2,11p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown option $1" >&2; exit 2 ;;
  esac
done
[[ "$origin" == https://* ]] || { echo "the origin must be https (no -k)" >&2; exit 2; }

body=$(mktemp); trap 'rm -f "$body"' EXIT
fail=0
probe() { # method path -> "code location"
  curl -sS --max-time 20 -o "$body" -X "$1" -w '%{http_code} %{redirect_url}' "$origin$2" ${3:+-H "$3"} ${4:+--data "$4"}
}
check() {
  local label=$1 ok=$2 got=$3
  if [[ $ok == 1 ]]; then printf '  ok    %-30s %s\n' "$label" "$got"; else printf '  FAIL  %-30s %s\n' "$label" "$got"; fail=1; fi
}

if (( dry )); then
  echo "would check $origin: GET /health/live, GET /canvas, POST /v1/mcp, POST /v1/slack/commands"
  exit 0
fi

echo "verify $origin (runbook 6.2)"
r=$(probe GET /health/live); check "/health/live 200" "$([[ ${r%% *} == 200 ]] && echo 1)" "$r"
r=$(probe GET /canvas); code=${r%% *} loc=${r#* }
signin=0
[[ $code == 200 ]] && grep -qi 'sign in' "$body" && signin=1
[[ $code == 30[1-8] && $loc == *signin* ]] && signin=1
check "/canvas signed out: sign-in" "$signin" "$r"
r=$(probe POST /v1/mcp "Content-Type: application/json" '{}'); check "/v1/mcp no bearer: 404" "$([[ ${r%% *} == 404 ]] && echo 1)" "$r"
r=$(probe POST /v1/slack/commands "Content-Type: application/x-www-form-urlencoded" 'text=x'); code=${r%% *}
check "/v1/slack/commands: 401|404" "$([[ $code == 401 || $code == 404 ]] && echo 1)" "$r ($([[ $code == 401 ]] && echo 'Slack on' || echo 'Slack off'))"
exit $fail
