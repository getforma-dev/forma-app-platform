"""Write the image's parser-manifest.json (ADR-009 D9.1).

app/Dockerfile's parser stage runs this after building the guest and the
native `parser-host` from this directory with its own lockfile. The manifest
names the exact bytes the image ships and binds them to the sources, lockfile,
vendored ZIP provenance and rustc that built them. The app checks the bytes
against it at startup (integrity); a release separately requires a passing
image-bytes qualification record for the guest digest.

`sources_sha256` is also computed by fixtures/qualify.py over the checkout, so
the qualification of image bytes can prove they came from these sources.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import tomllib

SCHEMA = "genui-parser-manifest/1"
IGNORED_NAMES = {"__pycache__", ".DS_Store"}


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def source_files(root: Path) -> list[str]:
    """Every file under `root` except build output (top-level target/)."""
    files = []
    for path in root.rglob("*"):
        relative = path.relative_to(root)
        if relative.parts[0] == "target" or IGNORED_NAMES.intersection(relative.parts):
            continue
        if path.is_symlink():
            raise SystemExit(f"symlink in parser sources: {relative}")
        if path.is_file():
            files.append(relative.as_posix())
    return sorted(files)


def sources_sha256(root: Path) -> tuple[str, int]:
    """SHA-256 over `path NUL sha256(file) LF` for each source file in byte order."""
    digest = hashlib.sha256()
    files = source_files(root)
    for relative in files:
        digest.update(relative.encode() + b"\0" + sha256(root / relative).encode() + b"\n")
    return digest.hexdigest(), len(files)


def rustc_identity(text: str) -> dict[str, str]:
    fields = dict(line.split(": ", 1) for line in text.splitlines() if ": " in line)
    return {"release": fields["release"], "commit-hash": fields["commit-hash"], "LLVM version": fields["LLVM version"], "host": fields["host"]}


def manifest(root: Path, guest: Path, guest_path: str, host: Path, host_path: str, rustc_vv: str) -> dict:
    tree, count = sources_sha256(root)
    channel = tomllib.loads((root / "rust-toolchain.toml").read_text())["toolchain"]["channel"]
    rustc = rustc_identity(rustc_vv)
    if rustc["release"] != channel:
        raise SystemExit(f"rustc {rustc['release']} is not the pinned toolchain {channel}")
    return {
        "schema": SCHEMA,
        "guest": {"path": guest_path, "sha256": sha256(guest), "bytes": guest.stat().st_size},
        "host": {"path": host_path, "sha256": sha256(host), "bytes": host.stat().st_size},
        "source": {
            "sources_sha256": tree,
            "source_files": count,
            "cargo_lock_sha256": sha256(root / "Cargo.lock"),
            "vendor_provenance_sha256": sha256(root / "vendor/provenance.json"),
            "rust_toolchain": channel,
            "rustc": rustc,
        },
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--guest", type=Path, required=True)
    parser.add_argument("--guest-path", required=True)
    parser.add_argument("--host", type=Path, required=True)
    parser.add_argument("--host-path", required=True)
    parser.add_argument("--rustc-vv", type=Path, required=True, help="file holding `rustc -vV` output")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    document = manifest(args.source, args.guest, args.guest_path, args.host, args.host_path, args.rustc_vv.read_text())
    args.output.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n")
    print(json.dumps({"guest_sha256": document["guest"]["sha256"], "sources_sha256": document["source"]["sources_sha256"]}))


if __name__ == "__main__":
    main()
