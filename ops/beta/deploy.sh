#!/usr/bin/env bash
# just deploy <step> <window> [options] (ADR-012 D12.6): the beta redeploy of
# docs/pitch/DEPLOY-RUNBOOK.md, one step per call, every file of the window in
# $PRIV/windows/<window-id>/. The runbook stays the authority; each of its
# items names the step that performs it.
#
# Options (any step):
#   --approved "<words>"  Victor's verbatim word for THIS action. Required by
#                         every [MUTATION] step; appended to $PRIV/approvals.md
#                         before any AWS call. Never reused across steps.
#   --dry-run             print what would run; call nothing, record nothing
#
# Steps ([read] calls only read APIs; [local] calls no AWS API):
#   new        [local]    --candidate <sha> --supersedes <live window id> [--input-from <file>]
#                         makes the window id and directory and release.json (0.1, 0.4)
#   observe    [read]     caller identity, read-only snapshot, stack-before.json, template-before.json (1.1-1.3)
#   push       [MUTATION] tag and push the three images, record their digests (2.1-2.2)
#   preflight  [local]    deployment-input.json from the superseded window, preflight.py at count 0 (3.1)
#   abandon    [local]    preflight.py --abandon this window (Rollback, before 3.6)
#   synth      [local]    cdk synth with the private names files; template.json + sha256 (3.2)
#   diff       [local]    cdk diff against template-before.json, no AWS call (3.3)
#   change-set [MUTATION] compose, upload the template, create the change set (3.4.1-3.4.3)
#   review     [read]     describe the change set and review it against the receipt (3.4.4)
#   execute    [MUTATION] execute the change set; stack-after.json; the outage starts (3.6)
#   snapshot   [MUTATION] --name <snapshot id>: RDS snapshot before a migration (5.1)
#   migrate    [MUTATION] operator task migrate, then verify-catalog (5.2); run it twice
#   seed       [MUTATION] operator task seed-library; extra args after -- (5.3)
#   library    [MUTATION] [--organization <id>] build and plan the sources, put the objects, If-None-Match (5.4)
#   grants     [MUTATION] operator task grant-library-access; extra args after -- (5.5)
#   activate   [MUTATION] preflight.py --activate at count 1 (6.1)
#   verify     [read]     the four HTTPS checks of 6.2 (no AWS API)
#   rollback-deny [MUTATION] the service onto the deny-all revision; --restore puts it back (Rollback)
#   help       this text
#
# Secrets (runbook section 4) stay Victor's own commands; no step reads .env.
set -euo pipefail
# shellcheck source=ops/beta/lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

usage() { sed -n '2,/^set -euo/p' "${BASH_SOURCE[0]}" | sed '$d; s/^# \{0,1\}//'; }

step=${1:-help}; shift || true
case $step in help|-h|--help) usage; exit 0 ;; esac

WINDOW_ID="" organization="" candidate="" supersedes="" input_from="" snap_name="" restore=0 extra=()
if [[ $step != verify && $step != new ]]; then
  [[ $# -gt 0 && $1 != --* ]] || die "usage: just deploy $step <window-id> [options] (just deploy help)"
  WINDOW_ID=$1; shift
fi
while (( $# )); do
  case $1 in
    --approved) APPROVED=${2:?--approved needs Victor\'s words}; shift 2 ;;
    --approved=*) APPROVED=${1#*=}; shift ;;
    --dry-run) DRY_RUN=1; shift ;;
    --candidate) candidate=$2; shift 2 ;;
    --supersedes) supersedes=$2; shift 2 ;;
    --input-from) input_from=$2; shift 2 ;;
    --name) snap_name=$2; shift 2 ;;
    --restore) restore=1; shift ;;
    --organization) organization=$2; shift 2 ;;
    --) shift; extra=("$@"); break ;;
    *) die "unknown option $1 (just deploy help)" ;;
  esac
