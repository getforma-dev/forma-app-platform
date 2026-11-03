#!/usr/bin/env bash
# just clean (ADR-012 D12.3, D12.7 rule 3): the per-repository sweep. The
# machine-wide one stays ~/dev/Forma/work/_maintenance/clean-space.sh.
#
#   clean.sh [docker|target|all] [--dry-run]     (default: all)
#
# docker  containers labeled genui.scope that are not running, and volumes
#         labeled genui.scope that no container uses, except dev data volumes
#         (genui.scope=dev: a stopped instance keeps its data). Running
#         containers are never touched. Unlabeled genui-native-* containers
#         (campaign fixtures, stands from before ADR-012 step 4) are listed,
#         not removed (a running one may be another checkout's). Then
#         (ADR-013 step D) the BuildKit cache older than 72 hours
#         (`docker builder prune --filter until=72h`) and every local image
#         set `genui-{service,qualification,operator}:pitch-<sha7>` except
#         the newest three (`just images` makes one per candidate; an image a
#         container uses is left alone by docker).
# target  target/ (the root workspace's, ADR-013 D13.3), a leftover app/target
#         from before it, app/admin/dist and .cache/dev of this checkout.
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
  # A stopped dev instance's data volume (genui.scope=dev) is kept on
  # purpose (`just dev stop` keeps the data); `just dev [clean] reset` drops it.
  vols=$(docker volume ls --filter label=genui.scope --filter dangling=true --format '{{.Name}} {{.Label "genui.scope"}}' | awk '$2 != "dev" {print $1}')
  if [[ -n "$vols" ]]; then do_ docker volume rm $vols; else echo "docker: no unused genui.scope volumes"; fi
  local kept
  kept=$(docker volume ls --filter label=genui.scope=dev --filter dangling=true -q)
  if [[ -n "$kept" ]]; then echo "docker: kept stopped dev instances' data volumes (just dev [clean] reset drops one):"; printf '  %s\n' $kept; fi
  prune_builder
  prune_image_sets
  local unlabeled
  unlabeled=$(docker ps -a --filter name=genui-native- --format '{{.Names}}|{{.Status}}|{{.Label "genui.scope"}}' | awk -F'|' '$3=="" {print "  " $1 "  " $2}')
  if [[ -n "$unlabeled" ]]; then
    echo "docker: unlabeled genui-native-* containers, left alone (a running one may be another checkout's stand):"
    echo "$unlabeled"
  fi
}

# The BuildKit cache: the image builds' layers and cache mounts (cargo
# registry and target); older than 72 hours is reclaimed, the rest keeps the
# next `just images` incremental.
prune_builder() {
  echo "docker: build cache $(docker system df --format '{{.Type}} {{.Size}}' 2>/dev/null | awk '/^Build Cache/ {print $3}')"
  do_ docker builder prune --force --filter until=72h
}

# Local candidate image sets (just images): keep the newest three sets,
# newest by the creation time of any image of the set.
keep_sets=3
prune_image_sets() {
  local sets old tag repo
  sets=$(docker images --format '{{.CreatedAt}}|{{.Repository}}|{{.Tag}}'     | awk -F'|' '$2 ~ /^genui-(service|qualification|operator)$/ && $3 ~ /^pitch-[0-9a-f]+$/ {print $1 "|" $3}'     | sort -r | awk -F'|' '!seen[$2]++ {print $2}')
  if [[ -z "$sets" ]]; then echo "docker: no local pitch-* image sets"; return; fi
  echo "docker: local image sets, newest first: $(echo $sets)"
  old=$(echo "$sets" | tail -n +$((keep_sets + 1)))
  if [[ -z "$old" ]]; then echo "docker: at most $keep_sets image sets; none removed"; return; fi
  for tag in $old; do
    for repo in genui-service genui-qualification genui-operator; do
      if docker image inspect "$repo:$tag" >/dev/null 2>&1; then do_ docker image rm "$repo:$tag" || true; fi
    done
  done
}

clean_target() {
  local p
  for p in target app/target app/admin/dist .cache/dev; do
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
