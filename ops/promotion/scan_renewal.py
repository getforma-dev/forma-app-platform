"""Regenerate a scan disposition from a sanitized scan delta, and print its registry pin.

The promotion accepts new scanner findings only through `ops/promotion/scan-disposition.json`
plus the matching `REVIEWED_DISPOSITION` pin in `registry.py` (evidence path, roles, expiry and
each CVE's severity and package). The APP-175 record built both programmatically from the
scan objects so that nothing was retyped. This tool is that method, kept in source, for the
release B renewal through 2026-12-16 and any later one:

  delta     the sanitized per-role delta (introduced findings, predecessor digests, counts)
            from private scan objects: the baseline findings per role (the canonical state's
            live.registry[role].findings, extracted privately) and the fresh
            describe-image-scan-findings output per role. Prints counts only.
  generate  the disposition JSON for a delta, an evidence path and an expiry, plus the exact
            Python pin block to paste into registry.py. Refuses a CRITICAL finding and roles
            whose introduced sets differ (the pin is one set for every role).
  check     the committed disposition equals what its delta generates, and the registry pin
            names the same evidence, expiry and findings.

Nothing here talks to AWS or changes the live file; the operator reviews the output, writes
the evidence record, and the reviewed merge carries the file, the pin and the record together.
"""
import argparse
import json
from pathlib import Path
import re
import sys

from .guards import REPOS
from .state import Refused, no_duplicates, require

ROOT = Path(__file__).resolve().parents[2]
DISPOSITION = Path(__file__).with_name('scan-disposition.json')
EXPIRY_RE = re.compile(r'\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z')
HEX_RE = re.compile(r'[0-9a-f]{64}')
DIGEST_RE = re.compile(r'sha256:[0-9a-f]{64}')
SEVERITIES = ('LOW', 'MEDIUM', 'HIGH', 'INFORMATIONAL', 'UNDEFINED')
DELTA_KEYS = frozenset({'deployment_contract_sha256', 'method', 'roles'})
ROLE_KEYS = frozenset({'repository', 'manifest_digest', 'index_digest', 'introduced', 'introduced_count', 'removed_count', 'critical_count',
                       'baseline_findings', 'rescan_findings', 'baseline_counts', 'rescan_counts', 'baseline_scan_completed_at',
                       'rescan_completed_at', 'vulnerability_source_updated_at'})
FINDING_KEYS = frozenset({'name', 'severity', 'attributes', 'uri', 'description'})
METHOD = 'ECR describe-image-scan-findings of each candidate or live image, then the promotion guard fingerprint (name, severity, sorted attributes) against the canonical baseline findings'


def read(path):
    return json.loads(Path(path).read_text(), object_pairs_hook=no_duplicates)


def fingerprint(finding):
    from .registry import finding_set
    return next(iter(finding_set([finding])))


def sanitized(finding):
    require(isinstance(finding, dict) and isinstance(finding.get('name'), str) and isinstance(finding.get('severity'), str)
            and isinstance(finding.get('attributes'), list), 'finding shape differs')
    for attribute in finding['attributes']:
        require(isinstance(attribute, dict) and set(attribute) == {'key', 'value'} and isinstance(attribute['key'], str)
                and isinstance(attribute['value'], str), 'finding attribute shape differs')
    return {'name': finding['name'], 'severity': finding['severity'], 'attributes': [dict(a) for a in finding['attributes']],
            'uri': finding.get('uri'), 'description': finding.get('description')}


def package_of(finding):
    packages = [a['value'] for a in finding['attributes'] if a['key'] == 'package_name']
    require(len(packages) == 1, 'finding names no single package: ' + finding['name'])
    return packages[0]


