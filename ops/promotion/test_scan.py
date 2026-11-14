import copy
from datetime import datetime, timedelta, timezone
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from ops.promotion import registry
from ops.promotion.guards import REPOS
from ops.promotion.registry import REVIEWED_DISPOSITION, guard_scan_delta, scan_disposition
from ops.promotion.state import Refused


def expiry(value):
    return datetime.fromisoformat(value['expires_at_utc'].replace('Z', '+00:00'))


class ScanDispositionTests(unittest.TestCase):
    def setUp(self):
        self.disposition, self.sha = scan_disposition()
        self.baseline = {role: {'manifest_digest': self.disposition['roles'][role]['predecessor_manifest_digest'],
                                'findings': []} for role in REPOS}
        self.current = copy.deepcopy(self.baseline)
        for role in REPOS:
            self.current[role]['findings'] = copy.deepcopy(self.disposition['roles'][role]['findings'])
        self.contract = self.disposition['deployment_contract_sha256']
        self.reviewed = sorted(REVIEWED_DISPOSITION['findings'])
        self.clock = expiry(self.disposition) - timedelta(days=1)

    def check(self):
        return guard_scan_delta(self.current, self.baseline, self.contract, self.clock)

    def test_exact_review_retains_findings_and_reports_all_introduced(self):
        before = copy.deepcopy(self.current)
        result = self.check()
        for role in REPOS:
            delta = result[role]
            count = len(self.reviewed)
            self.assertEqual((delta['introduced'], delta['reviewed_count'], delta['unreviewed']), (count, count, 0))
            self.assertEqual(delta['reviewed'], self.reviewed)
            self.assertEqual(delta['disposition_sha256'], self.sha)
            self.assertEqual(delta['review_expires_at_utc'], self.disposition['expires_at_utc'])
        self.assertEqual(self.current, before)

    def test_one_role_alone_may_introduce_the_reviewed_findings(self):
        for role in REPOS:
            with self.subTest(role=role):
                self.setUp()
                for other in REPOS:
                    if other != role: self.current[other]['findings'] = []
                result = self.check()
                self.assertEqual(result[role]['reviewed_count'], len(self.reviewed))
                self.assertTrue(all(result[o]['introduced'] == 0 for o in REPOS if o != role))

    def test_changed_package_version_severity_attributes_and_extra_finding_refuse(self):
        for mutate in [lambda f: f.update(severity='MEDIUM' if f['severity'] != 'MEDIUM' else 'HIGH'), lambda f: f.update(severity='CRITICAL'),
                lambda f: f.update(name='CVE-2099-12345'),
                lambda f: f['attributes'].append({'key': 'extra', 'value': 'new'}),
                lambda f: next(a for a in f['attributes'] if a['key'].endswith('_SCORE')).update(value='9.9'),
                lambda f: next(a for a in f['attributes'] if a['key'].endswith('_VECTOR')).update(value='CVSS:3.1/AV:L'),
                lambda f: next(a for a in f['attributes'] if a['key'] == 'package_name').update(value='another'),
                lambda f: next(a for a in f['attributes'] if a['key'] == 'package_version').update(value='new')]:
            for role in REPOS:
                with self.subTest(mutate=mutate, role=role):
                    self.setUp(); mutate(self.current[role]['findings'][0])
                    with self.assertRaises(Refused): self.check()

    def test_a_critical_finding_refuses_even_when_the_predecessor_had_it(self):
        # Review of cf9600e: only this case isolates the CRITICAL refusal (an
        # introduced CRITICAL also fails the reviewed fingerprint check).
        for role in REPOS:
            with self.subTest(role=role):
                self.setUp()
                critical = copy.deepcopy(self.current[role]['findings'][0])
                critical.update(name='CVE-2099-00001', severity='CRITICAL')
                self.baseline[role]['findings'].append(copy.deepcopy(critical))
                self.current[role]['findings'].append(critical)
                with self.assertRaisesRegex(Refused, 'critical scan finding'): self.check()

    def test_an_unreviewed_extra_finding_refuses(self):
        self.current['service']['findings'].append(
            {'name': 'CVE-2099-12345', 'severity': 'LOW', 'attributes': [{'key': 'package_name', 'value': 'x'}]})
        with self.assertRaises(Refused): self.check()

    def test_scope_and_expiry_refuse(self):
        for mutation in ['predecessor', 'contract', 'expiry']:
            for role in REPOS:
                with self.subTest(mutation=mutation, role=role):
                    self.setUp()
                    if mutation == 'predecessor': self.baseline[role]['manifest_digest'] = 'sha256:' + 'b' * 64
                    if mutation == 'contract': self.contract = 'b' * 64
                    if mutation == 'expiry': self.clock = expiry(self.disposition)
                    with self.assertRaises(Refused): self.check()

    def test_later_release_with_no_new_findings_needs_no_expired_disposition(self):
        self.baseline = copy.deepcopy(self.current)
        self.clock = expiry(self.disposition) + timedelta(days=10)
        for delta in self.check().values():
            self.assertEqual(delta['introduced'], 0)
            self.assertIsNone(delta['disposition_sha256'])


