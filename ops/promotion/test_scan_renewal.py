"""The disposition renewal tool: a synthetic delta round-trips through the real guard; release A's approved file regenerates from its delta."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from ops.promotion import registry, scan_renewal
from ops.promotion.guards import REPOS
from ops.promotion.state import Refused
from ops.promotion.test_scan import RELEASE_A_PIN, RELEASE_A_SHA256, release_a_disposition

ROOT = Path(__file__).resolve().parents[2]
APP175_DELTA = ROOT / 'evidence/APP-175/scan-delta-2026-11-14.json'
CONTRACT = 'c' * 64
HEX = {'service': 'a', 'qualification': 'b', 'operator': 'd'}


def predecessor(role):
    return 'sha256:' + HEX[role] * 64


def finding(name, severity, package, version='1.0-1', score='5.0'):
    return {'name': name, 'severity': severity, 'uri': 'https://nvd.nist.gov/vuln/detail/' + name, 'description': 'synthetic ' + name,
            'attributes': [{'key': 'CVSS3_SCORE', 'value': score}, {'key': 'package_version', 'value': version}, {'key': 'package_name', 'value': package}]}


def scan(role, findings):
    return {'repositoryName': REPOS[role], 'imageId': {'imageDigest': predecessor(role)}, 'imageScanStatus': {'status': 'COMPLETE'},
            'imageScanFindings': {'findings': copy.deepcopy(findings), 'imageScanCompletedAt': '2026-11-24T10:00:00+00:00',
                                  'vulnerabilitySourceUpdatedAt': '2026-11-24T09:00:00+00:00'}}


class ScanRenewalTests(unittest.TestCase):
    def setUp(self):
        self.old = [finding('CVE-2026-000001', 'LOW', 'glibc')]
        self.new = [finding('CVE-2026-000002', 'HIGH', 'krb5'), finding('CVE-2026-000003', 'MEDIUM', 'glibc')]
        self.baselines = {role: {'manifest_digest': predecessor(role), 'findings': copy.deepcopy(self.old), 'scan_completed_at': '2026-11-20 00:00:00+00:00'} for role in REPOS}
        self.currents = {role: scan(role, self.old + self.new) for role in REPOS}

    def test_delta_names_only_the_introduced_fingerprints_and_the_predecessors(self):
        document = scan_renewal.delta(CONTRACT, self.baselines, self.currents)
        self.assertEqual(set(document), {'deployment_contract_sha256', 'method', 'roles'})
        self.assertEqual(set(document['roles']), set(REPOS))
        for role, entry in document['roles'].items():
            self.assertEqual((entry['repository'], entry['manifest_digest'], entry['introduced_count'], entry['removed_count'], entry['critical_count']),
                             (REPOS[role], predecessor(role), 2, 0, 0))
            self.assertEqual([f['name'] for f in entry['introduced']], ['CVE-2026-000002', 'CVE-2026-000003'])
            self.assertEqual((entry['baseline_findings'], entry['rescan_findings']), (1, 3))
            self.assertEqual(set(entry['introduced'][0]), {'name', 'severity', 'attributes', 'uri', 'description'})
        # A removed baseline finding counts; a changed version is a new fingerprint; a wrong repository or incomplete scan refuses.
        self.currents['service']['imageScanFindings']['findings'] = self.new
        self.assertEqual(scan_renewal.delta(CONTRACT, self.baselines, self.currents)['roles']['service']['removed_count'], 1)
        for mutate in [lambda c: c['service'].update(repositoryName='other'), lambda c: c['service']['imageScanStatus'].update(status='IN_PROGRESS'),
                       lambda c: c.pop('operator')]:
            with self.subTest(mutate=mutate):
                currents = copy.deepcopy(self.currents); mutate(currents)
                with self.assertRaises(Refused): scan_renewal.delta(CONTRACT, self.baselines, currents)
        with self.assertRaises(Refused): scan_renewal.delta('not-hex', self.baselines, self.currents)

    def test_generate_produces_a_disposition_the_real_guard_accepts_with_its_printed_pin(self):
        document = scan_renewal.delta(CONTRACT, self.baselines, self.currents)
        disposition, pin = scan_renewal.generate(document, 'evidence/APP-190/SCAN-DISPOSITION.md', '2026-12-16T00:00:00Z')
        self.assertEqual(pin, {'evidence': 'evidence/APP-190/SCAN-DISPOSITION.md', 'roles': frozenset(REPOS), 'expires_at_utc': '2026-12-16T00:00:00Z',
                               'findings': {'CVE-2026-000002': ('HIGH', 'krb5'), 'CVE-2026-000003': ('MEDIUM', 'glibc')}})
        source = scan_renewal.pin_source(pin)
        self.assertIn("'CVE-2026-000002': ('HIGH', 'krb5'),", source)
        self.assertIn("'expires_at_utc': '2026-12-16T00:00:00Z',", source)
        self.assertTrue(source.startswith('REVIEWED_DISPOSITION = {'))
        self.assertEqual(set(disposition), {'schema_version', 'deployment_contract_sha256', 'evidence', 'expires_at_utc', 'roles'})
        for entry in disposition['roles'].values():
            self.assertEqual(set(entry), {'predecessor_manifest_digest', 'findings'})
            self.assertTrue(all(set(f) == {'name', 'severity', 'attributes'} for f in entry['findings']))  # no description or uri in the guard's file
        # The real guard loads it with the printed pin and admits exactly these findings.
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'scan-disposition.json'
            path.write_bytes(scan_renewal.encoded(disposition))
            with patch.object(registry, 'DISPOSITION', path), patch.object(registry, 'REVIEWED_DISPOSITION', pin):
                loaded, sha = registry.scan_disposition()
                self.assertEqual(loaded, disposition)
                baseline = {role: {'manifest_digest': predecessor(role), 'findings': self.old} for role in REPOS}
                current = {role: {'manifest_digest': 'x', 'findings': self.old + self.new} for role in REPOS}
                from datetime import datetime, timezone
                result = registry.guard_scan_delta(current, baseline, CONTRACT, datetime(2026, 12, 15, tzinfo=timezone.utc))
                for role in REPOS:
                    self.assertEqual((result[role]['introduced'], result[role]['reviewed_count'], result[role]['disposition_sha256']), (2, 2, sha))
                with self.assertRaises(Refused):
                    registry.guard_scan_delta(current, baseline, CONTRACT, datetime(2026, 12, 16, tzinfo=timezone.utc))
                # check() binds the generated file to its delta, and a widened file refuses.
                self.assertEqual(scan_renewal.check(document, path)['findings'], 2)
                widened = copy.deepcopy(disposition)
                widened['roles']['service']['findings'].append(dict(widened['roles']['service']['findings'][0], name='CVE-2026-000009'))
                path.write_bytes(scan_renewal.encoded(widened))
                with self.assertRaises(Refused): scan_renewal.check(document, path)

    def test_generate_refuses_critical_differing_roles_and_bad_scope(self):
        document = scan_renewal.delta(CONTRACT, self.baselines, self.currents)
        def critical(d): d['roles']['service']['introduced'][0]['severity'] = 'CRITICAL'
        def critical_count(d): d['roles']['service']['critical_count'] = 1
        def differing_roles(d): d['roles']['service']['introduced'].pop(); d['roles']['service']['introduced_count'] = 1
        def differing_severity(d): d['roles']['service']['introduced'][0]['severity'] = 'LOW'
        def differing_package(d): next(a for a in d['roles']['service']['introduced'][0]['attributes'] if a['key'] == 'package_name')['value'] = 'zlib'
        def no_package(d): d['roles']['service']['introduced'][0]['attributes'] = [{'key': 'CVSS3_SCORE', 'value': '1'}]
        def wrong_count(d): d['roles']['service']['introduced_count'] = 3
        def same_predecessor(d): d['roles']['service']['manifest_digest'] = d['roles']['operator']['manifest_digest']
        def missing_role(d): d['roles'].pop('qualification')
        def nothing(d):
            for entry in d['roles'].values(): entry['introduced'] = []; entry['introduced_count'] = 0
        def bad_contract(d): d['deployment_contract_sha256'] = 'X' * 64
        for name, mutate in [('critical severity', critical), ('critical count', critical_count), ('roles differ', differing_roles),
                             ('severity differs between roles', differing_severity), ('package differs between roles', differing_package),
                             ('no package', no_package), ('count differs', wrong_count), ('same predecessor', same_predecessor),
                             ('missing role', missing_role), ('nothing introduced', nothing), ('bad contract', bad_contract)]:
            with self.subTest(case=name):
                mutated = copy.deepcopy(document); mutate(mutated)
                with self.assertRaises(Refused):
                    scan_renewal.generate(mutated, 'evidence/APP-190/SCAN-DISPOSITION.md', '2026-12-16T00:00:00Z')
        for evidence, expiry in [('docs/x.md', '2026-12-16T00:00:00Z'), ('evidence/APP-190/SCAN-DISPOSITION.md', '2026-12-16'),
                                 ('evidence/APP-190/SCAN-DISPOSITION.md', 'soon')]:
            with self.subTest(evidence=evidence, expiry=expiry), self.assertRaises(Refused):
                scan_renewal.generate(document, evidence, expiry)

    def test_release_a_disposition_is_exactly_what_its_delta_generates(self):
        # The approved file (f00adb9a..., removed once spent) and its registry pin, from the committed delta.
        import hashlib
        document = json.loads(APP175_DELTA.read_text())
        disposition, pin = scan_renewal.generate(document, 'evidence/APP-175/SCAN-DISPOSITION.md', '2026-11-20T00:00:00Z')
        self.assertEqual(hashlib.sha256(scan_renewal.encoded(disposition)).hexdigest(), RELEASE_A_SHA256)
        self.assertEqual(pin, RELEASE_A_PIN)
        release_a_disposition(self)
        result = scan_renewal.check(document)
        self.assertEqual((result['status'], result['findings'], result['expires_at_utc'], result['disposition_sha256']),
                         ('bound', 16, '2026-11-20T00:00:00Z', RELEASE_A_SHA256))
        # A pin that names other findings than the delta refuses.
        with patch.dict(registry.REVIEWED_DISPOSITION['findings'], {'CVE-2026-76844': ('LOW', 'zlib')}):
            with self.assertRaises(Refused): scan_renewal.check(document)

    def test_check_refuses_cleanly_when_no_disposition_is_committed(self):
        import contextlib, io
        with tempfile.TemporaryDirectory() as directory, \
                patch.object(registry, 'DISPOSITION', Path(directory) / 'scan-disposition.json'), patch.object(registry, 'REVIEWED_DISPOSITION', None):
            with self.assertRaisesRegex(Refused, 'no scan disposition is committed'):
                scan_renewal.check(json.loads(APP175_DELTA.read_text()))
            err = io.StringIO()
            with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(err):
                self.assertEqual(scan_renewal.main(['check', '--delta', str(APP175_DELTA)]), 1)
            self.assertEqual(json.loads(err.getvalue()), {'refused': 'no scan disposition is committed'})

    def test_cli_writes_once_and_prints_counts_only(self):
        with tempfile.TemporaryDirectory() as directory:
            d = Path(directory)
            files = []
            for role in REPOS:
                (d / (role + '-baseline.json')).write_text(json.dumps(self.baselines[role]))
                (d / (role + '-current.json')).write_text(json.dumps(self.currents[role]))
                files += ['--baseline', '%s=%s' % (role, d / (role + '-baseline.json')), '--current', '%s=%s' % (role, d / (role + '-current.json'))]
            import contextlib, io
            out = io.StringIO()
            with contextlib.redirect_stdout(out):
                code = scan_renewal.main(['delta', '--contract', CONTRACT, '--output', str(d / 'delta.json'), *files])
            self.assertEqual(code, 0)
            printed = json.loads(out.getvalue())
            self.assertEqual(printed['roles']['service']['introduced'], ['CVE-2026-000002', 'CVE-2026-000003'])
            self.assertNotIn('description', out.getvalue())
            with contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(scan_renewal.main(['delta', '--contract', CONTRACT, '--output', str(d / 'delta.json'), *files]), 1)  # never overwritten
            out = io.StringIO()
            with contextlib.redirect_stdout(out):
                code = scan_renewal.main(['generate', '--delta', str(d / 'delta.json'), '--evidence', 'evidence/APP-190/SCAN-DISPOSITION.md',
                                          '--expires-at-utc', '2026-12-16T00:00:00Z', '--output', str(d / 'disposition.json')])
            self.assertEqual(code, 0)
            self.assertIn("REVIEWED_DISPOSITION = {", json.loads(out.getvalue())['registry_pin'])
            with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(scan_renewal.main(['check', '--delta', str(d / 'delta.json'), '--disposition', str(d / 'disposition.json')]), 0)
                self.assertEqual(scan_renewal.main(['check', '--delta', str(APP175_DELTA), '--disposition', str(d / 'disposition.json')]), 1)


if __name__ == '__main__':
    unittest.main()
