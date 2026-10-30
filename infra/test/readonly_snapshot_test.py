"""APP-078 read-only snapshot guard tests. No AWS SDK or network is used."""
from datetime import datetime, timezone
import importlib.util
import json
from pathlib import Path
import unittest

PATH = Path(__file__).parents[1] / 'scripts/readonly_snapshot.py'
spec = importlib.util.spec_from_file_location('readonly_snapshot', PATH)
snapshot = importlib.util.module_from_spec(spec)
spec.loader.exec_module(snapshot)
MUTATING = ('update_service', 'run_task', 'stop_task', 'create_change_set', 'execute_change_set', 'update_stack', 'delete_stack',
            'put_object', 'create_db_snapshot', 'put_image', 'put_secret_value', 'create_schedule', 'update_schedule',
            'register_task_definition', 'deregister_task_definition', 'modify_db_instance', 'set_stack_policy', 'delete_change_set')


class Fake:
    def __getattr__(self, name):
        return lambda **kwargs: {'ResponseMetadata': {'RequestId': 'req-' + name, 'HTTPStatusCode': 200}, 'called': name}


class Guard(unittest.TestCase):
    def test_only_allowlisted_reads_reach_the_client(self):
        record = []
        for service in snapshot.ALLOWED:
            client = snapshot.ReadOnlyClient(service, Fake(), record)
            for name in MUTATING:
                with self.assertRaises(snapshot.Refused):
                    getattr(client, name)
            for name in snapshot.ALLOWED[service]:
                self.assertTrue(name.startswith(('describe_', 'get_', 'list_')), name)
                self.assertEqual(getattr(client, name)(), {'called': name})
        self.assertEqual(len(record), sum(len(v) for v in snapshot.ALLOWED.values()))
        self.assertTrue(all(r['request_id'].startswith('req-') for r in record))
        with self.assertRaises(snapshot.Refused):
            snapshot.ReadOnlyClient('iam', Fake(), record)

    def test_summary_removes_the_database_endpoint_and_operator_address(self):
        now = datetime(2026, 10, 30, tzinfo=timezone.utc)
        raw = {'caller': {'Account': '877573132739'},
               'stack': {'StackName': 'GenuiSyntheticFoundation', 'StackId': 'id', 'StackStatus': 'UPDATE_COMPLETE',
                         'Parameters': [{'ParameterKey': 'OperatorIpv4Cidr', 'ParameterValue': '8.8.8.8/32'}],
                         'Outputs': [{'OutputKey': 'DatabaseEndpoint', 'OutputValue': 'private.example.rds.amazonaws.com'}]},
               'stack_policy': None, 'change_sets': [], 'stack_resources': [], 'services': {'services': []}, 'task_definitions': {},
               'tasks': {'RUNNING': [], 'STOPPED': []}, 'databases': [], 'db_snapshots': {}, 'repositories': [], 'images': {},
               'load_balancers': [], 'listeners': [], 'target_health': {}, 'budgets': [], 'cost_month_to_date': None,
               'qualification_secrets': []}
        text = json.dumps(snapshot.summarize(raw, [], now))
        self.assertNotIn('8.8.8.8', text)
        self.assertNotIn('private.example', text)


if __name__ == '__main__':
    unittest.main()
