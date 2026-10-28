#!/usr/bin/env python3
"""Validate the Markdown ticket board and regenerate derived indexes (stdlib only)."""
import argparse
import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BOARD = ROOT / 'Tickets'
STATES = {'0-backlog': 'Backlog', '1-selected': 'Selected', '2-in-progress': 'In progress',
          '3-in-review': 'In review', '4-done': 'Done', '9-blocked': 'Blocked'}
FIELDS = ['Epic', 'Status', 'Priority', 'Type', 'Sprint', 'Stage', 'Order', 'Size', 'Repos',
          'Decision', 'Requirements', 'Depends on', 'External dependencies', 'Assignee', 'Branch / PR',
          'Team', 'Blocked by', 'Unblock evidence']

def read_board():
    tickets = []
    for folder, state in STATES.items():
        for path in sorted((BOARD / folder).glob('APP-*.md')):
            text = path.read_text()
            title = re.search(r'^# (APP-\d{3}): (.+)$', text, re.M)
            if not title:
                raise ValueError(f'{path.name}: malformed title')
            fields = dict(re.findall(r'^\| ([^|]+?) \| ([^|]*?) \|$', text, re.M))
            missing = set(FIELDS) - fields.keys()
            if missing:
                raise ValueError(f'{path.name}: missing fields {missing}')
            if fields['Status'] != state:
                raise ValueError(f'{path.name}: status must match {folder}')
            for section in ['Why', 'What changes', 'Acceptance tests', 'Verification', 'Migration and rollback', 'Log']:
                if f'## {section}\n' not in text:
                    raise ValueError(f'{path.name}: missing {section}')
            if text.count('- [ ] ') + text.count('- [x] ') < 3:
                raise ValueError(f'{path.name}: at least three acceptance checks required')
            tickets.append({'id': title[1], 'title': title[2], 'path': str(path.relative_to(BOARD)),
                            **fields, 'dependencies': re.findall(r'APP-\d{3}', fields['Depends on'])})
    return sorted(tickets, key=lambda t: int(t['Order']))

