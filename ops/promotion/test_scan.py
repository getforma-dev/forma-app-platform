import copy
from datetime import datetime, timezone
import unittest

from ops.promotion.guards import REPOS
from ops.promotion.registry import guard_scan_delta, scan_disposition
from ops.promotion.state import Refused


class ScanDispositionTests(unittest.TestCase):
    def setUp(self):
        self.disposition, self.sha = scan_disposition()
        self.baseline = {role: {'manifest_digest': 'sha256:' + 'a' * 64, 'findings': []} for role in REPOS}
        self.baseline['operator']['manifest_digest'] = self.disposition['predecessor_manifest_digest']
        self.current = copy.deepcopy(self.baseline)
        self.current['operator']['findings'] = copy.deepcopy(self.disposition['findings'])
        self.contract = self.disposition['deployment_contract_sha256']
        self.clock = datetime(2026, 11, 2, tzinfo=timezone.utc)

    def check(self):
        return guard_scan_delta(self.current, self.baseline, self.contract, self.clock)

    def test_exact_review_retains_findings_and_reports_all_introduced(self):
        before = copy.deepcopy(self.current)
        delta = self.check()['operator']
        self.assertEqual((delta['introduced'], delta['reviewed_count'], delta['unreviewed']), (3, 3, 0))
        self.assertEqual(delta['disposition_sha256'], self.sha)
        self.assertEqual(delta['review_expires_at_utc'], self.disposition['expires_at_utc'])
        self.assertEqual(self.current, before)

    def test_changed_package_version_severity_attributes_and_extra_finding_refuse(self):
        for mutate in [lambda f: f.update(severity='HIGH'), lambda f: f.update(severity='CRITICAL'),
                lambda f: f.update(name='CVE-2099-12345'),
                lambda f: f['attributes'].append({'key': 'extra', 'value': 'new'}),
                lambda f: next(a for a in f['attributes'] if a['key'] == 'package_name').update(value='another'),
                lambda f: next(a for a in f['attributes'] if a['key'] == 'package_version').update(value='new')]:
            with self.subTest(mutate=mutate):
                self.setUp(); mutate(self.current['operator']['findings'][0])
                with self.assertRaises(Refused): self.check()

    def test_scope_and_expiry_refuse(self):
        for mutation in ['role', 'predecessor', 'contract', 'expiry']:
            with self.subTest(mutation=mutation):
                self.setUp()
                if mutation == 'role': self.current['service']['findings'] = self.current['operator']['findings']
                if mutation == 'predecessor': self.baseline['operator']['manifest_digest'] = 'sha256:' + 'b' * 64
                if mutation == 'contract': self.contract = 'b' * 64
                if mutation == 'expiry': self.clock = datetime(2026, 11, 9, tzinfo=timezone.utc)
                with self.assertRaises(Refused): self.check()

    def test_later_release_with_no_new_findings_needs_no_expired_disposition(self):
        self.baseline = copy.deepcopy(self.current)
        self.clock = datetime(2026, 11, 10, tzinfo=timezone.utc)
        delta = self.check()['operator']
        self.assertEqual(delta['introduced'], 0)
        self.assertIsNone(delta['disposition_sha256'])


if __name__ == '__main__': unittest.main()
