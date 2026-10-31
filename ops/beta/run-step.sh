#!/usr/bin/env bash
# One operator task in the window (the APP-078 operator-task run pattern; the
# runbook's 5.2, 5.3, 5.5). Replaces the other clone's run-step-pitch.sh, which
# hard-coded one window ($WIN_gov) and overwrote run-<step>.* on every run.
#
#   run-step.sh --window <id> [--approved "<words>"] [--dry-run] <label> <override> [window_requests args...]
#
# <override> is a documented entry of app/operator/overrides.json; extra args
# (--set=--flag=value, --add=--flag=value) go to window_requests.py
# operator-task, which composes the run-task request locally. Running the
# task is a [MUTATION]: refused without --approved; the words are appended to
# $PRIV/approvals.md first. Every run keeps its own files:
# $W/run-<label>-<n>.{json,compose.json,receipt.json,stopped.json,log.json}.
set -euo pipefail
# shellcheck source=ops/beta/lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

WINDOW_ID=""
while (( $# )); do
  case $1 in
    --window) WINDOW_ID=${2:?}; shift 2 ;;
    --approved) APPROVED=${2:?}; shift 2 ;;
    --dry-run) DRY_RUN=1; shift ;;
    -h|--help) sed -n '2,14p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) break ;;
  esac
done
[[ -n "$WINDOW_ID" ]] || die "--window <window id> is required"
(( $# >= 2 )) || die "usage: run-step.sh --window <id> [--approved <words>] <label> <override> [args...]"
label=$1 override=$2; shift 2
[[ "$label" =~ ^[a-z0-9-]+$ ]] || die "label: lowercase letters, digits and hyphens"
export DRY_RUN APPROVED WINDOW_ID

load_env; window_dir "$WINDOW_ID"; need_window_dir
[[ -f "$W/stack-after.json" ]] || (( DRY_RUN )) || die "no $W/stack-after.json (just deploy execute)"
n=1; while [[ -e "$W/run-$label-$n.json" ]]; do n=$((n + 1)); done
R="$W/run-$label-$n"
EXTRA=$(printf '%q ' "$@")

# Composing the request is local: window_requests.py calls no AWS API.
compose="python3 \"\$REPO/infra/scripts/window_requests.py\" operator-task --outputs \"\$W/stack-after.json\" --window-id \"\$WINDOW_ID\" --step $label --override $override $EXTRA--output \"\$R.json\" > \"\$R.compose.json\""
prepare "$compose"

mutate "run-step $label ($override)" '
aws ecs run-task --cli-input-json "file://$R.json" > "$R.receipt.json"
TASK=$(jq -r ".tasks[0].taskArn" "$R.receipt.json")
[ "$TASK" != null ] || { echo "no task started:" >&2; jq -c .failures "$R.receipt.json" >&2; exit 4; }
aws ecs wait tasks-stopped --cluster "$CLUSTER" --tasks "$TASK"
aws ecs describe-tasks --cluster "$CLUSTER" --tasks "$TASK" > "$R.stopped.json"
aws logs get-log-events --log-group-name "$LOGS" --log-stream-name "genui-operator/operator/${TASK##*/}" --start-from-head > "$R.log.json" || true
echo "step='"$label"' run='"$n"' exit=$(jq -r ".tasks[0].containers[0].exitCode" "$R.stopped.json") stop=$(jq -r ".tasks[0].stopCode + \" \" + (.tasks[0].stoppedReason // \"\")" "$R.stopped.json")"
jq -r ".events[].message" "$R.log.json" 2>/dev/null | cut -c1-1500'

# ADR-013 D13.5: the run in the window's release record (label, run number,
# the container's exit code); a migrate that exited 0 marks the migration.
if (( ! DRY_RUN )) && [[ -f "$R.stopped.json" && -f "$W/release.json" ]]; then
  code=$(jq -r '.tasks[0].containers[0].exitCode // "null"' "$R.stopped.json")
  release_set '.operator_tasks += [{label: $l, run: ($n | tonumber), override: $o, exit_code: ($c | fromjson), at_utc: (now | floor | todate)}]
               | if $l == "migrate" and $c == "0" then .migrations.migrated = true else . end' \
    --arg l "$label" --arg n "$n" --arg o "$override" --arg c "$code"
fi
