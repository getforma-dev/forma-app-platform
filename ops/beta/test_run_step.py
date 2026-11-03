"""Operator task lifecycle in a disposable checkout/private directory.

The AWS executable is a closed shim: no real AWS call or shared ledger is
reachable. The local composer uses the real request/token function with dummy
outputs, so two logical runs must start two tasks, while resume observes one.
"""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
WINDOW = "11111111-2222-4333-8444-555555555555"

COMPOSER = '''import argparse, hashlib, importlib.util, json, os
from pathlib import Path
p = argparse.ArgumentParser()
p.add_argument('command'); p.add_argument('--outputs'); p.add_argument('--window-id')
p.add_argument('--step'); p.add_argument('--override'); p.add_argument('--output')
a = p.parse_args()
spec = importlib.util.spec_from_file_location('real_composer', Path(__file__).with_name('request_impl.py'))
real = importlib.util.module_from_spec(spec); spec.loader.exec_module(real)
outputs = {'ClusterName': 'synthetic-cluster', 'PublicTaskSubnets': 'subnet-00000001,subnet-00000002',
           'TaskSecurityGroup': 'sg-00000001',
           'OperatorTaskDefinition': 'arn:aws:ecs:us-east-1:877573132739:task-definition/Synthetic:1'}
request = real.run_task_request(outputs, 'OperatorTaskDefinition',
    {'containerOverrides': [{'name': 'operator', 'command': [a.override]}]}, a.window_id, a.step)
data = (json.dumps(request) + '\\n').encode(); Path(a.output).write_bytes(data)
with open(os.environ['FAKE_COMPOSE_LOG'], 'a') as f: f.write(a.step + '\\n')
print(json.dumps({'request': a.output, 'sha256': hashlib.sha256(data).hexdigest()}))
'''

AWS = '''#!/usr/bin/env python3
import json, os, sys
from pathlib import Path
args = sys.argv[1:]
with open(os.environ['FAKE_AWS_LOG'], 'a') as f: f.write(json.dumps(args) + '\\n')
state_path = Path(os.environ['FAKE_AWS_STATE'])
state = json.loads(state_path.read_text()) if state_path.exists() else {}
if args[:2] == ['ecs', 'run-task']:
    request = json.loads(Path(args[args.index('--cli-input-json') + 1][7:]).read_text())
    token = request['clientToken']
    if token not in state:
        state[token] = {'arn': 'arn:aws:ecs:us-east-1:877573132739:task/synthetic/' + str(len(state) + 1),
                        'request': request}
    state_path.write_text(json.dumps(state))
    if os.environ.get('FAKE_UNKNOWN_START') == '1': sys.exit(69)
    print(json.dumps({'tasks': [{'taskArn': state[token]['arn']}], 'failures': []}))
elif args[:3] == ['ecs', 'wait', 'tasks-stopped']:
    sys.exit(13 if os.environ.get('FAKE_WAIT_FAIL') == '1' else 0)
elif args[:2] == ['ecs', 'describe-tasks']:
    container = {'name': 'operator'}
    code = os.environ.get('FAKE_TASK_EXIT', '0')
    if code != 'missing': container['exitCode'] = json.loads(code)
    print(json.dumps({'tasks': [{'taskArn': args[args.index('--tasks') + 1], 'lastStatus': 'STOPPED',
        'containers': [container], 'stopCode': 'EssentialContainerExited', 'stoppedReason': 'synthetic'}]}))
elif args[:2] == ['logs', 'get-log-events']:
    if os.environ.get('FAKE_LOG_FAIL') == '1': sys.exit(9)
    print(json.dumps({'events': [{'message': 'synthetic operator output'}]}))
else:
    print('unrecognized fake AWS request', file=sys.stderr); sys.exit(97)
'''


