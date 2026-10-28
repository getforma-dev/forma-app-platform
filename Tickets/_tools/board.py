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
FIELDS = ['Epic', 'Status', 'Priority', 'Type', 'Sprint', 'Phase', 'Order', 'Size', 'Repos',
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
            field_rows = re.findall(r'^\| ([^|]+?) \| ([^|]*?) \|$', text, re.M)
            fields = dict(field_rows)
            if sum(name == 'Phase' for name, _ in field_rows) != 1:
                raise ValueError(f'{path.name}: exactly one Phase field required')
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
    phases = {p['id']: p for p in config['phases']}
    if len(sprints) != len(config['sprints']):
        raise ValueError('Duplicate sprint IDs')
    if not phases or len(phases) != len(config['phases']):
        raise ValueError('Duplicate phase IDs or empty phase plan')
    if len({p['order'] for p in phases.values()}) != len(phases):
        raise ValueError('Duplicate phase order')
    for p in phases.values():
        if not isinstance(p['order'], int) or p['order'] < 1:
            raise ValueError(f"{p['id']}: phase order must be a positive integer")
        if p['gate_status'] not in ('pending', 'passed'):
            raise ValueError(f"{p['id']}: unknown phase gate status")
        if not isinstance(p['gate_evidence'], list) or any(not isinstance(e, str) or not e.strip() for e in p['gate_evidence']):
            raise ValueError(f"{p['id']}: gate evidence must be a list of nonempty references")
        for d in p['depends_on']:
            if d not in phases or phases[d]['order'] >= p['order']:
                raise ValueError(f"{p['id']}: missing, cyclic or out-of-order phase dependency {d}")
        subset = [t for t in tickets if t['Phase'] == p['id']]
        if not subset or not any(s['phase'] == p['id'] for s in sprints.values()):
            raise ValueError(f"{p['id']}: phase must contain sprints and tickets")
        if p['gate_status'] == 'passed':
            if not p['gate_evidence'] or any(t['Status'] != 'Done' for t in subset):
                raise ValueError(f"{p['id']}: passed gate requires evidence and all phase tickets Done")
            for evidence in p['gate_evidence']:
                path = (ROOT / evidence).resolve()
                if Path(evidence).is_absolute() or ROOT not in path.parents or not path.is_file():
                    raise ValueError(f"{p['id']}: gate evidence must name an existing repository-relative file: {evidence}")
            if any(phases[d]['gate_status'] != 'passed' for d in p['depends_on']):
                raise ValueError(f"{p['id']}: predecessor phase gate has not passed")
    for s in sprints.values():
        if s['phase'] not in phases:
            raise ValueError(f"{s['id']}: unknown phase")
    epics = {e['id'] for e in config['categories']}
    for t in tickets:
        if t['Team'] != 'GenUI':
            raise ValueError(f"{t['id']}: app tickets must identify GenUI ownership")
        if t['Status'] == 'Blocked' and (t['Blocked by'] in ('', 'None', 'none') or t['Unblock evidence'] in ('', 'None', 'none')):
            raise ValueError(f"{t['id']}: blocker and unblock criterion required")
        if t['Sprint'] not in sprints or t['Epic'] not in epics:
            raise ValueError(f"{t['id']}: unknown sprint/category")
        if t['Phase'] not in phases or t['Phase'] != sprints[t['Sprint']]['phase']:
            raise ValueError(f"{t['id']}: phase is unknown or disagrees with sprint")
        for d in t['dependencies']:
            if d not in ids or int(ids[d]['Order']) >= int(t['Order']):
                raise ValueError(f"{t['id']}: missing, cyclic or out-of-order dependency {d}")
            if phases[ids[d]['Phase']]['order'] > phases[t['Phase']]['order']:
                raise ValueError(f"{t['id']}: dependency {d} belongs to a later phase")
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
    generated_names = {'TRACKER.md', 'PHASES.md', 'SPRINTS.md', 'CATEGORIES.md', 'COVERAGE.md', 'BLOCKERS.md'}
    generated_paths = {(BOARD / n).resolve() for n in generated_names}
    generated_paths.update((BOARD / 'sprints' / f"{s['id']}.md").resolve() for s in config['sprints'])
    generated_paths.update((BOARD / 'categories' / f"{e['id']}.md").resolve() for e in config['categories'])
    generated_paths.update((BOARD / 'phases' / f"{p['id']}.md").resolve() for p in config['phases'])
    for top in [ROOT / 'README.md', ROOT / 'AGENTS.md', ROOT / 'CONTRIBUTING.md', ROOT / 'docs', BOARD, ROOT / 'fixtures', ROOT / 'evidence']:
        paths = list(top.rglob('*.md')) if top.is_dir() else [top]
        for p in paths:
            if not p.exists() or any(f'Tickets/evidence/{name}/' in str(p) for name in ('upstream', 'upstream-current')):
                continue
            if p.parent == BOARD and p.name in generated_names or p.parent in (BOARD / 'sprints', BOARD / 'categories', BOARD / 'phases'):
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
    lines = ['| Order | Ticket | Phase | Sprint | Category | Priority | Size | Status | Depends on |',
             '|---|---|---|---|---|---|---|---|---|']
    for t in tickets:
        lines.append(f"| {t['Order']} | [{t['id']}: {t['title']}]({prefix}{t['path']}) | [{t['Phase']}]({prefix}phases/{t['Phase']}.md) | {t['Sprint']} | {t['Epic']} | {t['Priority']} | {t['Size']} | {t['Status']} | {t['Depends on']} |")
    return '\n'.join(lines) + '\n'

def phase_progress(phase, tickets):
    if phase['gate_status'] == 'passed':
        return 'complete'
    if tickets and all(t['Status'] == 'Done' for t in tickets):
        return 'awaiting gate'
    if any(t['Status'] in ('Selected', 'In progress', 'In review', 'Done') for t in tickets):
        return 'active'
    return 'planned'

def outputs(tickets, config, coverage):
    result = {}
    result[BOARD / 'TRACKER.md'] = '# Forma app platform work board\n\nTeam: **GenUI**. Follow the [phase plan](PHASES.md). Upstream blockers belong to **FormaStack**; see the [blocker register](BLOCKERS.md).\n\nGenerated from ticket Markdown. Edit the tickets, then run `python3 Tickets/_tools/board.py render`.\n\n' + table(tickets)
    external = json.loads((BOARD / 'external-dependencies.json').read_text())
    ids = {t['id']: t for t in tickets}
    lines = ['# Cross-team blockers', '', 'GenUI owns the APP consumers. FormaStack owns the upstream FORMA work. Clear the consumer gate only after receiving and checking its evidence; assignment is not completion.', '', '| FormaStack ticket | Gate state | Blocked GenUI ticket | Unblock condition |', '|---|---|---|---|']
    for gate in external['gates']:
        consumers = ', '.join(f"[{d}]({ids[d]['path']})" for d in gate['blocks'])
        lines.append(f"| {gate['id']} | {gate['state']} | {consumers} | {gate['condition']} |")
    lines.extend(['', 'Read the [handoff](../docs/planning/CROSS-TEAM-HANDOFF.md). Source: `external-dependencies.json`; edit the register and ticket evidence, then regenerate.'])
    result[BOARD / 'BLOCKERS.md'] = '\n'.join(lines) + '\n'
    phases = sorted(config['phases'], key=lambda p: p['order'])
    phase_summaries = []
    lines = ['# Implementation phases', '', 'Each phase groups sprints around a concrete outcome. Tickets remain in their status folders; these pages are views, not duplicate boards. Complete a phase only after all its tickets are Done and its exit evidence is accepted in `plan.json`.', '', 'Dependencies control individual ticket selection. Independent work may proceed while an earlier integration gate waits on FormaStack; phase completion still requires predecessor phase gates.', '', '| Phase | Sprints | Tickets | Done | Blocked | Progress | Outcome |', '|---|---|---|---|---|---|---|']
    for p in phases:
        subset = [t for t in tickets if t['Phase'] == p['id']]
        phase_sprints = [s for s in config['sprints'] if s['phase'] == p['id']]
        progress = phase_progress(p, subset)
        done = sum(t['Status'] == 'Done' for t in subset)
        blocked = sum(t['Status'] == 'Blocked' for t in subset)
        sprint_links = ', '.join(f"[{s['id']}](sprints/{s['id']}.md)" for s in phase_sprints)
        lines.append(f"| [{p['id']}: {p['name']}](phases/{p['id']}.md) | {sprint_links} | {len(subset)} | {done} | {blocked} | {progress} | {p['outcome']} |")
        phase_summaries.append({'id': p['id'], 'name': p['name'], 'order': p['order'], 'depends_on': p['depends_on'], 'status': progress,
                                'gate_status': p['gate_status'], 'gate_evidence': p['gate_evidence'], 'sprints': [s['id'] for s in phase_sprints],
                                'tickets': [t['id'] for t in subset], 'ticket_count': len(subset), 'done_count': done, 'blocked_count': blocked})
        predecessors = ', '.join(f'[{d}]({d}.md)' for d in p['depends_on']) or 'None — start here.'
        page = [f"# {p['id']}: {p['name']}", '', f"Team: GenUI. Progress: **{progress}**. Exit gate: **{p['gate_status']}**.", '', f"Predecessor phases: {predecessors}", '',
                '## Outcome', '', p['outcome'], '', '## Entry', '', p['entry'], '', '## Deliverables', '']
        page.extend('- ' + item for item in p['deliverables'])
        page.extend(['', '## Exit gate', '', 'All phase tickets must be Done, predecessor phase gates must pass, and the following evidence must be accepted. Ticket completion alone does not complete the phase. Follow the [quality gates](../../docs/planning/QUALITY-GATES.md) for checks, release trials and evidence standards.', ''])
        marker = 'x' if p['gate_status'] == 'passed' else ' '
        page.extend(f'- [{marker}] {item}' for item in p['exit_criteria'])
        page.extend(['', 'Gate evidence:', ''])
        if p['gate_evidence']:
            page.extend(f'- [{item}](../../{item})' for item in p['gate_evidence'])
        else:
            page.append('- Pending; no phase acceptance claimed.')
        page.extend(['', '## FormaStack dependencies', ''])
        subset_ids = {t['id'] for t in subset}
        matching_gates = [g for g in external['gates'] if any(d in subset_ids for d in g['blocks'])]
        if matching_gates:
            page.extend(['| Upstream owner: FormaStack | Gate state | GenUI consumer | Required evidence |', '|---|---|---|---|'])
            for gate in matching_gates:
                consumers = ', '.join(f"[{d}](../{ids[d]['path']})" for d in gate['blocks'] if ids[d]['Phase'] == p['id'])
                page.append(f"| {gate['id']} | {gate['state']} | {consumers} | {gate['condition']} |")
        else:
            page.append('No new direct FormaStack gate in this phase. Earlier integration evidence and ticket prerequisites still apply.')
        page.extend(['', '## Parallel work', '', p['parallel_work'], '', '## Sprints', ''])
        page.extend(f"- [{s['id']}: {s['name']}](../sprints/{s['id']}.md) — {s['outcome']}" for s in phase_sprints)
        page.extend(['', '## Ordered tickets', '', table(subset, '../'), 'Return to the [phase plan](../PHASES.md).'])
        result[BOARD / 'phases' / f"{p['id']}.md"] = '\n'.join(page) + '\n'
    result[BOARD / 'PHASES.md'] = '\n'.join(lines) + '\n'
    counts = ', '.join(f"{status}: {sum(t['Status']==status for t in tickets)}" for status in STATES.values())
    lines = ['# Sprint plan', '', 'Sprints are ordered scope batches within the [implementation phases](PHASES.md), not calendar promises. Scheduling and capacity are agreed at sprint entry. Later batches are forecasts; split work before selection when it exceeds five focused days.', '', f'Current board: {counts}.', '', '| Sprint | Phase | Outcome | Tickets |', '|---|---|---|---|']
    for s in config['sprints']:
        subset = [t for t in tickets if t['Sprint'] == s['id']]
        state = 'complete' if subset and all(t['Status']=='Done' for t in subset) else 'active' if any(t['Status'] in ('Selected','In progress','In review','Done') for t in subset) else 'planned'
        lines.append(f"| [{s['id']}: {s['name']}](sprints/{s['id']}.md) | [{s['phase']}](phases/{s['phase']}.md) | {s['outcome']} | {len(subset)} |")
        result[BOARD / 'sprints' / f"{s['id']}.md"] = f"# {s['id']}: {s['name']}\n\nPhase: [{s['phase']}](../phases/{s['phase']}.md). Delivery progress: {state}.\n\n## Entry\n\n{s['entry']}\n\n## Outcome\n\n{s['outcome']}\n\n## Exit evidence\n\n{s['exit']}\n\n## Ordered work\n\n" + table(subset, '../') + '\nReturn to the [sprint plan](../SPRINTS.md).\n'
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
    result[BOARD / 'index.json'] = json.dumps({'generated': True, 'source_of_truth': 'status-folder Markdown files', 'phases': phase_summaries, 'ready_by_local_dependencies': [t['id'] for t in ready], 'tickets': tickets}, indent=2) + '\n'
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
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(text)
    elif args.command == 'check':
        stale = [str(p.relative_to(ROOT)) for p, text in generated.items() if not p.exists() or p.read_text()!=text]
        if stale:
            raise ValueError('Stale indexes; run render: ' + ', '.join(stale))
    else:
        data = json.loads(generated[BOARD / 'index.json'])
        for t in tickets:
            if t['id'] in data['ready_by_local_dependencies']:
                print(f"{t['id']} ({t['Phase']}, {t['Sprint']}) {t['title']} — check external dependencies before selecting")
    print(f"OK: {len(tickets)} tickets; {len(config['phases'])} phases; {len(config['sprints'])} sprints; {len(coverage)} requirements; design digests and active links verified.")

if __name__ == '__main__':
    main()
