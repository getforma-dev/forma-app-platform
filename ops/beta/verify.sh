#!/usr/bin/env bash
# just deploy verify (DEPLOY-RUNBOOK 6.2): the public HTTPS surface, checked
# with plain curl (no -k, no AWS API, no credentials).
#
#   verify.sh [--origin https://beta.getforma.dev] [--json <file>] [--dry-run]
#
# --json writes {"live","canvas","entry","mcp","slack": true|false, "slack_mode"}
# (`just deploy verify` records it in the window's release.json).
#
#   /health/live            200
#   /canvas signed out      the uniform 404
#   /invite                 200 with the public invitation form
#   /v1/mcp without bearer  the uniform 404
#   /v1/slack/commands      401 unsigned (Slack on) or 404 (Slack off)
set -euo pipefail

origin=https://beta.getforma.dev dry=0 json=""
while (( $# )); do
  case $1 in
    --origin) origin=${2:?}; shift 2 ;;
    --json) json=${2:?}; shift 2 ;;
    --dry-run) dry=1; shift ;;
    -h|--help) sed -n '2,15p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown option $1" >&2; exit 2 ;;
  esac
done
[[ "$origin" == https://* ]] || { echo "the origin must be https (no -k)" >&2; exit 2; }

body=$(mktemp); trap 'rm -f "$body"' EXIT
fail=0
probe() { # method path -> "code location"
  curl -sS --max-time 20 -o "$body" -X "$1" -w '%{http_code} %{redirect_url}' "$origin$2" ${3:+-H "$3"} ${4:+--data "$4"}
}
# Results by key (bash 3.2 has no associative arrays).
r_live=false r_canvas=false r_entry=false r_mcp=false r_slack=false
check() {
  local label=$1 ok=$2 got=$3 key=${4:-}
  if [[ -n $key ]]; then eval "r_$key=$([[ $ok == 1 ]] && echo true || echo false)"; fi
  if [[ $ok == 1 ]]; then printf '  ok    %-30s %s\n' "$label" "$got"; else printf '  FAIL  %-30s %s\n' "$label" "$got"; fail=1; fi
}

if (( dry )); then
  echo "would check $origin: GET /health/live, GET /canvas, GET /invite, POST /v1/mcp, POST /v1/slack/commands"
  exit 0
fi

echo "verify $origin (runbook 6.2)"
r=$(probe GET /health/live); check "/health/live 200" "$([[ ${r%% *} == 200 ]] && echo 1)" "$r" live
r=$(probe GET /canvas); canvas=0
[[ ${r%% *} == 404 ]] && jq -e '. == {code:"not_found",status:404}' "$body" >/dev/null 2>&1 && canvas=1
check "/canvas signed out: uniform404" "$canvas" "$r" canvas
r=$(probe GET /invite); entry=0
[[ ${r%% *} == 200 ]] && grep -q 'id="invite-code"' "$body" && grep -q 'action="/invite/validate"' "$body" && entry=1
check "/invite: public entry" "$entry" "$r" entry
r=$(probe POST /v1/mcp "Content-Type: application/json" '{}'); check "/v1/mcp no bearer: 404" "$([[ ${r%% *} == 404 ]] && echo 1)" "$r" mcp
r=$(probe POST /v1/slack/commands "Content-Type: application/x-www-form-urlencoded" 'text=x'); code=${r%% *}
check "/v1/slack/commands: 401|404" "$([[ $code == 401 || $code == 404 ]] && echo 1)" "$r ($([[ $code == 401 ]] && echo 'Slack on' || echo 'Slack off'))" slack
if [[ -n $json ]]; then
  printf '{"live":%s,"canvas":%s,"entry":%s,"mcp":%s,"slack":%s,"slack_mode":"%s"}\n' "$r_live" "$r_canvas" "$r_entry" \
    "$r_mcp" "$r_slack" "$([[ $code == 401 ]] && echo on || echo off)" > "$json"
fi
exit $fail
