#!/usr/bin/env bash
# just images <candidate> (DEPLOY-RUNBOOK 0.3): build and qualify the three
# ARM64 images locally. No AWS call; nothing is pushed (that is `just deploy push`).
# From the scratchpad pitch-images/build.sh of 2026-10-30, parameterized.
#
#   build-images.sh <candidate sha> [--clone <checkout>] [--out <dir>] [--allow-dirty] [--dry-run]
#
#   --clone  the checkout to build from (default: this one). It must be at the
#            candidate with a clean tree (runbook 0.1) and have its own venvs
#            (`just setup` there): app/runtime/.venv, app/operator/.venv.
#   --out    logs and check reports (default: <this checkout>/.cache/images/<sha7>)
#
# Images: genui-service, genui-qualification, genui-operator, each tagged
# pitch-<sha7>; checks: app/runtime/qualify_image.py,
# app/qualification/check_image.py, app/operator/check_image.py, then
# ops/beta/image_record.py writes <out>/build.json (ids, sizes, the parser
# manifests compared).
set -euo pipefail

self=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
candidate="" clone=$self out="" dirty=0 dry=0
while (( $# )); do
  case $1 in
    --clone) clone=$(cd "${2:?}" && pwd); shift 2 ;;
    --out) out=${2:?}; shift 2 ;;
    --allow-dirty) dirty=1; shift ;;
    --dry-run) dry=1; shift ;;
    -h|--help) sed -n '2,18p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    -*) echo "unknown option $1" >&2; exit 2 ;;
    *) candidate=$1; shift ;;
  esac
done
[[ "$candidate" =~ ^[0-9a-f]{7,40}$ ]] || { echo "usage: build-images.sh <candidate sha> [--clone <checkout>] (see --help)" >&2; exit 2; }
head=$(git -C "$clone" rev-parse HEAD)
[[ "$head" == "$candidate"* ]] || { echo "refused: $clone is at ${head:0:12}, not $candidate (check out the candidate first)" >&2; exit 2; }
if [[ -n "$(git -C "$clone" status --porcelain --untracked-files=no)" ]] && (( ! dirty )); then
  echo "refused: $clone has uncommitted changes (runbook 0.1; --allow-dirty to override)" >&2; exit 2
fi
C=${head:0:7}
out=${out:-$self/.cache/images/$C}
runtime_py=$clone/app/runtime/.venv/bin/python
operator_py=$clone/app/operator/.venv/bin/python
for p in "$runtime_py" "$operator_py"; do
  [[ -x "$p" ]] || (( dry )) || { echo "missing $p: run 'just setup' in $clone" >&2; exit 2; }
done

step() { printf '+ %s\n' "$*"; (( dry )) || "$@"; }
if (( dry )); then echo "dry run: candidate $C from $clone, reports in $out"; else mkdir -p "$out"; fi
cd "$clone"

for x in service:app/Dockerfile qualification:app/qualification/Dockerfile operator:app/operator/Dockerfile; do
  n=${x%%:*} f=${x#*:}
  printf '+ docker build --platform linux/arm64 -f %s -t genui-%s:pitch-%s . > %s/build-%s.log\n' "$f" "$n" "$C" "$out" "$n"
  (( dry )) && continue
  if ! docker build --platform linux/arm64 -f "$f" -t "genui-$n:pitch-$C" . > "$out/build-$n.log" 2>&1; then
    echo "FAILED to build $n; last lines of $out/build-$n.log:" >&2; tail -20 "$out/build-$n.log" >&2; exit 1
  fi
  echo "built $n"
done

failed=0
check() { # name, command...
  local name=$1; shift
  printf '+ %s\n' "$*"
  (( dry )) && return 0
  if "$@" > /dev/null 2> "$out/$name.err"; then echo "$name ok"; else echo "$name FAILED (see $out/$name.err)" >&2; tail -15 "$out/$name.err" >&2; failed=1; fi
}
check service-qualify "$runtime_py" app/runtime/qualify_image.py --image "genui-service:pitch-$C" --output "$out/service-qualify.json"
check qualification-image "$runtime_py" app/qualification/check_image.py --image "genui-qualification:pitch-$C" --base-revision "$C" --output "$out/qualification-image-check.json"
check operator-image "$operator_py" -P app/operator/check_image.py --image "genui-operator:pitch-$C" --base-revision "$C" --output "$out/operator-image-check.json"
if (( ! dry )) && [[ $failed == 1 && -f "$out/operator-image-check.json" ]]; then
  jq -r '.expectations[]? | select(.met == false) | " unmet: \(.expectation) \(.detail | tostring | .[0:400])"' "$out/operator-image-check.json" || true
fi

# The local image record (ADR-013 D13.5): ids, sizes, times and the parser
# identity, which must be equal in the service and operator images (runbook
# 0.3). `just deploy new` copies it into the window's release.json and
# `just deploy push` refuses images whose ids differ from it.
printf '+ python3 -I ops/beta/image_record.py --candidate %s --out %s\n' "$head" "$out"
(( dry )) && exit 0
python3 -I "$self/ops/beta/image_record.py" --candidate "$head" --out "$out" --checks-passed "$( (( failed )) && echo no || echo yes)" || failed=1
exit $failed