def delta(contract, baselines, currents):
    """The sanitized delta: per role the introduced fingerprints against the baseline, the predecessor digest and counts."""
    require(isinstance(contract, str) and HEX_RE.fullmatch(contract), 'deployment contract must be a sha256 hex digest')
    require(set(baselines) == set(currents) == set(REPOS), 'one baseline and one current scan per role')
    from .registry import finding_set
    roles = {}
    for role in sorted(REPOS):
        baseline, current = baselines[role], currents[role]
        require(isinstance(baseline, dict) and isinstance(baseline.get('findings'), list), 'baseline lacks findings: ' + role)
        require(isinstance(baseline.get('manifest_digest'), str) and DIGEST_RE.fullmatch(baseline['manifest_digest']), 'baseline lacks its manifest digest: ' + role)
        scan = current.get('imageScanFindings') if isinstance(current, dict) else None
        require(isinstance(scan, dict) and isinstance(scan.get('findings'), list), 'current scan lacks imageScanFindings.findings: ' + role)
        require(current.get('repositoryName') == REPOS[role], 'current scan is for another repository: ' + role)
        require(current.get('imageScanStatus', {}).get('status') == 'COMPLETE', 'current scan is not COMPLETE: ' + role)
        before = finding_set(baseline['findings'])
        now = finding_set(scan['findings'])
        introduced_prints = now - before
        introduced = sorted((sanitized(f) for f in scan['findings'] if fingerprint(f) in introduced_prints), key=lambda f: f['name'])
        require(len(introduced) == len(introduced_prints), 'duplicate fingerprints in the current scan: ' + role)
        counts = lambda findings: {s: sum(1 for f in findings if f.get('severity') == s) for s in sorted({f.get('severity') for f in findings})}
        roles[role] = {'repository': REPOS[role], 'manifest_digest': baseline['manifest_digest'], 'index_digest': baseline.get('index_digest', baseline['manifest_digest']),
                       'introduced': introduced, 'introduced_count': len(introduced), 'removed_count': len(before - now),
                       'critical_count': sum(1 for f in scan['findings'] if f.get('severity') == 'CRITICAL'),
                       'baseline_findings': len(baseline['findings']), 'rescan_findings': len(scan['findings']),
                       'baseline_counts': counts(baseline['findings']), 'rescan_counts': counts(scan['findings']),
                       'baseline_scan_completed_at': baseline.get('scan_completed_at'),
                       'rescan_completed_at': str(scan.get('imageScanCompletedAt')) if scan.get('imageScanCompletedAt') else None,
                       'vulnerability_source_updated_at': str(scan.get('vulnerabilitySourceUpdatedAt')) if scan.get('vulnerabilitySourceUpdatedAt') else None}
    return {'deployment_contract_sha256': contract, 'method': METHOD, 'roles': roles}


def generate(delta_document, evidence, expires_at_utc):
    """The disposition and its pin for a delta; the same reviewed set in every role, never CRITICAL."""
    require(isinstance(delta_document, dict) and DELTA_KEYS <= set(delta_document), 'delta shape differs')
    contract = delta_document['deployment_contract_sha256']
    require(isinstance(contract, str) and HEX_RE.fullmatch(contract), 'delta contract must be a sha256 hex digest')
    require(isinstance(evidence, str) and re.fullmatch(r'evidence/APP-[0-9]{3}/SCAN-DISPOSITION\.md', evidence), 'evidence path must be an evidence/APP-NNN/SCAN-DISPOSITION.md record')
    require(isinstance(expires_at_utc, str) and EXPIRY_RE.fullmatch(expires_at_utc), 'expiry must be a whole-second UTC timestamp')
    roles = delta_document['roles']
    require(isinstance(roles, dict) and set(roles) == set(REPOS), 'delta roles differ from the promoted images')
    findings = {}
    disposition_roles = {}
    for role in sorted(REPOS):
        entry = roles[role]
        require(isinstance(entry, dict) and isinstance(entry.get('introduced'), list) and isinstance(entry.get('manifest_digest'), str)
                and DIGEST_RE.fullmatch(entry['manifest_digest']) and entry.get('repository') == REPOS[role], 'delta role shape differs: ' + role)
        require(entry.get('critical_count') == 0 and not any(f.get('severity') == 'CRITICAL' for f in entry['introduced']),
                'a CRITICAL finding is never accepted: ' + role)
        reviewed = []
        names = set()
        for finding in entry['introduced']:
            clean = sanitized(finding)
            require(clean['severity'] in SEVERITIES, 'unknown severity: ' + clean['name'])
            require(clean['name'] not in names, 'finding repeated in a role: ' + clean['name'])
            names.add(clean['name'])
            pin = (clean['severity'], package_of(clean))
            require(findings.get(clean['name'], pin) == pin, 'the same CVE differs in severity or package between roles: ' + clean['name'])
            findings[clean['name']] = pin
            reviewed.append({'name': clean['name'], 'severity': clean['severity'], 'attributes': clean['attributes']})
        require(entry.get('introduced_count') == len(reviewed), 'introduced_count differs from the introduced list: ' + role)
        disposition_roles[role] = {'predecessor_manifest_digest': entry['manifest_digest'], 'findings': sorted(reviewed, key=lambda f: f['name'])}
    name_sets = {role: {f['name'] for f in entry['findings']} for role, entry in disposition_roles.items()}
    require(len({frozenset(s) for s in name_sets.values()}) == 1, 'roles introduce different finding sets; the pin is one set for every role')
    require(findings, 'no introduced finding: no disposition is needed')
    require(len({e['predecessor_manifest_digest'] for e in disposition_roles.values()}) == len(REPOS), 'predecessor digests must differ per role')
    disposition = {'schema_version': 2, 'deployment_contract_sha256': contract, 'evidence': evidence, 'expires_at_utc': expires_at_utc,
                   'roles': disposition_roles}
    pin = {'evidence': evidence, 'roles': frozenset(REPOS), 'expires_at_utc': expires_at_utc, 'findings': findings}
    return disposition, pin


