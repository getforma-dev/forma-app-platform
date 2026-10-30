"""APP-113: the MCP smoke scripts count JSON-RPC errors as failures, keep credentials off
redirects and plain http, and refuse addresses OpenAI cannot reach. No network: the HTTP
layer is replaced in each test.

Run: python3 -m unittest discover -s app/scripts -p 'test_*.py'
"""
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock
import urllib.error
import urllib.request

HERE = Path(__file__).resolve().parent


def load(name):
    spec = importlib.util.spec_from_file_location(name, HERE / (name + ".py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


haiku = load("mcp_haiku_agent")
openai = load("mcp_openai_smoke")
TOKEN = "fmat_synthetic"  # synthetic, not a credential


class HaikuAgent(unittest.TestCase):
    def test_json_rpc_error_answers_raise_instead_of_returning_an_empty_result(self):
        forma = haiku.Forma("https://beta.example.test", TOKEN)
        for reply in ({"jsonrpc": "2.0", "id": 1, "error": {"code": -32602, "message": "draft not found"}},
                      {"jsonrpc": "2.0", "id": 1}, {"jsonrpc": "2.0", "id": 1, "result": None}, ["not", "an", "object"]):
            with self.subTest(reply=reply), mock.patch.object(haiku, "post", return_value=reply):
                with self.assertRaises(haiku.RpcError):
                    forma.rpc("tools/call", {"name": "forma_commit", "arguments": {}})
        with mock.patch.object(haiku, "post", return_value={"jsonrpc": "2.0", "id": 1, "result": {"tools": []}}):
            self.assertEqual(forma.rpc("tools/list", {}), {"tools": []})

    def test_a_failed_commit_call_is_an_error_and_the_run_fails(self):
        replies = iter([
            {"result": {"tools": [{"name": "forma_commit", "inputSchema": {"type": "object"}}]}},
            {"jsonrpc": "2.0", "id": 2, "error": {"code": -32000, "message": "stale version"}},
        ])
        model = iter([
            {"content": [{"type": "tool_use", "id": "t1", "name": "forma_commit", "input": {}}]},
            {"content": [{"type": "text", "text": "Done."}]},
        ])

        def fake_post(url, body, headers, timeout=60):
            return next(model) if url == haiku.MESSAGES else next(replies)

        with tempfile.TemporaryDirectory() as directory:
            env = Path(directory) / ".env"; env.write_text("ANTHROPIC_API_KEY=sk-ant-synthetic\n")
            stand = Path(directory) / "stand.json"; stand.write_text(json.dumps({"sarah_token": TOKEN, "origin": "http://127.0.0.1:3000"}))
            out = io.StringIO()
            with mock.patch.object(haiku, "post", side_effect=fake_post), mock.patch.dict(os.environ, {"GENUI_ENV_FILE": str(env)}), \
                    mock.patch("sys.argv", ["mcp_haiku_agent.py", "--stand-file", str(stand)]), contextlib.redirect_stdout(out):
                code = haiku.main()
        report = json.loads(out.getvalue())
        self.assertEqual(code, 1)
        self.assertEqual(report["calls"][0]["tool"], "forma_commit")
        self.assertTrue(report["calls"][0]["is_error"])
        self.assertFalse(report["committed"])
        self.assertNotIn(TOKEN, out.getvalue())

    def test_origin_must_be_https_unless_loopback(self):
        for origin, allowed in (("https://beta.getforma.dev", True), ("http://127.0.0.1:3000", True), ("http://127.8.9.10:3000", True),
                                ("http://localhost:3000", True), ("http://[::1]:3000", True), ("http://app.localhost", True),
                                ("http://beta.getforma.dev", False), ("http://10.0.0.5:3000", False), ("ftp://127.0.0.1", False),
                                ("http://0.0.0.0:3000", False)):
            with self.subTest(origin=origin):
                self.assertEqual(haiku.origin_allowed(origin), allowed)

    def test_redirects_are_never_followed(self):
        handler = haiku.NoRedirect()
        request = urllib.request.Request("https://beta.example.test/v1/mcp", headers={"Authorization": "Bearer " + TOKEN})
        with self.assertRaises(urllib.error.HTTPError):
            handler.redirect_request(request, None, 302, "Found", {}, "https://elsewhere.example.test/")


class OpenAiSmoke(unittest.TestCase):
    def test_loopback_and_unspecified_addresses_are_unreachable(self):
        for host in ("localhost", "app.localhost", "127.0.0.1", "127.1.2.3", "0.0.0.0", "::1", "::", "::ffff:127.0.0.1", "", None, "LOCALHOST."):
            with self.subTest(host=host):
                self.assertTrue(openai.unreachable_from_openai(host))
        for host in ("beta.getforma.dev", "8.8.8.8", "2001:4860:4860::8888"):
            self.assertFalse(openai.unreachable_from_openai(host))

    def run_main(self, *argv, opener=None):
        with tempfile.TemporaryDirectory() as directory:
            env = Path(directory) / ".env"; env.write_text('OPENAI_API_KEY="sk-synthetic"\r\n')
            err, out = io.StringIO(), io.StringIO()
            patches = [mock.patch.dict(os.environ, {"GENUI_ENV_FILE": str(env), "FORMA_AGENT_TOKEN": TOKEN}),
                       mock.patch("sys.argv", ["mcp_openai_smoke.py", *argv]), contextlib.redirect_stderr(err), contextlib.redirect_stdout(out)]
            if opener is not None:
                patches.append(mock.patch.object(openai.urllib.request, "build_opener", return_value=opener))
            with contextlib.ExitStack() as stack:
                for patch in patches:
                    stack.enter_context(patch)
                code = openai.main()
        return code, out.getvalue(), err.getvalue()

    def test_plain_http_and_loopback_server_urls_are_refused_before_any_request(self):
        for url in ("http://beta.getforma.dev/v1/mcp", "https://127.0.0.1/v1/mcp", "https://0.0.0.0/v1/mcp", "https://[::1]/v1/mcp"):
            with self.subTest(url=url):
                code, _, err = self.run_main("--server-url", url, opener=mock.Mock(side_effect=AssertionError("no request")))
                self.assertEqual(code, 2)
                self.assertNotIn(TOKEN, err)

    def test_the_request_asks_for_no_storage_and_network_failures_report_without_secrets(self):
        sent = {}

        class Opener:
            def open(self, request, timeout):
                sent["body"] = json.loads(request.data)
                raise urllib.error.URLError("synthetic DNS failure")

        code, out, _ = self.run_main("--server-url", "https://beta.example.test/v1/mcp", opener=Opener())
        self.assertEqual(code, 1)
        self.assertIs(sent["body"]["store"], False)
        report = json.loads(out)
        self.assertEqual((report["passed"], report["status"], report["error"]), (False, None, "URLError"))
        self.assertNotIn(TOKEN, out); self.assertNotIn("sk-synthetic", out)

        class Slow:
            def open(self, request, timeout):
                raise TimeoutError("synthetic")

        code, out, _ = self.run_main("--server-url", "https://beta.example.test/v1/mcp", opener=Slow())
        self.assertEqual((code, json.loads(out)["error"]), (1, "TimeoutError"))


if __name__ == "__main__":
    unittest.main()
