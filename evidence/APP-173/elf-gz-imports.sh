#!/bin/bash
# usage: scan.sh <image> <outdir>
set -euo pipefail
img=$1; out=$2; root=$out/rootfs; mkdir -p "$root"
cid=$(docker create --platform linux/arm64 "$img" true)
docker export "$cid" | tar -x -C "$root" 2>/dev/null || true
docker rm -f "$cid" >/dev/null
docker image inspect "$img" --format '{{.Id}} {{.Architecture}}' > "$out/image-id.txt"
: > "$out/elf-gz-imports.txt"; : > "$out/elf-libz-needed.txt"; n=0
while IFS= read -r -d '' f; do
  head -c4 "$f" 2>/dev/null | grep -q $'\x7fELF' || continue
  n=$((n+1)); rel=${f#$root}
  syms=$(nm -D -u "$f" 2>/dev/null | awk '{print $NF}' | grep -E '^gz[a-z_]+' | sed 's/@.*//' | sort -u | tr '\n' ' ' || true)
  [ -n "$syms" ] && echo "$rel: $syms" >> "$out/elf-gz-imports.txt"
  defs=$(nm -D --defined-only "$f" 2>/dev/null | awk '{print $NF}' | grep -E '^gz(write|printf|puts|fwrite)$' | tr '\n' ' ' || true)
  [ -n "$defs" ] && echo "$rel DEFINES: $defs" >> "$out/elf-gz-imports.txt"
done < <(find "$root" -type f -size +1k -print0)
echo "elf_files_scanned=$n" > "$out/summary.txt"
rm -rf "$root"
