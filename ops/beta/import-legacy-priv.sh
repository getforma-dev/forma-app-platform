#!/usr/bin/env bash
# One-time relocation of the APP-078 window's private directory into this
# checkout (ADR-012 D12.6). COPIES, never moves or deletes: the old directory
# stays until Victor decides what to do with it.
#
#   import-legacy-priv.sh <old private dir> [--dry-run]
#
# Result, all under $PRIV (<checkout>/.cache/ops/beta, gitignored), directories
# 0700 and files 0600:
#   env.sh                    the old env.sh without REPO, CAND, PRIV, OPVENV
#                             (lib.sh derives those from the checkout)
#   approvals.md, budget-ledger.json, display-names.json, signin-aliases.json
#                             copied once, then carried forward by the recipes
#   windows/_pre-adr-012/     the whole old flat directory as history, never
#                             reorganized (its opvenv/ is left out: an
#                             environment, not a record; `just setup` makes
#                             app/operator/.venv instead)
# Prints counts and names only, never file contents or values.
set -euo pipefail
# shellcheck source=ops/beta/lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

old="" dry=0
for a in "$@"; do
  case $a in
    --dry-run) dry=1 ;;
    -h|--help) sed -n '2,19p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) old=$a ;;
  esac
done
[[ -n "$old" && -d "$old" ]] || die "usage: import-legacy-priv.sh <old private dir> [--dry-run]"
old=$(cd "$old" && pwd)
case "$old" in "$REPO"/*) die "the old directory is inside this checkout" ;; esac
history="$PRIV/windows/_pre-adr-012"
[[ -e "$history" ]] && die "already imported: $history exists"
for f in approvals.md budget-ledger.json display-names.json signin-aliases.json env.sh; do
  [[ -f "$old/$f" ]] || die "the old directory has no $f"
  [[ -e "$PRIV/$f" ]] && die "$PRIV/$f already exists; not overwriting"
done

files=$(find "$old" -path "$old/opvenv" -prune -o -type f ! -path "$old/budget-ledger.json.lock" -print | wc -l | tr -d ' ')
echo "import $files files from $old (opvenv/ and the ledger lock excluded) into $PRIV"
if (( dry )); then echo "dry run: nothing copied"; exit 0; fi

umask 077
mkdir -p "$history"
chmod 700 "$(dirname "$REPO/.cache/ops")" "$REPO/.cache/ops" "$PRIV" "$PRIV/windows" 2>/dev/null || true
rsync -a --exclude '/opvenv/' --exclude '/budget-ledger.json.lock' "$old/" "$history/"
for f in approvals.md budget-ledger.json display-names.json signin-aliases.json; do cp "$old/$f" "$PRIV/$f"; done
grep -vE '^[[:space:]]*(export[[:space:]]+)?(REPO|CAND|PRIV|OPVENV)=' "$old/env.sh" > "$PRIV/env.sh" || true
find "$PRIV" -type d -exec chmod 700 {} +
find "$PRIV" -type f -exec chmod 600 {} +

copied=$(find "$history" -type f | wc -l | tr -d ' ')
echo "history: $copied files in $history"
echo "top level: $(cd "$PRIV" && ls -p | grep -v / | tr '\n' ' ')"
echo "env.sh: $(grep -cE '^[[:space:]]*(export[[:space:]]+)?[A-Za-z_][A-Za-z0-9_]*=' "$PRIV/env.sh") variables (path variables dropped)"
[[ "$copied" == "$files" ]] || die "copied $copied files, expected $files"
