"""Tests for trial_checks.py against a local stub HTTP server (standard library only).

    python3 -m unittest discover -s evidence/APP-076/prep -p 'test_*.py'

The stub reproduces the service's refusal contract (app/src/http/mod.rs,
auth/routes.rs, query/routes.rs, commands/routes.rs): fetch metadata first
(403 cross_site), then the session (the uniform 404), query strings refused,
the neutral sign-in failure on a forged callback. Fault switches make it leak
in one way each, and the script must notice. With GENUI_TRIAL_BINARY set to a
built service binary (app/target/debug/forma-app-platform, admin assets
built), LocalBinaryTests also starts the real binary as the inert public shell
(no identity registry, no provider: every protected route is the uniform 404),
exactly as app/runtime/smoke_http.py does, and runs the outsider matrix
against it; GENUI_TRIAL_REPORT names where its sanitized report is written.
"""
import hashlib
import http.server
import importlib.util
import json
import os
import queue
import re
import secrets
import subprocess
import sys
import tempfile
import threading
import time
import unittest
import urllib.parse
import uuid
from pathlib import Path

HERE = Path(__file__).resolve().parent
SCRIPT = HERE / "trial_checks.py"
spec = importlib.util.spec_from_file_location("trial_checks", SCRIPT)
tc = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tc)

BOUND = "00000000-0000-4000-8000-0000000000f2"
PROBLEM_HEADERS = [("Content-Type", "application/problem+json"), ("Cache-Control", "no-store"),
                   ("X-Content-Type-Options", "nosniff"), ("Referrer-Policy", "no-referrer")]
WORK = re.compile(r"^/w/([^/]+)/([^/]+)$")
QUERY = re.compile(r"^/v1/work/([^/]+)/([^/]+)/queries/([^/]+)$")
COMMAND = re.compile(r"^/v1/work/([^/]+)/([^/]+)/commands/([^/]+)$")
OUTCOME = re.compile(r"^/v1/work/([^/]+)/([^/]+)/commands/([^/]+)/outcomes/([^/]+)$")
RECORD = re.compile(r"^/work/([^/]+)/([^/]+)/records/([^/]+)/([^/]+)$")
SIGNIN = re.compile(r"^/signin/([^/]+)$")


