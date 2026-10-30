"""APP-113: the one `.env` reader strips quotes, CR and `export`, and refuses duplicates and missing names.

Run: python3 -m unittest discover -s app/scripts -p 'test_*.py'
"""
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

PATH = Path(__file__).resolve().parent / "read_env.py"
spec = importlib.util.spec_from_file_location("read_env", PATH)
read_env = importlib.util.module_from_spec(spec)
spec.loader.exec_module(read_env)
FAKE_KEY = "sk-ant-synthetic-0123456789"  # synthetic, not a credential


class ReadEnv(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.path = Path(self.directory.name) / ".env"

    def tearDown(self):
        self.directory.cleanup()

    def write(self, text, newline="\n"):
        self.path.write_bytes(text.replace("\n", newline).encode())

    def run_cli(self, *args):
        return subprocess.run([sys.executable, str(PATH), "--file", str(self.path), *args], capture_output=True, text=True)

    def test_quotes_crlf_export_and_inline_comments_are_stripped(self):
        cases = {
            'ANTHROPIC_API_KEY=%s\n': FAKE_KEY,
            'ANTHROPIC_API_KEY="%s"\n': FAKE_KEY,
            "ANTHROPIC_API_KEY='%s'\n": FAKE_KEY,
            "export ANTHROPIC_API_KEY=%s\n": FAKE_KEY,
            'export   ANTHROPIC_API_KEY = "%s"  \n': FAKE_KEY,
            "ANTHROPIC_API_KEY=%s # the pitch key\n": FAKE_KEY,
            '  # ANTHROPIC_API_KEY=commented-out\nANTHROPIC_API_KEY=%s\n': FAKE_KEY,
        }
        for template, expected in cases.items():
            for newline in ("\n", "\r\n"):
                with self.subTest(template=template, newline=repr(newline)):
                    self.write(template % FAKE_KEY, newline)
                    self.assertEqual(read_env.read(self.path, ["ANTHROPIC_API_KEY"]), {"ANTHROPIC_API_KEY": expected})
                    done = self.run_cli("--raw", "ANTHROPIC_API_KEY")
                    self.assertEqual((done.returncode, done.stdout), (0, expected), done.stderr)

    def test_slack_object_is_one_compact_json_value(self):
        self.write('OTHER=1\nSLACK_BOT_TOKEN="xoxb-synthetic"\r\nexport SLACK_SIGNING_SECRET=abc123\n')
        done = self.run_cli("--object", "bot_token=SLACK_BOT_TOKEN", "signing_secret=SLACK_SIGNING_SECRET")
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertEqual(json.loads(done.stdout), {"bot_token": "xoxb-synthetic", "signing_secret": "abc123"})
        self.assertFalse(done.stdout.endswith("\n"))

    def test_duplicates_missing_empty_and_bad_values_refuse_without_printing_values(self):
        for text, message in (
            ("ANTHROPIC_API_KEY=%s\nexport ANTHROPIC_API_KEY=%s\n" % (FAKE_KEY, FAKE_KEY), "more than once"),
            ("OTHER=%s\n" % FAKE_KEY, "missing or empty"),
            ("ANTHROPIC_API_KEY=\n", "missing or empty"),
            ('ANTHROPIC_API_KEY=""\n', "missing or empty"),
            ('ANTHROPIC_API_KEY="%s\n' % FAKE_KEY, "unbalanced quote"),
            ("ANTHROPIC_API_KEY=%s\x0bx\n" % FAKE_KEY, "control character"),
            ("ANTHROPIC_API_KEY=%s\x85\n" % FAKE_KEY, "control character"),
        ):
            with self.subTest(message=message):
                self.write(text)
                done = self.run_cli("--raw", "ANTHROPIC_API_KEY")
                self.assertEqual((done.returncode, done.stdout), (2, ""))
                self.assertIn(message, done.stderr)
                self.assertNotIn(FAKE_KEY, done.stderr)
        # A duplicate of a name nobody asked for is not this reader's business.
        self.write("OTHER=1\nOTHER=2\nANTHROPIC_API_KEY=%s\n" % FAKE_KEY)
        self.assertEqual(self.run_cli("--raw", "ANTHROPIC_API_KEY").stdout, FAKE_KEY)

    def test_optional_missing_exits_three_with_no_output(self):
        self.write("OTHER=1\n")
        done = self.run_cli("--optional", "--raw", "OPENAI_API_KEY")
        self.assertEqual((done.returncode, done.stdout), (3, ""))
        self.path.unlink()
        self.assertEqual(self.run_cli("--optional", "--raw", "OPENAI_API_KEY").returncode, 3)
        self.assertEqual(self.run_cli("--raw", "OPENAI_API_KEY").returncode, 2)
        # Duplicates still refuse under --optional.
        self.write("OPENAI_API_KEY=a\nOPENAI_API_KEY=b\n")
        self.assertEqual(self.run_cli("--optional", "--raw", "OPENAI_API_KEY").returncode, 2)

    def test_bad_arguments(self):
        self.write("A=1\n")
        for args in ([], ["--raw", "A", "--object", "a=A"], ["--object", "a"], ["--object", "a=A", "a=A"]):
            with self.subTest(args=args):
                self.assertEqual(self.run_cli(*args).returncode, 2)
        with self.assertRaises(read_env.EnvError):
            read_env.read(self.path, ["not-a-name"])


if __name__ == "__main__":
    unittest.main()
