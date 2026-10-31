#!/usr/bin/env bash
# just clean (ADR-012 D12.3, D12.7 rule 3): the per-repository sweep. The
# machine-wide one stays ~/dev/Forma/work/_maintenance/clean-space.sh.
#
#   clean.sh [docker|target|all] [--dry-run]     (default: all)
#
# docker  containers labeled genui.scope that are not running, and volumes
#         labeled genui.scope that no container uses. Running containers are
#         never touched. The labels arrive with ADR-012 step 4 (compose.dev.yaml,
#         campaign fixtures); until then unlabeled genui-native-* containers are
#         listed, not removed (a running one may be another checkout's stand).
# target  app/target, app/admin/dist and .cache/dev of this checkout.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
what=all dry=0
for a in "$@"; do
  case $a in
    docker|target|all) what=$a ;;
    --dry-run|-n) dry=1 ;;
    *) echo "usage: just clean [docker|target|all] [--dry-run]" >&2; exit 2 ;;
  esac
done
do_() { if (( dry )); then echo "would run: $*"; else echo "+ $*"; "$@"; fi; }

clean_docker() {
  if ! docker info >/dev/null 2>&1; then echo "docker: engine not reachable; skipped"; return; fi
  local ids vols
  ids=$(docker ps -a --filter label=genui.scope --filter status=exited --filter status=created --filter status=dead -q)
  if [[ -n "$ids" ]]; then do_ docker rm --volumes $ids; else echo "docker: no stopped genui.scope containers"; fi
  vols=$(docker volume ls --filter label=genui.scope --filter dangling=true -q)
  if [[ -n "$vols" ]]; then do_ docker volume rm $vols; else echo "docker: no unused genui.scope volumes"; fi
  local unlabeled
  unlabeled=$(docker ps -a --filter name=genui-native- --format '{{.Names}}|{{.Status}}|{{.Label "genui.scope"}}' | awk -F'|' '$3=="" {print "  " $1 "  " $2}')
  if [[ -n "$unlabeled" ]]; then
    echo "docker: unlabeled genui-native-* containers, left alone (a running one may be another checkout's stand):"
    echo "$unlabeled"
  fi
}

clean_target() {
  local p
  for p in app/target app/admin/dist .cache/dev; do
    if [[ -e "$root/$p" ]]; then
      echo "$p: $(du -sh "$root/$p" 2>/dev/null | cut -f1)"
      do_ rm -rf "${root:?}/$p"
    fi
  done
}

case $what in
  docker) clean_docker ;;
  target) clean_target ;;
  all) clean_docker; clean_target ;;
esac