class Stub:
    """A faithful model of the refusal contract with optional faults."""

    def __init__(self, member=None, canary=None, faults=()):
        self.member, self.canary, self.faults = member, canary, set(faults)
        self.signin_counts = {}
        stub = self

        class Handler(http.server.BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass

            def do_GET(self):
                stub.handle(self, "GET")

            def do_POST(self):
                stub.handle(self, "POST")

        self.server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)

    def __enter__(self):
        self.thread.start()
        return f"http://127.0.0.1:{self.server.server_address[1]}"

    def __exit__(self, *exc):
        self.server.shutdown()
        self.server.server_close()

    # -- responses
    def send(self, h, status, body, headers, cookies=()):
        h.send_response(status)
        for name, value in headers:
            h.send_header(name, value)
        for value in cookies:
            h.send_header("Set-Cookie", value)
        h.send_header("Content-Length", str(len(body)))
        h.end_headers()
        h.wfile.write(body)

    def problem(self, h, status, code, extra_headers=(), cookies=()):
        body = json.dumps({"code": code, "status": status}, separators=(",", ":"), sort_keys=True).encode()
        self.send(h, status, body, PROBLEM_HEADERS + list(extra_headers), cookies)

    def not_found(self, h, path=""):
        extra = [("X-Debug", "1")] if "extra_header" in self.faults and path.startswith("/auth/pending") else []
        cookies = ["__Host-genui_trace=abc123; Path=/"] if "set_cookie_refusal" in self.faults and path == "/workspace" else []
        self.problem(h, 404, "not_found", extra, cookies)

    def page(self, h, status, text, cookies=()):
        nonce = secrets.token_urlsafe(16)
        body = f'<!doctype html><html><head><script nonce="{nonce}"></script></head><body>{text}</body></html>'.encode()
        self.send(h, status, body, [("Content-Type", "text/html; charset=utf-8"), ("Cache-Control", "no-store"),
                                    ("Content-Security-Policy", f"script-src 'nonce-{nonce}'")], cookies)

    # -- the contract
    def session(self, h):
        cookie = h.headers.get("Cookie", "")
        for part in cookie.split(";"):
            name, _, value = part.strip().partition("=")
            if name == "__Host-genui_session":
                if self.member and value == self.member:
                    return "member"
                if self.canary and value == self.canary:
                    return "canary"
        return None

    def same_origin(self, h):
        sites = h.headers.get_all("Sec-Fetch-Site") or []
        return sites == ["same-origin"]

    def handle(self, h, method):
        split = urllib.parse.urlsplit(h.path)
        path, query = split.path, split.query
        length = int(h.headers.get("Content-Length") or 0)
        body = h.rfile.read(length) if length else b""
        who = self.session(h)
        if "%FF" in path.upper():
            return self.not_found(h, path)
        if method == "GET":
            if path in ("/signin",) or SIGNIN.match(path):
                return self.page(h, 200, "Sign in")
            if path == "/auth/callback":
                return self.page(h, 400, "Sign in failed", ["__Host-genui_prelogin=; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=0"])
            if path == "/workspace":
                return self.page(h, 200, "Workspace") if who else self.not_found(h, path)
            m = WORK.match(path)
            if m:
                if "leak_known" in self.faults and m.group(2) == BOUND and not who:
                    return self.send(h, 404, b'{"code":"not_found","hint":"exists","status":404}', PROBLEM_HEADERS)
                if "canary_leak" in self.faults and who == "canary" and m.group(2) == BOUND:
                    return self.page(h, 200, "Northstar work")
                if who == "member" and m.groups() == (tc.NORTHSTAR_APP, BOUND) and not query:
                    return self.page(h, 200, "Northstar work")
            return self.not_found(h, path)
        posts = (QUERY.match(path) or WORK.match(path) or COMMAND.match(path) or SIGNIN.match(path)
                 or path in ("/auth/signout", "/auth/signout-all", "/auth/pending/resend", "/invite/validate"))
        if not posts:
            return self.not_found(h, path)
        if not self.same_origin(h):
            return self.problem(h, 403, "cross_site")
        m = SIGNIN.match(path)
        if m:
            peer = h.headers.get("X-Forwarded-For", "") if "xff_trust" in self.faults else h.client_address[0]
            self.signin_counts[peer] = self.signin_counts.get(peer, 0) + 1
            if self.signin_counts[peer] > 10:
                return self.problem(h, 429, "too_many_requests", [("Retry-After", "60")])
            return self.page(h, 400, "Sign in failed")
        if path == "/auth/signout":
            return self.send(h, 303, b"", [("Location", "/")])
        if path == "/invite/validate":
            return self.page(h, 400, "Invitation not accepted")
        # The synchronizer token is judged on the cookie's presence, before any
        # lookup: no token the stub would accept exists.
        cookie = h.headers.get("Cookie", "")
        if path == "/auth/signout-all":
            return self.problem(h, 403, "cross_site") if "__Host-genui_session=" in cookie else self.not_found(h, path)
        if path == "/auth/pending/resend":
            return self.problem(h, 403, "cross_site") if "__Host-genui_pre=" in cookie else self.not_found(h, path)
        if who is None:
            return self.not_found(h, path)
        m = QUERY.match(path)
        if m and who == "member":
            if query:
                return self.problem(h, 422, "invalid_parameters")
            if m.groups()[:2] == (tc.NORTHSTAR_APP, BOUND) and m.group(3) in ("total", "timeline"):
                parameters = json.loads(body)["parameters"]
                total = "900000" if parameters.get("destination") == tc.DESTINATION else "1140000"
                rows = [[total]] if m.group(3) == "total" else [["2026-10-22T10:00:00Z", tc.DESTINATION, "240000"]]
                document = {"view_id": m.group(3), "columns": ["total_bytes"], "rows": rows, "truncated_page": False,
                            "receipt": {"digest": "sha256:" + "1" * 64, "result_digest": "sha256:" + "2" * 64}}
                return self.send(h, 200, json.dumps(document).encode(), [("Content-Type", "application/json"), ("Cache-Control", "no-store")])
        return self.not_found(h, path)


