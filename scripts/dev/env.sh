# Sourced by scripts/dev/*.sh (ADR-012 D12.1): one dev instance's identity,
# derived from the checkout so two checkouts never share a port, a Compose
# project or a state directory.
#
#   source scripts/dev/env.sh <pitch|clean>
#
# Port base: GENUI_DEV_PORT_BASE in this checkout's gitignored .env.dev. When
# it is not set, the main checkout uses 4000 and a worktree picks a free base
# from 4100 upward (derived from its path) and records it in .env.dev, so it
# stays stable. From the base:
#
#   pitch  personas +1 +2 +3 (Diego, Sarah, Priya)   database +20   supervisor API +30
#   clean  persona  +10 (Alex)                       database +21   supervisor API +31
#
# State: <checkout>/.cache/dev/<profile>/ (0700): TLS material, credentials,
# the revision cache, objects, logs. Nothing here reads .env or a secret.

GENUI_DEV_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
GENUI_DEV_PROFILE=${1:?profile}

genui_dev_listening() { lsof -nP -iTCP:"$1" -sTCP:LISTEN >/dev/null 2>&1; }

genui_dev_ports_free() {
  local base=$1 offset
  for offset in 1 2 3 10 20 21 30 31; do
    if genui_dev_listening $((base + offset)); then return 1; fi
  done
}

genui_dev_port_base() {
  local file="$GENUI_DEV_ROOT/.env.dev" base=""
  if [[ -n "${GENUI_DEV_PORT_BASE:-}" ]]; then echo "$GENUI_DEV_PORT_BASE"; return; fi
  if [[ -f "$file" ]]; then
    base=$(sed -n 's/^[[:space:]]*GENUI_DEV_PORT_BASE[[:space:]]*=[[:space:]]*\([0-9][0-9]*\)[[:space:]]*$/\1/p' "$file" | tail -n 1)
  fi
  if [[ -n "$base" ]]; then echo "$base"; return; fi
  local common git_dir
  common=$(git -C "$GENUI_DEV_ROOT" rev-parse --path-format=absolute --git-common-dir 2>/dev/null || true)
  git_dir=$(git -C "$GENUI_DEV_ROOT" rev-parse --path-format=absolute --git-dir 2>/dev/null || true)
  if [[ -n "$common" && "$common" == "$git_dir" ]]; then
    base=4000
  else
    # A worktree: a base derived from its path, moved on while taken.
    local slot tries=0
    slot=$(( $(printf '%s' "$GENUI_DEV_ROOT" | cksum | cut -d' ' -f1) % 50 ))
    while :; do
      base=$(( 4100 + slot * 100 ))
      if genui_dev_ports_free "$base"; then break; fi
      slot=$(( (slot + 1) % 50 )); tries=$((tries + 1))
      if (( tries >= 50 )); then echo "no free dev port base between 4100 and 9000" >&2; return 1; fi
    done
  fi
  printf '# This checkout'"'"'s dev port base (scripts/dev/env.sh); gitignored.\nGENUI_DEV_PORT_BASE=%s\n' "$base" >>"$file"
  echo "$base"
}

GENUI_DEV_PORT_BASE=$(genui_dev_port_base)
genui_dev_slug=$(basename "$GENUI_DEV_ROOT" | tr '[:upper:]' '[:lower:]' | tr -c 'a-z0-9\n' '-' | cut -c1-32)
genui_dev_hash=$(printf '%s' "$GENUI_DEV_ROOT" | shasum -a 256 | cut -c1-6)
GENUI_DEV_PROJECT="genui-dev-${genui_dev_slug}-${genui_dev_hash}-${GENUI_DEV_PROFILE}"
GENUI_DEV_DB_CONTAINER="${GENUI_DEV_PROJECT}-db"
GENUI_DEV_STATE="$GENUI_DEV_ROOT/.cache/dev/$GENUI_DEV_PROFILE"
case $GENUI_DEV_PROFILE in
  pitch)
    GENUI_DEV_PERSONAS=("diego:Diego Ramos:$((GENUI_DEV_PORT_BASE + 1))"
                        "sarah:Sarah Chen:$((GENUI_DEV_PORT_BASE + 2))"
                        "priya:Priya Natarajan:$((GENUI_DEV_PORT_BASE + 3))")
    GENUI_DEV_DB_PORT=$((GENUI_DEV_PORT_BASE + 20))
    GENUI_DEV_PC_PORT=$((GENUI_DEV_PORT_BASE + 30))
    ;;
  clean)
    GENUI_DEV_PERSONAS=("alex:Alex Rivera:$((GENUI_DEV_PORT_BASE + 10))")
    GENUI_DEV_DB_PORT=$((GENUI_DEV_PORT_BASE + 21))
    GENUI_DEV_PC_PORT=$((GENUI_DEV_PORT_BASE + 31))
    ;;
  *) echo "profile must be pitch or clean" >&2; return 2 ;;
esac
GENUI_DEV_PERSONA_PORTS=$(for p in "${GENUI_DEV_PERSONAS[@]}"; do printf '%s,' "${p##*:}"; done)
GENUI_DEV_PERSONA_PORTS=${GENUI_DEV_PERSONA_PORTS%,}
GENUI_DEV_SERVER_PORT=${GENUI_DEV_PERSONA_PORTS%%,*}
export GENUI_DEV_ROOT GENUI_DEV_PROFILE GENUI_DEV_PORT_BASE GENUI_DEV_PROJECT GENUI_DEV_DB_CONTAINER \
  GENUI_DEV_STATE GENUI_DEV_DB_PORT GENUI_DEV_PC_PORT GENUI_DEV_PERSONA_PORTS GENUI_DEV_SERVER_PORT
# process-compose's own API port and log for this instance.
export PC_PORT_NUM=$GENUI_DEV_PC_PORT
export PC_LOG_FILE="$GENUI_DEV_STATE/logs/process-compose.log"
export PC_DISABLE_DOTENV=1