done
[[ -n "$APPROVED" && -z "${APPROVED//[[:space:]]/}" ]] && die "--approved is blank"
export DRY_RUN APPROVED WINDOW_ID

# The three images: local name, ECR repository.
IMAGES="service:genui-synthetic qualification:genui-qualification operator:genui-operator"

case $step in
  new)
    priv_ready
    [[ "$candidate" =~ ^[0-9a-f]{7,40}$ ]] || die "--candidate <commit sha> is required"
    [[ -n "$supersedes" ]] || die "--supersedes <live window id> is required (the head of the chain)"
    full=$(git -C "$REPO" rev-parse --verify "$candidate^{commit}") || die "unknown commit $candidate"
    git -C "$REPO" merge-base --is-ancestor "$full" "@{upstream}" 2>/dev/null || echo "note: $candidate is not on this branch's upstream (runbook 0.1: pushed)" >&2
    WINDOW_ID=$(uuidgen | tr 'A-Z' 'a-z')
    window_dir "$WINDOW_ID"
    if (( DRY_RUN )); then echo "would create $W and release.json for candidate ${full:0:12}"; exit 0; fi
    mkdir -m 700 -p "$W"
    if [[ -n "$input_from" ]]; then
      [[ -f "$input_from" ]] || die "no file $input_from"
      cp "$input_from" "$W/base-input.json"
    elif [[ -f "$PRIV/windows/$supersedes/deployment-input.json" ]]; then
      cp "$PRIV/windows/$supersedes/deployment-input.json" "$W/base-input.json"
    else
      die "no input for window $supersedes under $PRIV/windows; pass --input-from <its deployment input> (for example under windows/_pre-adr-012/)"
    fi
    jq -n --arg w "$WINDOW_ID" --arg c "$full" --arg s "$supersedes" \
      '{window_id:$w, candidate:$c, tag:("pitch-" + $c[0:7] + "-arm64"), supersedes_window_id:$s,
        digests:{}, created_at:(now|todate), updated_at:(now|todate)}' > "$W/release.json"
    echo "window $WINDOW_ID"
    echo "directory $W"
    ;;

  observe)
    load_env; window_dir "$WINDOW_ID"; need_window_dir
    shown '
aws sts get-caller-identity --query Account --output text
"$PY" "$REPO/infra/scripts/readonly_snapshot.py" --private-dir "$W/snapshot" --summary "$W/snapshot-summary.json"
aws cloudformation describe-stacks --stack-name "$STACK" > "$W/stack-before.json"
aws cloudformation get-template --stack-name "$STACK" --template-stage Original --query TemplateBody > "$W/template-before.json"'
    echo "check: the account, UPDATE_COMPLETE, the service 1/1 on the live revision, no pending change sets (runbook 1.1-1.2)"
    ;;

  push)
    load_env; window_dir "$WINDOW_ID"; need_window_dir
    C=$(release_get .candidate); C7=${C:0:7}; T=$(release_get .tag)
    for p in $IMAGES; do docker image inspect "genui-${p%%:*}:pitch-$C7" >/dev/null 2>&1 || (( DRY_RUN )) || die "local image genui-${p%%:*}:pitch-$C7 missing (just images $C7)"; done
    mutate push '