def run(args, private=None):
    command = [sys.executable, "-I", str(SCRIPT)] + args
    with tempfile.TemporaryDirectory() as directory:
        output = Path(directory) / "report.json"
        if private is not None:
            path = Path(directory) / "private.json"
            path.write_text(json.dumps(private))
            path.chmod(0o600)
            command += ["--private-input", str(path)]
        command += ["--output", str(output)]
        done = subprocess.run(command, capture_output=True, text=True, timeout=120)
        report = json.loads(output.read_text()) if output.exists() else None
        return done, report


def matrix_args(base, *extra):
    return ["matrix", "--base-url", base, "--bound-work", BOUND, "--timeout", "5", "--pace", "0", *extra]


class InputTests(unittest.TestCase):
    def test_plaintext_is_refused_off_loopback(self):
        with self.assertRaises(tc.Refused):
            tc.base_url("http://beta.getforma.dev")
        for url in ("https://beta.getforma.dev/x", "https://u:p@beta.getforma.dev", "https://beta.getforma.dev?x=1", "ftp://x"):
            with self.assertRaises(tc.Refused):
                tc.base_url(url)
        self.assertEqual(tc.base_url("https://beta.getforma.dev"), ("https", "beta.getforma.dev", None))
        self.assertEqual(tc.base_url("http://127.0.0.1:3000"), ("http", "127.0.0.1", 3000))

    def test_private_input_must_be_0600_and_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "p.json"
            path.write_text(json.dumps({"member_session": secrets.token_urlsafe(32)}))
            path.chmod(0o644)
            with self.assertRaises(tc.Refused):
                tc.private_input(str(path))
            path.chmod(0o600)
            self.assertEqual(set(tc.private_input(str(path))), {"member_session"})
            path.write_text(json.dumps({"member_session": "short"}))
            with self.assertRaises(tc.Refused):
                tc.private_input(str(path))
            path.write_text(json.dumps({"password": secrets.token_urlsafe(32)}))
            with self.assertRaises(tc.Refused):
                tc.private_input(str(path))
            link = Path(directory) / "link.json"
            link.symlink_to(path)
            with self.assertRaises(tc.Refused):
                tc.private_input(str(link))

    def test_no_secret_is_accepted_in_argv(self):
        options = {action.dest for sub in tc.parser()._subparsers._group_actions for p in sub.choices.values() for action in p._actions}
        self.assertFalse(options & {"cookie", "session", "member_session", "canary_session", "token", "code", "password"})

    def test_probe_commands_carry_an_unreachable_premise(self):
        body = json.loads(tc.command_body(tc.NORTHSTAR_APP, BOUND, tc.EXP_002, "0" * 32))
        self.assertEqual(body["expected_work_version"], 999999)


