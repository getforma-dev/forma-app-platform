#!/usr/bin/env bash
# The front-end watcher of a dev instance (process-compose `admin`, ADR-012
# D12.1): `npm run dev` in app/admin (the framework's own --watch build).
#
# One watcher per checkout: it holds .cache/dev/admin.lock. A second instance
# (pitch and clean side by side) waits on the lock and shares the running
# watcher's build; it takes over when that one stops. The watcher that takes
# the lock first removes the old manifest, so the `admin` readiness probe
# (manifest present) means "built from this checkout's sources now".
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
mkdir -p "$root/.cache/dev"
[[ -d "$root/app/admin/node_modules" ]] || { echo "app/admin/node_modules is missing: run 'just setup'" >&2; exit 1; }
exec python3 -I -c '
import fcntl, os, sys
lock, admin = sys.argv[1], sys.argv[2]
fd = os.open(lock, os.O_CREAT | os.O_RDWR, 0o600)
try:
    fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
    first = True
except BlockingIOError:
    print("admin: another instance of this checkout runs the watcher; sharing its build", flush=True)
    fcntl.flock(fd, fcntl.LOCK_EX)
    first = False
os.set_inheritable(fd, True)
os.chdir(admin)
if first:
    try:
        os.remove("dist/manifest.json")
    except FileNotFoundError:
        pass
print("admin: npm run dev (watching app/admin/src)", flush=True)
os.execvp("npm", ["npm", "run", "dev"])
' "$root/.cache/dev/admin.lock" "$root/app/admin"