class DispositionFileTests(unittest.TestCase):
    """Editing scan-disposition.json alone must never widen what is accepted."""

    def load_text(self, text):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'scan-disposition.json'
            path.write_text(text)
            with patch.object(registry, 'DISPOSITION', path):
                return scan_disposition()

    def load(self, mutate):
        value = json.loads(registry.DISPOSITION.read_bytes())
        replaced = mutate(value)  # only a mutation that rebuilds the document returns it
        return self.load_text(json.dumps(replaced if isinstance(replaced, list) else value))

    def test_duplicate_keys_and_invalid_json_refuse(self):
        text = registry.DISPOSITION.read_text()
        pinned = '"expires_at_utc": "' + REVIEWED_DISPOSITION['expires_at_utc'] + '"'
        self.assertIn(pinned, text)
        for changed in [text.replace(pinned, pinned + ', "expires_at_utc": "2099-01-01T00:00:00Z"'),
                        text.replace('"roles": {', '"roles": {"operator": {}, ', 1),
                        text[:-3], '', 'null', '[]']:
            with self.subTest(changed=changed[:40]):
                self.assertNotEqual(changed, text)
                with self.assertRaises(Refused): self.load_text(changed)

    def test_the_committed_file_is_valid(self):
        value, sha = scan_disposition()
        self.assertEqual(set(value['roles']), set(REPOS))
        self.assertEqual(len(sha), 64)

    def test_widening_or_reshaping_the_file_refuses(self):
        first = lambda v: v['roles']['service']['findings'][0]
        attribute = lambda v, key: next(a for a in first(v)['attributes'] if a['key'] == key)
        mutations = [
            lambda v: v.update(expires_at_utc='2099-01-01T00:00:00Z'),
            lambda v: v.update(expires_at_utc=(expiry(v) + timedelta(days=1)).strftime('%Y-%m-%dT%H:%M:%SZ')),
            lambda v: attribute(v, 'package_name').update(value='openssl'),
            lambda v: first(v)['attributes'].remove(attribute(v, 'package_name')),
            lambda v: first(v)['attributes'].append(copy.deepcopy(attribute(v, 'package_name'))),
            lambda v: attribute(v, 'package_version').update(value=['1.2.13']),
            lambda v: attribute(v, 'package_version').update(extra='x'),
            lambda v: first(v).update(attributes='package_name=zlib'),
            lambda v: first(v).update(attributes={'package_name': 'zlib'}),
            lambda v: first(v).update(name=['CVE-2026-76844']),
            lambda v: v.update(schema_version=2.0),
            lambda v: v.update(schema_version=True),
            lambda v: [[k, x] for k, x in v.items()],
            lambda v: first(v).update(severity='CRITICAL'),
            lambda v: first(v).update(severity='MEDIUM'),
            lambda v: first(v).update(name='CVE-2099-12345'),
            lambda v: first(v).update(uri='https://example.invalid'),
            lambda v: v['roles']['service']['findings'].append(copy.deepcopy(first(v))),
            lambda v: v['roles']['service']['findings'].pop(),
            lambda v: v['roles'].pop('operator'),
            lambda v: v['roles'].update(extra=copy.deepcopy(v['roles']['service'])),
            lambda v: v['roles']['service'].update(predecessor_manifest_digest='latest'),
            lambda v: v['roles']['service'].update(note='x'),
            lambda v: v.update(schema_version=1),
            lambda v: v.update(evidence='evidence/APP-167/SCAN-DISPOSITION.md'),
            lambda v: v.update(evidence='evidence/APP-173/SCAN-DISPOSITION.md'),
            lambda v: v['roles']['qualification'].update(findings=v['roles']['qualification']['findings'][:-1]),
            lambda v: v.update(deployment_contract_sha256='X' * 64),
            lambda v: v.update(expires_at_utc='2026-13-40T00:00:00Z'),
            lambda v: v.update(expires_at_utc='soon'),
            lambda v: v.update(role='operator'),
        ]
        for mutate in mutations:
            with self.subTest(mutate=mutate):
                with self.assertRaises(Refused): self.load(mutate)