def pin_source(pin):
    lines = ['REVIEWED_DISPOSITION = {', "    'evidence': %r," % pin['evidence'], "    'roles': frozenset(REPOS),",
             "    'expires_at_utc': %r," % pin['expires_at_utc'], "    'findings': {"]
    for name in sorted(pin['findings']):
        severity, package = pin['findings'][name]
        lines.append("        %r: (%r, %r)," % (name, severity, package))
    lines += ['    },', '}']
    return '\n'.join(lines) + '\n'


def encoded(document):
    return (json.dumps(document, indent=2, sort_keys=True) + '\n').encode()


def check(delta_document, disposition_path=DISPOSITION):
    """The committed disposition is exactly what its delta generates, and the registry pin agrees."""
    from . import registry
    committed, sha = registry.scan_disposition() if disposition_path == DISPOSITION else (read(disposition_path), None)
    require(committed is not None, 'no scan disposition is committed')
    generated, pin = generate(delta_document, committed['evidence'], committed['expires_at_utc'])
    require(generated == committed, 'committed disposition differs from what the delta generates')
    if disposition_path == DISPOSITION:
        reviewed = registry.REVIEWED_DISPOSITION
        require(reviewed['evidence'] == pin['evidence'] and reviewed['expires_at_utc'] == pin['expires_at_utc']
                and reviewed['roles'] == pin['roles'] and dict(reviewed['findings']) == pin['findings'], 'registry pin differs from the delta')
    return {'status': 'bound', 'findings': len(pin['findings']), 'expires_at_utc': pin['expires_at_utc'], 'evidence': pin['evidence'],
            'disposition_sha256': sha}


def pairs(values, label):
    result = {}
    for value in values or []:
        role, sep, path = value.partition('=')
        require(sep == '=' and role in REPOS and role not in result, label + ' takes <role>=<file> once per role')
        result[role] = read(path)
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest='command', required=True)
    p = sub.add_parser('delta')
    p.add_argument('--contract', required=True, help='the deployment contract sha256 the disposition will bind (the release\'s `to` contract)')
    p.add_argument('--baseline', action='append', required=True, help='<role>=<file>: {"manifest_digest", "findings": [...]} from the canonical live registry (private)')
    p.add_argument('--current', action='append', required=True, help='<role>=<file>: describe-image-scan-findings output for the image (private)')
    p.add_argument('--output', required=True, type=Path, help='the sanitized delta (publishable under evidence/)')
    p = sub.add_parser('generate')
    p.add_argument('--delta', required=True, type=Path)
    p.add_argument('--evidence', required=True, help='evidence/APP-NNN/SCAN-DISPOSITION.md')
    p.add_argument('--expires-at-utc', required=True)
    p.add_argument('--output', required=True, type=Path, help='the disposition JSON to review and commit')
    p = sub.add_parser('check')
    p.add_argument('--delta', required=True, type=Path)
    p.add_argument('--disposition', type=Path, default=DISPOSITION)
    args = parser.parse_args(argv)
    try:
        if args.command == 'delta':
            document = delta(args.contract, pairs(args.baseline, '--baseline'), pairs(args.current, '--current'))
            require(not args.output.exists(), 'output exists; never overwrite a delta')
            args.output.write_bytes(encoded(document))
            result = {'status': 'written', 'output': str(args.output), 'roles': {
                role: {k: entry[k] for k in ('introduced_count', 'removed_count', 'critical_count')} | {'introduced': [f['name'] for f in entry['introduced']]}
                for role, entry in document['roles'].items()}}
        elif args.command == 'generate':
            disposition, pin = generate(read(args.delta), args.evidence, args.expires_at_utc)
            require(not args.output.exists(), 'output exists; never overwrite a disposition')
            args.output.write_bytes(encoded(disposition))
            result = {'status': 'generated', 'output': str(args.output), 'findings': sorted(pin['findings']), 'expires_at_utc': pin['expires_at_utc'],
                      'registry_pin': pin_source(pin)}
        else:
            result = check(read(args.delta), args.disposition)
    except (Refused, OSError, ValueError, KeyError) as error:
        print(json.dumps({'refused': str(error)}), file=sys.stderr)
        return 1
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