class RunStepTests(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory(prefix="forma task lifecycle ")
        self.addCleanup(self.scratch.cleanup)
        self.root = Path(self.scratch.name)
        self.repo, self.priv, self.shims = (self.root / name for name in ("repo", "priv", "shims"))
        scripts = self.repo / "ops/beta"
        scripts.mkdir(parents=True)
        for name in ("run-step.sh", "lib.sh", "deploy.sh", "release.py"):
            shutil.copy2(HERE / name, scripts / name)
        infra = self.repo / "infra/scripts"
        infra.mkdir(parents=True)
        shutil.copy2(HERE.parents[1] / "infra/scripts/window_requests.py", infra / "request_impl.py")
        shutil.copy2(HERE.parents[1] / "infra/scripts/common.py", infra / "common.py")
        (infra / "window_requests.py").write_text(COMPOSER)
        self.shims.mkdir()
        (self.shims / "aws").write_text(AWS)
        (self.shims / "aws").chmod(0o755)
        self.priv.mkdir(mode=0o700)
        (self.priv / "env.sh").write_text("AWS_REGION=synthetic\nCLUSTER=synthetic-cluster\nLOGS=synthetic-logs\n")
        self.window = self.priv / "windows" / WINDOW
        self.window.mkdir(parents=True)
        (self.window / "stack-after.json").write_text("{}")
        (self.window / "release.json").write_text(json.dumps({
            "operator_tasks": [], "approvals": [], "migrations": {"migrated": False}}))
        self.env = {k: v for k, v in os.environ.items() if not k.startswith(("AWS_", "FAKE_"))}
        self.env.update(GENUI_OPS_PRIV=str(self.priv), FAKE_AWS_LOG=str(self.root / "aws.jsonl"),
                        FAKE_AWS_STATE=str(self.root / "aws-state.json"),
                        FAKE_COMPOSE_LOG=str(self.root / "compose.log"),
                        PATH=str(self.shims) + os.pathsep + os.environ["PATH"])

    def run_step(self, *, code="0", resume=None, extra=(), env=None, parent=False, override="migrate"):
        if parent:
            args = ["bash", str(self.repo / "ops/beta/deploy.sh"), "migrate", WINDOW,
                    "--approved", "synthetic test only"]
        else:
            args = ["bash", str(self.repo / "ops/beta/run-step.sh"), "--window", WINDOW,
                    "--approved", "synthetic test only"]
            if resume is not None:
                args += ["--resume-run", str(resume)]
            args += ["migrate", override, *extra]
        return subprocess.run(args, cwd=self.repo, env=dict(self.env, FAKE_TASK_EXIT=code, **(env or {})),
                              stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=15)

    def record(self):
        return json.loads((self.window / "release.json").read_text())

    def calls(self):
        path = self.root / "aws.jsonl"
        return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []

    def test_confirmed_zero_is_journaled_and_marks_migrated(self):
        result = self.run_step()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        record = self.record()
        self.assertTrue(record["migrations"]["migrated"])
        self.assertEqual(record["operator_tasks"][0]["exit_code"], 0)
        self.assertEqual(record["operator_tasks"][0]["run"], 1)
        self.assertTrue((self.window / "run-migrate-1.stopped.json").exists())

    def assert_failed_exit(self, code, recorded):
        result = self.run_step(code=code)
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("did not exit successfully", result.stderr)
        record = self.record()
        self.assertFalse(record["migrations"]["migrated"])
        self.assertEqual(len(record["operator_tasks"]), 1)
        self.assertEqual(record["operator_tasks"][0]["exit_code"], recorded)
        self.assertTrue((self.window / "run-migrate-1.stopped.json").exists())

    def test_failed_exit_is_journaled_before_process_failure(self):
        self.assert_failed_exit("7", 7)

    def test_missing_exit_is_journaled_before_process_failure(self):
        self.assert_failed_exit("missing", None)

    def test_null_exit_is_journaled_before_process_failure(self):
        self.assert_failed_exit("null", None)

    def test_string_zero_is_not_a_confirmed_success(self):
        self.assert_failed_exit('"0"', None)

    def test_failed_migration_stops_parent_before_catalog_task(self):
        result = self.run_step(code="4", parent=True)
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual([c[:2] for c in self.calls()].count(["ecs", "run-task"]), 1)
        self.assertEqual([t["label"] for t in self.record()["operator_tasks"]], ["migrate"])
        self.assertFalse(self.record()["migrations"]["migrated"])

    def test_log_outage_does_not_prevent_failed_task_receipt(self):
        result = self.run_step(code="5", env={"FAKE_LOG_FAIL": "1"})
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.record()["operator_tasks"][0]["exit_code"], 5)

    def test_two_confirmed_logical_runs_start_two_distinct_tasks(self):
        for _ in range(2):
            result = self.run_step()
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        requests = [json.loads((self.window / f"run-migrate-{n}.json").read_text()) for n in (1, 2)]
        self.assertNotEqual(requests[0]["clientToken"], requests[1]["clientToken"])
        self.assertEqual([r["startedBy"] for r in requests], ["app078-migrate-1", "app078-migrate-2"])
        state = json.loads((self.root / "aws-state.json").read_text())
        self.assertEqual(len(state), 2, "the fake ECS deduplicates identical RunTask tokens")
        self.assertEqual([t["run"] for t in self.record()["operator_tasks"]], [1, 2])

    def test_resume_preserves_request_and_waits_for_original_task_without_restarting(self):
        first = self.run_step(env={"FAKE_WAIT_FAIL": "1"})
        self.assertNotEqual(first.returncode, 0)
        request = (self.window / "run-migrate-1.json").read_bytes()
        receipt = (self.window / "run-migrate-1.receipt.json").read_bytes()
        refused = self.run_step()
        self.assertNotEqual(refused.returncode, 0)
        self.assertIn("unresolved", refused.stderr)
        result = self.run_step(resume=1)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual((self.window / "run-migrate-1.json").read_bytes(), request)
        self.assertEqual((self.window / "run-migrate-1.receipt.json").read_bytes(), receipt)
        self.assertEqual((self.window / "run-migrate-1-resume-1.receipt.json").read_bytes(), receipt)
        self.assertEqual((self.root / "compose.log").read_text().splitlines(), ["migrate-1"])
        self.assertEqual([c[:2] for c in self.calls()].count(["ecs", "run-task"]), 1)
        self.assertEqual(self.record()["operator_tasks"][0]["run"], 1)

    def test_unknown_start_is_not_blindly_replayed(self):
        first = self.run_step(env={"FAKE_UNKNOWN_START": "1"})
        self.assertNotEqual(first.returncode, 0)
        self.assertEqual(len(json.loads((self.root / "aws-state.json").read_text())), 1)
        self.assertNotEqual(self.run_step().returncode, 0)
        resumed = self.run_step(resume=1)
        self.assertNotEqual(resumed.returncode, 0)
        self.assertIn("RunTask acceptance is unknown", resumed.stderr)
        self.assertEqual([c[:2] for c in self.calls()].count(["ecs", "run-task"]), 1)
        self.assertEqual(self.record()["operator_tasks"], [])

    def test_resume_refuses_modified_request_or_extra_arguments_before_aws(self):
        self.run_step(env={"FAKE_WAIT_FAIL": "1"})
        count = len(self.calls())
        different = self.run_step(resume=1, override="verify-catalog")
        self.assertNotEqual(different.returncode, 0)
        self.assertIn("resume override differs", different.stderr)
        self.assertNotEqual(self.run_step(resume=1, extra=("--set=--different=yes",)).returncode, 0)
        request = self.window / "run-migrate-1.json"
        request.write_text(request.read_text() + " ")
        result = self.run_step(resume=1)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("original request changed", result.stderr)
        self.assertEqual(len(self.calls()), count)


if __name__ == "__main__":
    unittest.main()
