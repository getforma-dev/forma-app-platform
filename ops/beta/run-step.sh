#!/usr/bin/env bash
# One operator task in the window (the APP-078 operator-task run pattern; the
# runbook's 5.2, 5.3, 5.5). Replaces the other clone's run-step-pitch.sh, which
# hard-coded one window ($WIN_gov) and overwrote run-<step>.* on every run.
#
#   run-step.sh --window <id> [--approved "<words>"] [--dry-run] [--resume-run <n>] <label> <override> [window_requests args...]
#
# <override> is a documented entry of app/operator/overrides.json; extra args
# (--set=--flag=value, --add=--flag=value) go to window_requests.py
# operator-task, which composes the run-task request locally. Running the
# task is a [MUTATION]: refused without --approved; the words are appended to
# $PRIV/approvals.md first. Every run keeps its own files:
# $W/run-<label>-<n>.{json,compose.json,receipt.json,stopped.json,log.json}.
# A resume waits for the original known task; it never calls RunTask again.
# Unknown RunTask acceptance requires reconciliation before another run.
set -euo pipefail
# shellcheck source=ops/beta/lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

WINDOW_ID="" RESUME_RUN=""
while (( $# )); do
  case $1 in
    --window) WINDOW_ID=${2:?}; shift 2 ;;
    --approved) APPROVED=${2:?}; shift 2 ;;
    --dry-run) DRY_RUN=1; shift ;;
    --resume-run) RESUME_RUN=${2:?}; shift 2 ;;
    -h|--help) sed -n '2,15p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) break ;;
  esac
done
[[ -n "$WINDOW_ID" ]] || die "--window <window id> is required"
(( $# >= 2 )) || die "usage: run-step.sh --window <id> [--approved <words>] <label> <override> [args...]"
label=$1 override=$2; shift 2
[[ "$label" =~ ^[a-z0-9-]{1,35}$ ]] || die "label: 1–35 lowercase letters, digits and hyphens"
[[ "$override" =~ ^[a-z0-9-]+$ ]] || die "override: lowercase letters, digits and hyphens"
export DRY_RUN APPROVED WINDOW_ID

load_env; window_dir "$WINDOW_ID"; need_window_dir
[[ -f "$W/stack-after.json" ]] || (( DRY_RUN )) || die "no $W/stack-after.json (just deploy execute)"
if [[ -n "$RESUME_RUN" ]]; then
  [[ "$RESUME_RUN" =~ ^[1-9][0-9]{0,3}$ ]] || die "--resume-run must be 1–9999"
  (( $# == 0 )) || die "resume cannot change the frozen request's arguments"
  n=$RESUME_RUN
  BASE="$W/run-$label-$n"
  [[ -f "$BASE.json" && -f "$BASE.compose.json" ]] || die "no original request for run $n"
  [[ "$(jq -r .override "$BASE.compose.json")" == "$override" ]] || die "resume override differs from the original run"
  expected=$(jq -er .sha256 "$BASE.compose.json")
  actual=$(python3 -c 'import hashlib,sys; print(hashlib.sha256(open(sys.argv[1], "rb").read()).hexdigest())' "$BASE.json")
  [[ "$actual" == "$expected" ]] || die "original request changed; reconcile run $n before continuing"
  TASK=$(jq -er '.tasks[0].taskArn | select(type == "string" and startswith("arn:aws:ecs:") and contains(":task/"))' "$BASE.receipt.json" 2>/dev/null) \
    || die "RunTask acceptance is unknown; reconcile the original request/task before resuming or starting a new run"
  attempt=1; while [[ -e "$BASE-resume-$attempt.receipt.json" ]]; do attempt=$((attempt + 1)); done
  R="$BASE-resume-$attempt"
else
  n=1
  while [[ -e "$W/run-$label-$n.json" ]]; do
    jq -e --arg l "$label" --argjson n "$n" \
      'any(.operator_tasks[]; .label == $l and .run == $n and (.exit_code | type) == "number")' "$W/release.json" >/dev/null \
      || die "run $label/$n is unresolved; use --resume-run $n for its known task or reconcile unknown RunTask acceptance before a new run"
    n=$((n + 1))
  done
  (( n <= 9999 )) || die "run limit reached for $label"
  BASE="$W/run-$label-$n" R="$W/run-$label-$n"
  EXTRA=""
  if (( $# )); then EXTRA=$(printf '%q ' "$@"); fi
  # One logical run gets one token. AWS CLI retries reuse this exact request;
  # a later confirmed run uses a new label-number identity.
  compose="python3 \"\$REPO/infra/scripts/window_requests.py\" operator-task --outputs \"\$W/stack-after.json\" --window-id \"\$WINDOW_ID\" --step $label-$n --override $override $EXTRA--output \"\$BASE.json\" > \"\$BASE.compose.json\"
jq --arg o $override '. + {override: \$o}' \"\$BASE.compose.json\" > \"\$BASE.compose.json.new\" && mv \"\$BASE.compose.json.new\" \"\$BASE.compose.json\""
  prepare "$compose"
fi

mutate "run-step $label ($override)" '
if [[ -n "$RESUME_RUN" ]]; then
  cp "$BASE.receipt.json" "$R.receipt.json"
else
  aws ecs run-task --cli-input-json "file://$BASE.json" > "$R.receipt.json"
  TASK=$(jq -r ".tasks[0].taskArn" "$R.receipt.json")
fi
[ "$TASK" != null ] || { echo "no task started:" >&2; jq -c .failures "$R.receipt.json" >&2; exit 4; }
aws ecs wait tasks-stopped --cluster "$CLUSTER" --tasks "$TASK"
aws ecs describe-tasks --cluster "$CLUSTER" --tasks "$TASK" > "$R.stopped.json"
aws logs get-log-events --log-group-name "$LOGS" --log-stream-name "genui-operator/operator/${TASK##*/}" --start-from-head > "$R.log.json" || true
echo "step='"$label"' run='"$n"' exit=$(jq -r ".tasks[0].containers[0].exitCode" "$R.stopped.json") stop=$(jq -r ".tasks[0].stopCode + \" \" + (.tasks[0].stoppedReason // \"\")" "$R.stopped.json")"
jq -r ".events[].message" "$R.log.json" 2>/dev/null | cut -c1-1500 || true'

# ADR-013 D13.5: the run in the window's release record (label, run number,
# the container's exit code); a migrate that exited 0 marks the migration.
if (( ! DRY_RUN )) && [[ -f "$R.stopped.json" && -f "$W/release.json" ]]; then
  code=$(jq -c '.tasks[0].containers[0].exitCode | if type == "number" then . else null end' "$R.stopped.json")
  release_set '.operator_tasks += [{label: $l, run: ($n | tonumber), override: $o, exit_code: ($c | fromjson), at_utc: (now | floor | todate)}]
               | if $l == "migrate" and $c == "0" then .migrations.migrated = true else . end' \
    --arg l "$label" --arg n "$n" --arg o "$override" --arg c "$code"
  # Record the stopped outcome first, then stop the caller on anything other
  # than a confirmed numeric zero. In particular, migrate must not continue
  # to verify-catalog after a failed task or a missing container exit status.
  [[ "$code" == 0 ]] || die "operator task $label/$n did not exit successfully (exit=$code; receipt: $R.stopped.json)"
fi