class StubMatrixTests(unittest.TestCase):
    def test_faithful_stub_passes_and_the_report_is_sanitized(self):
        member, canary = secrets.token_urlsafe(32), secrets.token_urlsafe(32)
        with Stub(member=member, canary=canary) as base:
            done, report = run(matrix_args(base, "--check", "matrix,canary,controls,forwarded-ip"),
                               {"member_session": member, "canary_session": canary})
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        self.assertTrue(report["passed"])
        for name in ("not_found", "cross_site", "callback"):
            self.assertTrue(report["classes"][name]["uniform"], name)
        self.assertEqual(report["classes"]["not_found"]["statuses"], [404])
        self.assertEqual(report["classes"]["cross_site"]["statuses"], [403])
        contexts = {row["context"] for row in report["rows"] if row["class"] == "not_found"}
        self.assertTrue({"anonymous", "forged_session_cookie", "forged_bearer", "forged_organization_headers",
                         "real_member", "canary_replay"} <= contexts)
        totals = {c["id"]: c.get("result", {}).get("total") for c in report["controls"]}
        self.assertEqual(totals["real_member.control_total_window"], "1140000")
        self.assertEqual(totals["real_member.control_total_destination"], "900000")
        self.assertTrue(any(r["id"].startswith("forwarded_ip.") and r["status"] == 429 for r in report["rows"]))
        text = json.dumps(report) + done.stdout + done.stderr
        self.assertNotIn(member, text)
        self.assertNotIn(canary, text)
        self.assertEqual(report["inputs"]["private_input_keys"], ["canary_session", "member_session"])
        # The script's own scan finds nothing secret-shaped in its report.
        with tempfile.TemporaryDirectory() as directory:
            Path(directory, "report.json").write_text(json.dumps(report, indent=2))
            files, findings = tc.scan_tree(directory)
        self.assertEqual((files, findings), (1, []))

    def test_outsider_matrix_runs_without_any_private_input(self):
        with Stub() as base:
            done, report = run(matrix_args(base, "--check", "matrix"))
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        self.assertEqual(report["inputs"]["private_input_keys"], [])
        self.assertTrue(any("member_session" in line for line in report["not_run"]))
        self.assertFalse(report["controls"])

    def assert_fails(self, faults, failing, private=None, checks="matrix,canary,controls"):
        member, canary = secrets.token_urlsafe(32), secrets.token_urlsafe(32)
        keys = private if private is not None else {}
        values = {"member_session": member, "canary_session": canary}
        with Stub(member=member, canary=canary, faults=faults) as base:
            done, report = run(matrix_args(base, "--check", checks), {k: values[k] for k in keys} if keys else None)
        self.assertEqual(done.returncode, 1, done.stdout + done.stderr)
        self.assertFalse(report["passed"])
        self.assertTrue(any(failing in item for item in report["failures"]), report["failures"])
        return report

    def test_existence_disclosure_on_a_known_id_is_caught(self):
        report = self.assert_fails({"leak_known"}, "anonymous.work_page_known")
        self.assertFalse(report["classes"]["not_found"]["uniform"])

    def test_a_differing_header_set_is_caught(self):
        report = self.assert_fails({"extra_header"}, "class:not_found")
        self.assertEqual(report["classes"]["not_found"]["distinct_header_sets"], 2)

    def test_a_cookie_on_a_refusal_is_caught(self):
        self.assert_fails({"set_cookie_refusal"}, "anonymous.workspace")

    def test_canary_reaching_northstar_is_caught(self):
        self.assert_fails({"canary_leak"}, "canary_replay.work_page_known", private=["canary_session"])

    def test_a_trusted_forwarded_header_is_caught(self):
        self.assert_fails({"xff_trust"}, "forwarded_ip.", checks="forwarded-ip")

    def test_revoked_canary_joins_the_uniform_class(self):
        canary = secrets.token_urlsafe(32)
        with Stub() as base:  # the stub no longer knows the cookie: revoked
            done, report = run(matrix_args(base, "--check", "canary", "--canary-revoked"), {"canary_session": canary})
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        self.assertIn("canary_replay.workspace_after_revocation", {r["id"] for r in report["rows"]})


class ScanTests(unittest.TestCase):
    def test_self_test_and_findings_never_print_the_match(self):
        with tempfile.TemporaryDirectory() as directory:
            secret = "".join(secrets.choice("ABCDEFGHIJKLMNOPQRSTUVWXYZ234567") for _ in range(52))
            Path(directory, "leak.md").write_text(f"code {secret}\n")
            Path(directory, "clean.json").write_text(json.dumps({"digest": "sha256:" + "a" * 64, "id": str(uuid.uuid4())}))
            done = subprocess.run([sys.executable, "-I", str(SCRIPT), "scan", "--root", directory, "--self-test"],
                                  capture_output=True, text=True, timeout=60)
        self.assertEqual(done.returncode, 1)
        report = json.loads(done.stdout)
        self.assertTrue(report["self_test"]["passed"])
        self.assertEqual([(f["file"], f["pattern"]) for f in report["findings"]], [("leak.md", "access_code_base32")])
        self.assertNotIn(secret, done.stdout + done.stderr)

    def test_clean_tree_passes(self):
        with tempfile.TemporaryDirectory() as directory:
            Path(directory, "a.json").write_text(json.dumps({"handle": "se45m37vabqrfvbh3gbwn22qkm", "sha": hashlib.sha256(b"x").hexdigest()}))
            files, findings = tc.scan_tree(directory)
        self.assertEqual((files, findings), (1, []))