aws ecr get-login-password | docker login --username AWS --password-stdin "$REG" >/dev/null
for p in $IMAGES; do
  n=${p%%:*}; r=${p#*:}
  docker tag "genui-$n:pitch-$C7" "$REG/$r:$T"
  ok=0; for i in 1 2 3 4 5 6; do docker push -q "$REG/$r:$T" > "$W/push-$r.log" 2>&1 && { ok=1; break; }; done
  [ $ok = 1 ] || { echo "push failed: $r (see $W/push-$r.log)" >&2; exit 2; }
  aws ecr describe-images --repository-name "$r" --image-ids imageTag="$T" > "$W/ecr-$r.json"
  d=$(jq -r ".imageDetails[0].imageDigest" "$W/ecr-$r.json")
  release_set ".digests[\$r] = \$d" --arg r "$r" --arg d "$d"
  echo "pushed $r $d"
done'
    echo "next: read the scan findings from each arm64 child digest (runbook 2.3); a CRITICAL is Victor's call"
    ;;

  preflight)
    load_env; window_dir "$WINDOW_ID"; need_window_dir
    for r in genui-synthetic genui-qualification genui-operator; do
      [[ -n "$(release_get ".digests[\"$r\"] // empty")" ]] || (( DRY_RUN )) || die "no digest for $r in release.json (just deploy push)"
    done
    if (( ! DRY_RUN )); then
      jq --arg w "$WINDOW_ID" --arg sup "$(release_get .supersedes_window_id)" \
         --arg s "$REG/genui-synthetic@$(release_get '.digests["genui-synthetic"]')" \
         --arg q "$REG/genui-qualification@$(release_get '.digests["genui-qualification"]')" \
         --arg o "$REG/genui-operator@$(release_get '.digests["genui-operator"]')" \
         '.window_id=$w | .supersedes_window_id=$sup | .image_digest=$s | .qualification_image_digest=$q | .operator_image_digest=$o | .service_desired_count=0' \
         "$W/base-input.json" > "$W/deployment-input.json"
      echo "deployment-input.json written; deployment_expires_at_utc kept from the base input: $(jq -r .deployment_expires_at_utc "$W/deployment-input.json") (renew it if it is less than a day away, runbook 3.1)"
    fi
    shown '
python3 "$REPO/infra/scripts/preflight.py" --input "$W/deployment-input.json" --ledger "$PRIV/budget-ledger.json" > "$W/receipt.json"
jq -c "{status, window_id, superseded_window_id}" "$W/receipt.json"'
    ;;

  abandon)
    load_env; window_dir "$WINDOW_ID"
    shown 'python3 "$REPO/infra/scripts/preflight.py" --ledger "$PRIV/budget-ledger.json" --abandon "$WINDOW_ID"'
    ;;

  synth)
    load_env; window_dir "$WINDOW_ID"; need_window_dir
    for f in display-names.json signin-aliases.json; do [[ -f "$PRIV/$f" ]] || (( DRY_RUN )) || die "missing $PRIV/$f (runbook 0.5)"; done
    shown '
(cd "$REPO/infra" && npm run synth -- --quiet -c displayNamesFile="$PRIV/display-names.json" -c signinAliasesFile="$PRIV/signin-aliases.json") > "$W/synth.log" 2>&1
cp "$REPO/infra/cdk.out/GenuiSyntheticFoundation.template.json" "$W/template.json"
shasum -a 256 "$W/template.json" | cut -d" " -f1 > "$W/template.sha256"
release_set ".template_sha256 = \$t" --arg t "$(cat "$W/template.sha256")"'
    ;;

  diff)
    load_env; window_dir "$WINDOW_ID"; need_window_dir
    [[ -f "$W/template-before.json" ]] || (( DRY_RUN )) || die "no template-before.json (just deploy observe)"
    shown '
(cd "$REPO/infra" && npx cdk diff GenuiSyntheticFoundation --no-lookups -c displayNamesFile="$PRIV/display-names.json" -c signinAliasesFile="$PRIV/signin-aliases.json" --template "$W/template-before.json") > "$W/cdk-diff.txt" 2>&1 || true'
    echo "diff in $W/cdk-diff.txt"
    ;;

  change-set)
    load_env; window_dir "$WINDOW_ID"; need_window_dir
    for f in receipt.json template.json; do [[ -f "$W/$f" ]] || (( DRY_RUN )) || die "no $W/$f (preflight, synth)"; done
    # Composing the request is local (window_requests.py calls no AWS API).
    prepare 'python3 "$REPO/infra/scripts/window_requests.py" change-set --receipt "$W/receipt.json" --template "$W/template.json" --window-id "$WINDOW_ID" --output "$W/change-set-request.json" > "$W/change-set.out"'
    mutate change-set '
