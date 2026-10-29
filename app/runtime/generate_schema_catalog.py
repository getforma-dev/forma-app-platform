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
from qualify_tenant_schema import LocalPostgres, MIGRATIONS, IMAGE


def readable(value):
    return '{\n'+',\n'.join('  '+json.dumps(key)+': [\n'+',\n'.join(
        '    '+json.dumps(row,separators=(',', ':')) for row in rows)+'\n  ]'
        for key,rows in sorted(value.items()))+'\n}\n'


def generate(destination):
    if destination.exists(): raise ValueError('candidate output directory already exists')
    sources=runtime.migration_sources(MIGRATIONS)
    query=json.loads((MIGRATIONS/'catalog-query.json').read_text())
    snapshots={}
    pg=LocalPostgres()
    with pg:
        with pg.connect() as c:
            c.execute(sources[0][2])
            c.execute('SET LOCAL search_path=pg_catalog, pg_temp')
            snapshots[1]=json.loads(c.execute(query['sql']).fetchone()[0])
            # Deliberate candidate construction from reviewed SQL, confined to
            # this newly created local fixture. This is not a migration bypass
            # on an operator-supplied connection or managed endpoint.
            runtime.initialize_migration_history(c,sources[0][1])
            for version,digest,source in sources[1:]:
                c.execute(source)
                c.execute('INSERT INTO genui_meta.schema_migrations(version,sha256) VALUES (%s,%s)',(version,digest))
                c.execute('UPDATE genui_meta.schema_version SET version=%s WHERE singleton',(version,))
            snapshots[2]=json.loads(c.execute(query['sql']).fetchone()[0])
    if not pg.removed: raise RuntimeError('local catalog generator cleanup failed')
    for value in snapshots.values():
        if not value or any(len(rows)>=query['section_row_limit'] for rows in value.values()):
            raise ValueError('catalog candidate exceeds bounded projection')
    destination.mkdir(parents=True)
    for version,value in snapshots.items():
        (destination/f'catalog-v{version}.json').write_text(readable(value))
    receipt={'schema_version':1,'scope':'local catalog review candidates; not automatic baseline adoption',
             'image':IMAGE,'server_version':'16.13','container_removed':pg.removed,
             'source_sha256':{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in
               (MIGRATIONS/'0001_role_foundation.sql',MIGRATIONS/'0002_tenant_foundation.sql',MIGRATIONS/'catalog-query.json',Path(__file__),Path(__file__).with_name('qualify_tenant_schema.py'),Path(runtime.__file__))},
             'candidate_sha256':{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in destination.glob('catalog-v*.json')}}
    (destination/'generation.json').write_text(json.dumps(receipt,indent=2)+'\n')
    return receipt


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output-dir',type=Path,required=True)
    print(json.dumps(generate(parser.parse_args().output_dir),indent=2))
