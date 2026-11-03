#!/usr/bin/env bash
# Syntax-check every shell script kept in Git (ADR-012 D12.3: the scripts stay
# testable, as getforma-infra's Makefile checks its own).
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$root"
status=0 count=0
while IFS= read -r f; do
  count=$((count + 1))
  case "$(head -1 "$f")" in
    *zsh*) zsh -n "$f" || status=1 ;;
    *) bash -n "$f" || status=1 ;;
  esac
done < <(git ls-files -co --exclude-standard -- '*.sh' | grep -v -e '^spikes/' -e '^evidence/' -e '/node_modules/')
echo "bash -n: $count scripts checked"
exit $status
