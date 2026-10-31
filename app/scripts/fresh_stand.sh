#!/usr/bin/env bash
# A clean workspace for testing as a new customer: a second, separate dev stand
# (its own database) with one persona, Alex Rivera, on http://127.0.0.1:4010/canvas.
# The 20-app library is there; no records, no folders, nothing on the canvas,
# and no demo shortcuts (sentences go to the model).
#
#   fresh_stand.sh [start]   build if needed, start, print the URL
#   fresh_stand.sh reset     brand new again in seconds: data reseeded, and the
#                            browser's canvas is cleared on the next page load
#   fresh_stand.sh stop | status | restart-server
#
# The pitch stand (app/scripts/dev_stand.sh, :4001-4003) is not touched.
set -euo pipefail
export GENUI_DEV_STAND_PROFILE=fresh
exec "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/dev_stand.sh" "$@"
