#!/usr/bin/env python3
"""Verify public fixture hashes and, when supplied, immutable upstream Git blobs."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parent


def verify(checkout=None):
    manifest = json.loads((ROOT / 'source-contract.json').read_text())
    for name, expected in manifest['fixtures'].items():
        path = ROOT / name
        assert path.is_relative_to(ROOT) and path.is_file(), name
        assert hashlib.sha256(path.read_bytes()).hexdigest() == expected, name
    verified = 0
    if checkout is not None:
        for profile in manifest['profiles']:
            for name, expected in profile['files'].items():
                data = subprocess.check_output(
                    ['git', 'show', f"{profile['commit']}:{name}"], cwd=checkout
                )
                assert len(data) == expected['bytes'], name
                assert hashlib.sha256(data).hexdigest() == expected['sha256'], name
                verified += 1
    return {'fixture_files_verified': len(manifest['fixtures']),
            'immutable_upstream_blobs_verified': verified,
            'upstream_status': 'verified locally' if checkout else 'not available in this run'}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--checkout', type=Path)
    print(json.dumps(verify(parser.parse_args().checkout), indent=2))
