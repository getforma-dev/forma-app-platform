"""Generate review candidates from exact migrations in a fresh local PG16.13.

Never reads a live environment or updates the authoritative manifest/baselines.
An explicit new output directory receives readable catalog candidates and a
source/image receipt only after the disposable generator container is removed.
"""
import argparse
import hashlib
import json
from pathlib import Path

import runtime
from runtime import LEDGER_VERSION
from qualify_tenant_schema import LocalPostgres, MIGRATIONS, IMAGE


def readable(value):
    return '{\n'+',\n'.join('  '+json.dumps(key)+': [\n'+',\n'.join(
        '    '+json.dumps(row,separators=(',', ':')) for row in rows)+'\n  ]'
        for key,rows in sorted(value.items()))+'\n}\n'


def generate(destination):
    if destination.exists(): raise ValueError('candidate output directory already exists')
    sources=runtime.migration_sources(MIGRATIONS)
    manifest=runtime.migration_manifest(MIGRATIONS)
    query=json.loads((MIGRATIONS/'catalog-query.json').read_text())
    # Headroom below the native/runtime bound: 80 % of 262,144 is 209,715 bytes.
    limit=query['max_output_bytes']*4//5
    snapshots={}
    pg=LocalPostgres()
    with pg:
        with pg.connect() as c:
            def snapshot(version):
                c.execute('SET LOCAL search_path=pg_catalog, pg_temp')
                raw=c.execute(query['sql']).fetchone()[0]
                if not isinstance(raw,str): raise ValueError(f'catalog-v{version} candidate exceeds the catalog bound')
                snapshots[version]=(raw,json.loads(raw))
            c.execute(sources[0][2])
            snapshot(1)
            # Deliberate candidate construction from reviewed SQL, confined to
            # this newly created local fixture. This is not a migration bypass
            # on an operator-supplied connection or managed endpoint. As in the
            # runner, the upgrade to LEDGER_VERSION creates the ledger at its
            # historical bound and each later migration ALTERs both version
            # CHECKs (ADR-007 D7.12), so every snapshot is that version's state.
            for version,digest,source in sources[1:]:
                if version==LEDGER_VERSION: runtime.initialize_migration_history(c,sources[0][1])
                c.execute(source)
                c.execute('INSERT INTO genui_meta.schema_migrations(version,sha256) VALUES (%s,%s)',(version,digest))
                c.execute('UPDATE genui_meta.schema_version SET version=%s WHERE singleton',(version,))
                snapshot(version)
    if not pg.removed: raise RuntimeError('local catalog generator cleanup failed')
    for version,(raw,value) in snapshots.items():
        if not value or any(len(rows)>=query['section_row_limit'] for rows in value.values()):
            raise ValueError('catalog candidate exceeds bounded projection')
        # Both the native projection text and the readable file stay under it.
        if max(len(raw.encode()),len(readable(value).encode()))>limit:
            raise ValueError(f'catalog-v{version} candidate exceeds {limit} bytes, 80% of the {query["max_output_bytes"]}-byte catalog bound')
    destination.mkdir(parents=True)
    for version,(_,value) in snapshots.items():
        (destination/f'catalog-v{version}.json').write_text(readable(value))
    receipt={'schema_version':1,'scope':'local catalog review candidates; not automatic baseline adoption',
             'image':IMAGE,'server_version':'16.13','container_removed':pg.removed,
             'source_sha256':{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in
               (*(MIGRATIONS/item['file'] for item in manifest['migrations']),MIGRATIONS/'catalog-query.json',Path(__file__),Path(__file__).with_name('qualify_tenant_schema.py'),Path(runtime.__file__))},
             'candidate_sha256':{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in destination.glob('catalog-v*.json')}}
    (destination/'generation.json').write_text(json.dumps(receipt,indent=2)+'\n')
    return receipt


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output-dir',type=Path,required=True)
    print(json.dumps(generate(parser.parse_args().output_dir),indent=2))