def verify(tickets, config):
    ids = {t['id']: t for t in tickets}
    if len(ids) != len(tickets) or not tickets:
        raise ValueError('Duplicate IDs or empty board')
    if len({t['Order'] for t in tickets}) != len(tickets):
        raise ValueError('Duplicate execution order')
    sprints = {s['id']: s for s in config['sprints']}
    epics = {e['id'] for e in config['categories']}
    for t in tickets:
        if t['Team'] != 'GenUI':
            raise ValueError(f"{t['id']}: app tickets must identify GenUI ownership")
        if t['Status'] == 'Blocked' and (t['Blocked by'] in ('', 'None', 'none') or t['Unblock evidence'] in ('', 'None', 'none')):
            raise ValueError(f"{t['id']}: blocker and unblock criterion required")
        if t['Sprint'] not in sprints or t['Epic'] not in epics:
            raise ValueError(f"{t['id']}: unknown sprint/category")
        if t['Stage'] != sprints[t['Sprint']]['stage']:
            raise ValueError(f"{t['id']}: stage disagrees with sprint")
        for d in t['dependencies']:
            if d not in ids or int(ids[d]['Order']) >= int(t['Order']):
                raise ValueError(f"{t['id']}: missing, cyclic or out-of-order dependency {d}")
            if t['Status'] in ('Selected', 'In progress', 'In review', 'Done') and ids[d]['Status'] != 'Done':
                raise ValueError(f"{t['id']}: predecessor {d} is not Done")
    coverage = {}
    for t in tickets:
        for req in re.findall(r'\b(?:C\d{2}|R\d+|N\d+|SR\d+|S\d{2}|P\d{2})\b', t['Requirements']):
            coverage.setdefault(req, []).append(t)
    required = ([f'C{i:02}' for i in range(1,17)] + [f'R{i}' for i in range(1,17)] +
                [f'N{i}' for i in range(1,13)] + [f'SR{i}' for i in range(1,14)] + [f'S{i:02}' for i in range(1,8)] + [f'P{i:02}' for i in range(1,17)])
    if set(required) - coverage.keys():
        raise ValueError(f'Missing requirement coverage: {set(required)-coverage.keys()}')
    external = json.loads((BOARD / 'external-dependencies.json').read_text())
    for gate in external['gates']:
        if gate['state'] not in ('open', 'cleared'):
            raise ValueError(f"{gate['id']}: unknown gate state")
        if gate['state'] == 'cleared' and not gate['evidence']:
            raise ValueError(f"{gate['id']}: cleared gate requires evidence")
        for consumer in gate['blocks']:
            if consumer not in ids or gate['id'] not in ids[consumer]['External dependencies']:
                raise ValueError(f"{gate['id']}: consumer reference missing or inconsistent")
            if gate['state'] == 'open' and ids[consumer]['Status'] != 'Blocked':
                raise ValueError(f"{consumer}: uncleared {gate['id']} must remain Blocked")
    manifest = json.loads((ROOT / 'design/snapshot-manifest.json').read_text())
    for f in manifest['files']:
        p = ROOT / f['path']
        if not p.is_file() or hashlib.sha256(p.read_bytes()).hexdigest() != f['sha256']:
            raise ValueError(f"Frozen design changed: {f['path']}")
    lock = json.loads((ROOT / 'design/forma-northstar-demo/platform-contract-lock.json').read_text())
    contract = ROOT / 'design/forma-generative-app-platform-v2/SHARED-CONTRACTS.md'
    if hashlib.sha256(contract.read_bytes()).hexdigest() != lock['sha256']:
        raise ValueError('Paired design contract digest mismatch')
    for name in ('upstream', 'upstream-current'):
        base = BOARD / 'evidence' / name
        snapshot = json.loads((base / 'manifest.json').read_text())
        for item in snapshot['tickets']:
            if hashlib.sha256((base / item['snapshot_path']).read_bytes()).hexdigest() != item['sha256']:
                raise ValueError(f"Historical upstream snapshot changed: {name}/{item['ticket']}")
    # Frozen evidence retains historical links; active docs must be portable and resolve.
    generated_names = {'TRACKER.md', 'SPRINTS.md', 'CATEGORIES.md', 'COVERAGE.md', 'BLOCKERS.md'}
    generated_paths = {(BOARD / n).resolve() for n in generated_names}
    generated_paths.update((BOARD / 'sprints' / f"{s['id']}.md").resolve() for s in config['sprints'])
    generated_paths.update((BOARD / 'categories' / f"{e['id']}.md").resolve() for e in config['categories'])
    for top in [ROOT / 'README.md', ROOT / 'AGENTS.md', ROOT / 'CONTRIBUTING.md', ROOT / 'docs', BOARD, ROOT / 'fixtures', ROOT / 'evidence']:
        paths = list(top.rglob('*.md')) if top.is_dir() else [top]
        for p in paths:
            if not p.exists() or any(f'Tickets/evidence/{name}/' in str(p) for name in ('upstream', 'upstream-current')):
                continue
            if p.parent == BOARD and p.name in generated_names or p.parent in (BOARD / 'sprints', BOARD / 'categories'):
                continue  # Derived links may be stale after a move; render repairs them.
            for target in re.findall(r'\]\(([^)]+)\)', p.read_text()):
                target = target.strip('<>').split('#')[0]
                if not target or re.match(r'[a-zA-Z]+:', target):
                    continue
                if target.startswith('/'):
                    raise ValueError(f'{p.relative_to(ROOT)}: nonportable link {target}')
                if not (p.parent / target).exists() and (p.parent / target).resolve() not in generated_paths:
                    raise ValueError(f'{p.relative_to(ROOT)}: broken link {target}')
    return coverage

def table(tickets, prefix=''):
    lines = ['| Order | Ticket | Sprint | Category | Priority | Size | Status | Depends on |',
             '|---|---|---|---|---|---|---|---|']
    for t in tickets:
        lines.append(f"| {t['Order']} | [{t['id']}: {t['title']}]({prefix}{t['path']}) | {t['Sprint']} | {t['Epic']} | {t['Priority']} | {t['Size']} | {t['Status']} | {t['Depends on']} |")
    return '\n'.join(lines) + '\n'