class ReleaseBindingTests(unittest.TestCase):
    """The committed disposition belongs to the APP-175 schema release and its evidence.

    Not bindable here: that each predecessor is the live image (the canonical state is
    private) and that the candidate images report the same findings. The guard checks
    both at preparation and again before the outage.
    """

    def setUp(self):
        self.value, _ = scan_disposition()
        self.delta = json.loads((Path(__file__).resolve().parents[2] / 'evidence/APP-175/scan-delta-2026-11-14.json').read_text())

    def test_contract_and_expiry_are_the_schema_releases(self):
        from ops.promotion import schema_release
        self.assertEqual(self.value['deployment_contract_sha256'], schema_release.REVIEWED_SCHEMA_RELEASE['to']['deployment_contract_sha256'])
        self.assertEqual(self.value['expires_at_utc'], schema_release.REVIEWED_SCHEMA_RELEASE['expires_at_utc'])
        if schema_release.SPEC.exists():
            spec, _ = schema_release.load()
            self.assertEqual(self.value['deployment_contract_sha256'], spec['to']['deployment_contract_sha256'])
        self.assertEqual(self.value['evidence'], 'evidence/APP-175/SCAN-DISPOSITION.md')

    def test_every_role_reviews_exactly_the_committed_scan_delta(self):
        from ops.promotion.registry import finding_set
        self.assertEqual(self.delta['deployment_contract_sha256'], self.value['deployment_contract_sha256'])
        self.assertEqual(set(self.delta['roles']), set(REPOS))
        predecessors = set()
        for role, entry in self.value['roles'].items():
            with self.subTest(role=role):
                observed = self.delta['roles'][role]
                self.assertEqual(observed['repository'], REPOS[role])
                self.assertEqual(entry['predecessor_manifest_digest'], observed['manifest_digest'])
                self.assertEqual(finding_set(entry['findings']), finding_set(observed['introduced']))
                self.assertEqual(observed['introduced_count'], len(entry['findings']))
                # The one removed fingerprint is CVE-2026-76844's November 5 vector; the scanner re-vectored
                # it on 2026-11-14 (same package, version and score), so it is also among the introduced.
                self.assertEqual((observed['removed_count'], observed['critical_count']), (1, 0))
                self.assertIn('CVE-2026-76844', {f['name'] for f in observed['introduced']})
                self.assertEqual(sorted(f['name'] for f in entry['findings']), sorted(REVIEWED_DISPOSITION['findings']))
                predecessors.add(entry['predecessor_manifest_digest'])
        self.assertEqual(len(predecessors), len(REPOS))


if __name__ == '__main__': unittest.main()