aws s3api put-object --bucket "$ASSETS" --key "$(jq -r .upload.key "$W/change-set.out")" --body "$W/template.json" --content-type application/json --checksum-sha256 "$(jq -r .upload.checksum_sha256_base64 "$W/change-set.out")" --if-none-match "*" > /dev/null || echo "(template object already present: a 412 is expected on a retry)"
aws cloudformation create-change-set --cli-input-json "file://$W/change-set-request.json" > "$W/change-set-create.json"
aws cloudformation wait change-set-create-complete --stack-name "$STACK" --change-set-name "genui-app078-$WINDOW_ID"'
    ;;

  review)
    load_env; window_dir "$WINDOW_ID"; need_window_dir
    shown '
aws cloudformation describe-change-set --stack-name "$STACK" --change-set-name "genui-app078-$WINDOW_ID" > "$W/change-set-describe.json"
python3 "$REPO/infra/scripts/window_requests.py" review-change-set --describe "$W/change-set-describe.json" --receipt "$W/receipt.json" --output "$W/change-set-review.json"
jq -r ".Changes[].ResourceChange | [.Action, .LogicalResourceId, (.Replacement // \"-\")] | @tsv" "$W/change-set-describe.json"'
    echo "now check every change by hand against $W/cdk-diff.txt (runbook 3.5) before 'just deploy execute'"
    ;;

  execute)
    load_env; window_dir "$WINDOW_ID"; need_window_dir
    [[ -f "$W/change-set-review.json" ]] || (( DRY_RUN )) || die "no change-set-review.json: run 'just deploy review' first"
    mutate execute '
aws cloudformation execute-change-set --stack-name "$STACK" --change-set-name "genui-app078-$WINDOW_ID"
aws cloudformation wait stack-update-complete --stack-name "$STACK"
aws cloudformation describe-stacks --stack-name "$STACK" > "$W/stack-after.json"
aws ecs describe-services --cluster "$CLUSTER" --services "$SERVICE" --query "services[0].{desired:desiredCount,running:runningCount,pending:pendingCount,taskDefinition:taskDefinition}"'
    (( DRY_RUN )) || release_set '.executed_at = (now|todate)'
    (( DRY_RUN )) || echo "the outage has started; a desired count other than 0 is a stop (runbook 'Downtime')"
    ;;

  snapshot)
    load_env; window_dir "$WINDOW_ID"; need_window_dir
    [[ "$snap_name" =~ ^genui-pitch-pre-migration-schema[0-9]+-[0-9]{8}$ ]] || die "--name genui-pitch-pre-migration-schema<live version>-<yyyymmdd> (runbook 5.1)"
    SNAP_NAME=$snap_name; export SNAP_NAME
    mutate snapshot '
