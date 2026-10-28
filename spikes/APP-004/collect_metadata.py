"""Emit a bounded build/feature/license manifest for the qualification evidence."""
from pathlib import Path
import hashlib
import json
import platform
import subprocess
import sys
import tomllib
import zlib

ROOT = Path(__file__).resolve().parent

def command(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True).strip()

def sha(path):
    return 'sha256:' + hashlib.sha256(path.read_bytes()).hexdigest()

metadata = json.loads(command('cargo', 'metadata', '--locked', '--format-version', '1'))
locks = tomllib.loads((ROOT / 'Cargo.lock').read_text())['package']
checksums = {(p['name'], p['version']): p.get('checksum') for p in locks}
features = {p['id']: p['features'] for p in metadata['resolve']['nodes']}
result = {
    'platform': platform.platform(),
    'checkout_head_at_capture': command('git', 'rev-parse', 'HEAD'),
    'source_identity_note': 'APP-004 was uncommitted at local qualification capture; source hashes bind these exact bytes. Final review/CI must identify the committed candidate separately.',
    'machine': platform.machine(),
    'python': sys.version,
    'fixture_zlib': zlib.ZLIB_RUNTIME_VERSION,
    'rustc': command('rustc', '-Vv'),
    'cargo': command('cargo', '-V'),
    'wasm_sha256': sha(ROOT / 'target/wasm32-unknown-unknown/release/parser_guest.wasm'),
    'wasm_bytes': (ROOT / 'target/wasm32-unknown-unknown/release/parser_guest.wasm').stat().st_size,
    'cargo_lock_sha256': sha(ROOT / 'Cargo.lock'),
    'vendor_provenance_sha256': sha(ROOT / 'vendor/provenance.json'),
    'source_files_sha256': {str(p.relative_to(ROOT)): sha(p) for p in sorted(ROOT.rglob('*')) if p.is_file() and not any(x in p.relative_to(ROOT).parts for x in ['target', 'vendor', '__pycache__'])},
    'fixtures_sha256': {p.name: sha(p) for p in sorted((ROOT / 'fixtures').iterdir()) if p.is_file()},
    'packages': sorted([{
        'name': p['name'], 'version': p['version'], 'license': p['license'],
        'source': p['source'] or ('published-source-with-manifest-patch' if p['name'] == 'zip' else 'app-owned'),
        'registry_checksum': checksums.get((p['name'], p['version'])),
        'resolved_features': features[p['id']],
    } for p in metadata['packages']], key=lambda p: (p['name'], p['version'])),
    'feature_scope_note': 'Cargo metadata is the resolved workspace graph; guest-features.txt records the wasm32 guest-only build graph.',
}
print(json.dumps(result, indent=2, sort_keys=True))
