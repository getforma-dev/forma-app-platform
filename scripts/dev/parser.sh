#!/usr/bin/env bash
# just parser (ADR-012 D12.6): build the sealed parser from parser/ into
# <checkout>/.cache/parser-target/ (gitignored), the default GENUI_BOX_PARSER_DIR
# of the dev binary and the box campaign (app/src/dev/mod.rs, app/src/storage/box_campaign.rs).
#
# A local development build: the release guest is built only by app/Dockerfile's
# parser stage (parser/.cargo/config.toml), and these bytes are never shipped or
# qualified. Same commands, toolchain (parser/rust-toolchain.toml) and lockfile.
#
#   parser.sh [--print-dir]
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
target="$root/.cache/parser-target"
if [[ "${1:-}" == --print-dir ]]; then echo "$target"; exit 0; fi

mkdir -p "$root/.cache"
chmod 700 "$root/.cache"
cd "$root/parser"
python3 verify_vendor.py
export CARGO_TARGET_DIR="$target"
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-3}
cargo build --locked --release -p parser-guest --target wasm32-unknown-unknown
cargo build --locked --release -p parser-host
for f in release/parser-host wasm32-unknown-unknown/release/parser_guest.wasm; do
  printf '%s  %s\n' "$(shasum -a 256 "$target/$f" | cut -d' ' -f1)" "$f"
done
echo "sealed parser built in $target"
