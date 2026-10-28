#!/bin/sh
set -eu
: "${PGHOST:?Set the dedicated GenUI RDS hostname}"
: "${PGDATABASE:?Set genui}"
: "${PGUSER:?Set genui_migration}"
: "${PGPASSWORD:?Supply the separate migration credential through the authorized runtime channel}"
: "${PGSSLROOTCERT:?Set the trusted RDS CA bundle path}"
base=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
exec "${GENUI_PYTHON:-$base/runtime/.venv/bin/python}" "$base/runtime/runtime.py" migrate