def outputs(tickets, config, coverage):
    result = {}
    result[BOARD / 'TRACKER.md'] = '# Forma app platform work board\n\nTeam: **GenUI**. Upstream blockers belong to **FormaStack**; see the [blocker register](BLOCKERS.md).\n\nGenerated from ticket Markdown. Edit the tickets, then run `python3 Tickets/_tools/board.py render`.\n\n' + table(tickets)
    external = json.loads((BOARD / 'external-dependencies.json').read_text())
    ids = {t['id']: t for t in tickets}
    lines = ['# Cross-team blockers', '', 'GenUI owns the APP consumers. FormaStack owns the upstream FORMA work. Clear the consumer gate only after receiving and checking its evidence; assignment is not completion.', '', '| FormaStack ticket | Gate state | Blocked GenUI ticket | Unblock condition |', '|---|---|---|---|']
    for gate in external['gates']:
        consumers = ', '.join(f"[{d}]({ids[d]['path']})" for d in gate['blocks'])
        lines.append(f"| {gate['id']} | {gate['state']} | {consumers} | {gate['condition']} |")
    lines.extend(['', 'Read the [handoff](../docs/planning/CROSS-TEAM-HANDOFF.md). Source: `external-dependencies.json`; edit the register and ticket evidence, then regenerate.'])
    result[BOARD / 'BLOCKERS.md'] = '\n'.join(lines) + '\n'
    counts = ', '.join(f"{status}: {sum(t['Status']==status for t in tickets)}" for status in STATES.values())
    lines = ['# Sprint plan', '', 'Sprints are ordered scope batches, not calendar promises. Scheduling and capacity are agreed at sprint entry. Later batches are forecasts; split work before selection when it exceeds five focused days.', '', f'Current board: {counts}.', '', '| Sprint | Stage | Outcome | Tickets |', '|---|---|---|---|']
    for s in config['sprints']:
        subset = [t for t in tickets if t['Sprint'] == s['id']]
        state = 'complete' if subset and all(t['Status']=='Done' for t in subset) else 'active' if any(t['Status'] in ('Selected','In progress','In review','Done') for t in subset) else 'planned'
        lines.append(f"| [{s['id']}: {s['name']}](sprints/{s['id']}.md) | {s['stage']} | {s['outcome']} | {len(subset)} |")
        result[BOARD / 'sprints' / f"{s['id']}.md"] = f"# {s['id']}: {s['name']}\n\nStage: {s['stage']}. Delivery progress: {state}.\n\n## Entry\n\n{s['entry']}\n\n## Outcome\n\n{s['outcome']}\n\n## Exit evidence\n\n{s['exit']}\n\n## Ordered work\n\n" + table(subset, '../') + '\nReturn to the [sprint plan](../SPRINTS.md).\n'
    result[BOARD / 'SPRINTS.md'] = '\n'.join(lines) + '\n'
    lines = ['# Categories', '', 'Categories describe ownership; sprints describe execution order. A ticket lives once, in its status folder.', '', '| Category | Responsibility | Tickets |', '|---|---|---|']
    for e in config['categories']:
        subset = [t for t in tickets if t['Epic'] == e['id']]
        lines.append(f"| [{e['id']}: {e['name']}](categories/{e['id']}.md) | {e['description']} | {len(subset)} |")
        result[BOARD / 'categories' / f"{e['id']}.md"] = f"# {e['id']}: {e['name']}\n\n{e['description']}\n\n" + table(subset, '../')
    result[BOARD / 'CATEGORIES.md'] = '\n'.join(lines) + '\n'
    lines = ['# Requirement coverage', '', 'Planning coverage only: links do not claim implemented or passing behavior. C = shared contract; R/N/SR = platform functional/quality/security requirement; P = product validation; S = Northstar scenario. Read the [source guide](../docs/planning/SOURCE-GUIDE.md) for precedence.', '', '| Requirement | Tickets |', '|---|---|']
    for req, ts in coverage.items():
        lines.append('| ' + req + ' | ' + ', '.join(f"[{t['id']}]({t['path']})" for t in ts) + ' |')
    result[BOARD / 'COVERAGE.md'] = '\n'.join(lines) + '\n'
    ready = [t for t in tickets if t['Status'] == 'Backlog' and all(next(x for x in tickets if x['id']==d)['Status']=='Done' for d in t['dependencies'])]
    result[BOARD / 'index.json'] = json.dumps({'generated': True, 'source_of_truth': 'status-folder Markdown files', 'ready_by_local_dependencies': [t['id'] for t in ready], 'tickets': tickets}, indent=2) + '\n'
    return result

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['check', 'render', 'next'])
    args = parser.parse_args()
    config = json.loads((BOARD / 'plan.json').read_text())
    tickets = read_board()
    coverage = verify(tickets, config)
    generated = outputs(tickets, config, coverage)
    for p, content in generated.items():
        if p.suffix != '.md':
            continue
        for target in re.findall(r'\]\(([^)]+)\)', content):
            target = target.split('#')[0]
            if target and not re.match(r'[a-zA-Z]+:', target) and not (p.parent / target).exists():
                # A cross-index target can be created in this same render.
                if (p.parent / target).resolve() not in {q.resolve() for q in generated}:
                    raise ValueError(f'{p.relative_to(ROOT)}: generated broken link {target}')
    if args.command == 'render':
        for p, text in generated.items():
            p.write_text(text)
    elif args.command == 'check':
        stale = [str(p.relative_to(ROOT)) for p, text in generated.items() if not p.exists() or p.read_text()!=text]
        if stale:
            raise ValueError('Stale indexes; run render: ' + ', '.join(stale))
    else:
        data = json.loads(generated[BOARD / 'index.json'])
        for t in tickets:
            if t['id'] in data['ready_by_local_dependencies']:
                print(f"{t['id']} ({t['Sprint']}) {t['title']} — check external dependencies before selecting")
    print(f"OK: {len(tickets)} tickets; {len(config['sprints'])} sprints; {len(coverage)} requirements; design digests and active links verified.")

if __name__ == '__main__':
    main()
