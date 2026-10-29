"""Verify the vendored registry source and the single reviewed manifest change."""
from pathlib import Path
import difflib
import hashlib
import json

ROOT = Path(__file__).resolve().parent
meta = json.loads((ROOT / 'vendor/provenance.json').read_text())
base = ROOT / 'vendor/zip-9.1.3'
expected = meta['published_files_sha256']
actual_files = {str(p.relative_to(base)) for p in base.rglob('*') if p.is_file()}
assert actual_files == set(expected), f'vendor file set changed: {actual_files ^ set(expected)}'
for name, digest in expected.items():
    data = (base / name).read_bytes()
    if name == 'Cargo.toml':
        assert hashlib.sha256(data).hexdigest() == meta['patched_cargo_toml_sha256']
        patched = data.decode()
        marker = "[target.'cfg(target_arch = \"wasm32\")'.dependencies.getrandom]\nversion = \"0.4\"\n"
        assert patched.count(marker) == 1
        original = patched.replace(marker, marker + 'features = ["wasm_js"]\n', 1)
        data = original.encode()
        diff = ''.join(difflib.unified_diff(original.splitlines(keepends=True), patched.splitlines(keepends=True), fromfile='published/zip-9.1.3/Cargo.toml', tofile='vendor/zip-9.1.3/Cargo.toml'))
        assert diff == (ROOT / 'vendor/zip-9.1.3-manifest.patch').read_text()
    assert hashlib.sha256(data).hexdigest() == digest, f'changed published file: {name}'
print(f'Verified {len(expected)} published ZIP 9.1.3 files; only the declared manifest feature deletion differs')
