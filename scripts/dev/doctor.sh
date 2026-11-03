#!/usr/bin/env bash
# just doctor (ADR-012 D12.3): installed tool versions against tools.versions
# and the pin files; the per-checkout venvs; no borrowed paths. Prints what
# differs and exits non-zero if anything required does.
set -uo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$root"
bad=0

row() { printf '  %-22s %-10s %-10s %s\n' "$1" "$2" "$3" "$4"; }
want() { awk -v t="$1" '$1==t {print $2}' tools.versions; }
since() { awk -v t="$1" '$1==t {print $3}' tools.versions; }

version_of() {
  case $1 in
    rustc) (cd app && rustc --version 2>/dev/null) | awk '{print $2}' ;;
    node) node --version 2>/dev/null | sed 's/^v//' ;;
    npm) npm --version 2>/dev/null ;;
    python) app/runtime/.venv/bin/python -c 'import platform;print(platform.python_version())' 2>/dev/null ;;
    uv) uv --version 2>/dev/null | awk '{print $2}' ;;
    just) just --version 2>/dev/null | awk '{print $2}' ;;
    process-compose) process-compose version --short 2>/dev/null | sed 's/^v//' ;;
    docker) docker info --format '{{.ServerVersion}}' 2>/dev/null ;;
  esac
}

echo "tools (tools.versions)"
row tool want have status
for tool in $(awk '!/^#/ && NF {print $1}' tools.versions); do
  w=$(want "$tool"); s=$(since "$tool"); h=$(version_of "$tool")
  if [[ -z "$h" ]]; then
    if [[ -n "$s" ]]; then row "$tool" "$w" "-" "not installed (adopted in ADR-012 $s)"
    else row "$tool" "$w" "-" "MISSING"; bad=1; fi
  elif [[ "$w" == present || "$h" == "$w" ]]; then row "$tool" "$w" "$h" ok
  else row "$tool" "$w" "$h" "DIFFERS"; bad=1; fi
done

echo "pin files"
pin() { if [[ "$2" == "$3" ]]; then row "$1" "$3" "" ok; else row "$1" "$3" "$2" "DIFFERS from tools.versions"; bad=1; fi; }
pin rust-toolchain.toml "$(awk -F'"' '/^channel/ {print $2}' rust-toolchain.toml)" "$(want rustc)"
pin .node-version "$(tr -d '[:space:]' < .node-version)" "$(want node)"
pin .python-version "$(tr -d '[:space:]' < .python-version)" "$(want python)"
active=$(cd app && rustup show active-toolchain 2>/dev/null)
case "$active" in
  *"$root/rust-toolchain.toml"*) row "app toolchain" "" "" "from rust-toolchain.toml" ;;
  *) row "app toolchain" "" "" "NOT from rust-toolchain.toml: $active"; bad=1 ;;
esac

echo "venvs (scripts/dev/setup.sh)"
for v in app/runtime/.venv contracts/.venv app/operator/.venv spikes/APP-005/.venv; do
  if [[ -L "$v" ]]; then row "$v" "" "" "SYMLINK to $(readlink "$v") (borrowed; run just setup)"; bad=1
  elif [[ -x "$v/bin/python" ]]; then
    h=$("$v/bin/python" -c 'import platform;print(platform.python_version())' 2>/dev/null)
    if [[ "$h" == "$(want python)" ]]; then row "$v" "" "$h" ok; else row "$v" "" "$h" "DIFFERS (run just setup)"; bad=1; fi
  else row "$v" "" "" "MISSING (run just setup)"; bad=1; fi
done

if (( bad )); then echo "doctor: differences found"; exit 1; fi
echo "doctor: ok"
