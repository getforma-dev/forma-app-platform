# The front door (ADR-012 D12.3). Every recipe is a thin call into a script in
# Git (scripts/dev/, ops/beta/), so the editor tasks, the terminal and CI-like
# runs do the same thing. `just` with no arguments lists the recipes.
#
# .env is deliberately NOT loaded: it holds secrets, and only the scripts that
# need one value read it through app/scripts/read_env.py.

set shell := ["bash", "-euo", "pipefail", "-c"]
set positional-arguments

# List the recipes
default:
    @just --list --unsorted

# Toolchains, per-checkout venvs from the hashed locks, npm ci, Docker check (idempotent)
setup *args:
    @scripts/dev/setup.sh "$@"

# Compare installed tools with tools.versions and the pin files
doctor:
    @scripts/dev/doctor.sh

# Dev instance (genui-dev under process-compose): `just dev [clean] [start|stop|reset|status|logs|seed-library] [-d]`; clean = new customer on base+10
dev *args:
    @scripts/dev/dev.sh "$@"

# Rebuild and restart the dev server only, data kept (`just restart clean` for the clean instance)
restart *args:
    @scripts/dev/dev.sh "$@" restart-server

# What is running, and the URLs
status *args:
    @scripts/dev/dev.sh "$@" status

# The CI-fast set in one command; `just check <step>...` runs only those steps (`just check --list`)
check *steps:
    @scripts/dev/check.sh "$@"

# `check` plus the schema qualifier and the 10 CI campaigns: a local green branch
ci:
    @scripts/dev/check.sh --ci

# The ignored-test catalog: short name, test path, report variable, CI status
campaigns *args:
    @scripts/dev/campaigns.sh list "$@"

# Run one ignored campaign by short name; its report goes to .cache/reports/<name>.json
campaign name *args:
    @scripts/dev/campaigns.sh run "$@"

# Build the sealed parser from parser/ into .cache/parser-target/
parser *args:
    @scripts/dev/parser.sh "$@"

# Build and qualify the three ARM64 images for <candidate> (local, no AWS)
images candidate *args:
    @ops/beta/build-images.sh "$@"

# Beta deploy, one runbook step: `just deploy <step> <window> [--approved "<Victor's words>"] [--dry-run]`; `just deploy help`
deploy step *args:
    @ops/beta/deploy.sh "$@"

# Remove local build output and stray containers: `just clean [docker|target|all] [--dry-run]`
clean *args:
    @scripts/dev/clean.sh "$@"