aws rds create-db-snapshot --db-instance-identifier "$DB" --db-snapshot-identifier "$SNAP_NAME" > "$W/snapshot-create.json"
aws rds wait db-snapshot-available --db-snapshot-identifier "$SNAP_NAME"'
    (( DRY_RUN )) || release_set '.snapshot = $s' --arg s "$snap_name"
    ;;

  migrate|seed|grants)
    # Operator tasks: ops/beta/run-step.sh asks for and records the approval of
    # each task it runs ([MUTATION]); the composing is local.
    load_env; window_dir "$WINDOW_ID"; need_window_dir
    fwd=(--window "$WINDOW_ID"); [[ -n "$APPROVED" ]] && fwd+=(--approved "$APPROVED"); (( DRY_RUN )) && fwd+=(--dry-run)
    case $step in
      migrate)
        # 5.2: migrate, then the read-only catalog check. Run the step twice;
        # the second migrate output must be identical.
        "$REPO/ops/beta/run-step.sh" "${fwd[@]}" migrate migrate
        "$REPO/ops/beta/run-step.sh" "${fwd[@]}" verify-catalog verify-catalog ;;
      seed)
        (( ${#extra[@]} )) || die "pass the operator flags after --, e.g. -- --set=--organization=<org id> (runbook 5.3)"
        "$REPO/ops/beta/run-step.sh" "${fwd[@]}" seed-library admission-seed-library "${extra[@]}" ;;
      grants)
        (( ${#extra[@]} )) || die "pass the operator flags after --, e.g. -- --set=--organization=<org> --set=--principal=<id> ... (runbook 5.5)"
        "$REPO/ops/beta/run-step.sh" "${fwd[@]}" grant-library-access admission-grant-library-access "${extra[@]}" ;;
    esac
    ;;

  library)
    load_env; window_dir "$WINDOW_ID"; need_window_dir
    [[ -f "$W/stack-after.json" ]] || (( DRY_RUN )) || die "no $W/stack-after.json (just deploy execute)"
    ORG=${organization:-00000000-0000-4000-8000-000000000001}; export ORG
    # [read] the live task definition's environment (bucket, tenants); [local] the files and the plan.
    prepare '
TD=$(jq -r ".Stacks[0].Outputs[] | select(.OutputKey==\"TaskDefinition\") | .OutputValue" "$W/stack-after.json")
aws ecs describe-task-definition --task-definition "$TD" --query "taskDefinition.containerDefinitions[0].environment" > "$W/task-env.json"
"$PY" "$REPO/app/runtime/library_sources.py" build --organization "$ORG" --cache-root "$PRIV/library" > "$W/library-build.json"
"$PY" "$REPO/app/runtime/library_sources.py" plan --organization "$ORG" > "$W/library-plan.json"'
    mutate library '
"$OPVENV/bin/python" -I "$REPO/ops/beta/upload-library.py" --repo "$REPO" --priv "$PRIV" --window-dir "$W" --organization "$ORG" > "$W/library-upload.json"
jq -c "[.objects[].state] | group_by(.) | map({(.[0]): length}) | add" "$W/library-upload.json"'
    ;;

  activate)
    load_env; window_dir "$WINDOW_ID"; need_window_dir
    prepare 'jq ".service_desired_count = 1" "$W/deployment-input.json" > "$W/deployment-input-activate.json"'
    mutate activate '
python3 "$REPO/infra/scripts/preflight.py" --input "$W/deployment-input-activate.json" --ledger "$PRIV/budget-ledger.json" --activate > "$W/activation.json"
jq -c "{status, window_id}" "$W/activation.json"'
    (( DRY_RUN )) || release_set '.activated_at = (now|todate)'
    echo "wait for one healthy target; then 'just deploy verify' (runbook 6.1-6.2)"
    ;;

  verify)
    exec "$REPO/ops/beta/verify.sh" ${extra[@]+"${extra[@]}"}
    ;;

  rollback-deny)
    load_env; window_dir "$WINDOW_ID"; need_window_dir
    [[ -f "$W/stack-after.json" ]] || (( DRY_RUN )) || die "no $W/stack-after.json (the stack outputs of this window)"
    if (( restore )); then OUTPUT_KEY=TaskDefinition; else OUTPUT_KEY=RollbackTaskDefinition; fi
    export OUTPUT_KEY
    name=rollback-deny; (( restore )) && name="rollback-deny --restore"
    mutate "$name" '
TD=$(jq -r --arg k "$OUTPUT_KEY" ".Stacks[0].Outputs[] | select(.OutputKey==\$k) | .OutputValue" "$W/stack-after.json")
aws ecs update-service --cluster "$CLUSTER" --service "$SERVICE" --task-definition "$TD" --query "service.taskDefinition"'
    ;;

  *) die "unknown step '$step' (just deploy help)" ;;
esac