@unittest.skipUnless(os.environ.get("GENUI_TRIAL_BINARY"), "set GENUI_TRIAL_BINARY to a built service binary")
class LocalBinaryTests(unittest.TestCase):
    """The real router as the inert public shell (smoke_http.py's launch)."""

    def test_outsider_matrix_against_the_real_binary(self):
        binary = os.environ["GENUI_TRIAL_BINARY"]
        with tempfile.TemporaryDirectory(prefix="app076-local-") as directory:
            path = Path(directory) / "credentials.json"
            now = int(time.time())
            document = {"schema_version": 1, "generation": str(uuid.uuid4()), "issued_at": now, "expires_at": now + 500,
                        "database": {"host": "synthetic.invalid", "port": 5432, "name": "genui",
                                     "roles": {n: {"username": "genui_" + n, "token": "unit-only-" + n, "expires_at": now + 500}
                                               for n in ("workspace", "control", "data", "worker")}},
                        "storage": {}}
            path.write_text(json.dumps(document))
            path.chmod(0o600)
            helper = Path(directory) / "helper.py"
            helper.write_text("import sys; sys.exit(1)\n")
            env = {k: v for k, v in os.environ.items() if not k.startswith(("AWS_", "GENUI_"))}
            env.update({"GENUI_RUNTIME_CONFIG_FILE": str(path), "GENUI_HELPER_PATH": str(helper),
                        "AWS_EC2_METADATA_DISABLED": "true", "FORMA_PORT": "0", "FORMA_BIND_ADDRESS": "127.0.0.1"})
            process = subprocess.Popen([binary], env=env, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
            try:
                lines = queue.Queue()
                threading.Thread(target=lambda: [lines.put(l.strip()) for l in process.stdout] + [lines.put(None)], daemon=True).start()
                deadline = time.monotonic() + 20
                while True:
                    line = lines.get(timeout=max(0.01, deadline - time.monotonic()))
                    self.assertIsNotNone(line, "server exited before binding")
                    if line.startswith("FORMA_LISTENING http://127.0.0.1:"):
                        break
                base = line.split(" ", 1)[1]
                done, report = run(matrix_args(base, "--check", "matrix,forwarded-ip", "--label", "local-binary"))
            finally:
                process.terminate()
                try:
                    process.communicate(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.communicate()
        if os.environ.get("GENUI_TRIAL_REPORT"):
            Path(os.environ["GENUI_TRIAL_REPORT"]).write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
        self.assertIsNotNone(report, done.stderr)
        # G7 and G8 (RUNBOOK.md §Gaps) are fixed: an undecodable segment and a
        # method mismatch on a mounted route answer the uniform 404 bytes with
        # the same header set as an unknown path (`http::UniformPath`,
        # `http::uniform`). Every row passes and every class is uniform.
        self.assertEqual(report["failures"], [], report["failures"])
        self.assertTrue(report["passed"])
        not_found = [r for r in report["rows"] if r["class"] == "not_found"]
        self.assertTrue(all(r["passed"] for r in not_found))
        self.assertEqual(len({r["body_sha256"] for r in not_found}), 1)
        self.assertEqual(len({r["header_fingerprint"] for r in not_found}), 1)
        self.assertTrue(all("allow" not in r["header_names"] for r in report["rows"]))
        self.assertTrue(all(c["uniform"] for c in report["classes"].values()), report["classes"])


if __name__ == "__main__":
    unittest.main()
