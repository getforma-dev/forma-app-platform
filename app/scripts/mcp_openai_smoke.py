#!/usr/bin/env python3
"""APP-099: the cross-vendor rehearsal client. An OpenAI model acts through
Forma's MCP server with the Responses API's hosted `mcp` tool: it lists the
tools, asks what is waiting for the member's approval, drafts the approval of
the Nobu dinner and commits it (the prompt states the member's confirmation).

The OpenAI key is read from the gitignored `.env` (`OPENAI_API_KEY=...`, the
clone root's by default, `GENUI_ENV_FILE` to override) by the one `.env` reader
(`read_env.py`) into this process's memory only: never printed, logged, put on argv or written anywhere. Without
the file or the line the smoke is skipped (exit 0). The Forma agent token
comes from `FORMA_AGENT_TOKEN` or a local stand file (`--stand-file`, the
`approver_token` of `GENUI_MCP_STAND_FILE`) and is never printed either.

OpenAI's servers call the MCP endpoint themselves, so `--server-url` must be
an https URL reachable from the internet (the beta deployment, or a tunnel to
the local stand); a loopback or unspecified address (localhost, 127.0.0.0/8,
0.0.0.0, ::1, ::) is refused here rather than failing remotely. The request
asks OpenAI not to store the response (`store: false`) and follows no redirect.

The report (stdout, JSON) holds decisions and latencies only: the tools the
model saw, each MCP call's tool name and whether it was an error, the
response status and the wall time. No token, no key, no argument values.

Usage:
  app/scripts/mcp_openai_smoke.py --server-url https://<host>/v1/mcp \\
      [--stand-file <scratch>/mcp-stand.json] [--model gpt-4.1]
"""
import argparse
import importlib.util
import ipaddress
import json
import os
import socket
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ENDPOINT = "https://api.openai.com/v1/responses"
PROMPT = (
    "You are Sarah Chen's assistant, connected to Forma. First list what is waiting for my "
    "approval. Then draft approving the Nobu dinner expense claim, and commit that draft: I "
    "confirm the approval now. Answer in two short sentences with the record version."
)


def _read_env():
    spec = importlib.util.spec_from_file_location("read_env", Path(__file__).resolve().parent / "read_env.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


read_env = _read_env()


def read_key(env_file: Path):
    """The one variable, through the shared reader (quotes, CR and `export` stripped; duplicates refused)."""
    if not env_file.is_file():
        return None
    return read_env.read(env_file, ["OPENAI_API_KEY"]).get("OPENAI_API_KEY")


class NoRedirect(urllib.request.HTTPRedirectHandler):
    """Never follow a redirect: the request carries the OpenAI key."""

    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise urllib.error.HTTPError(req.full_url, code, "redirect refused: " + msg, headers, fp)


def unreachable_from_openai(host):
    """Loopback or unspecified: names (localhost, *.localhost) and literals (127.0.0.0/8, 0.0.0.0, ::1, ::)."""
    host = (host or "").lower().rstrip(".")
    if not host or host == "localhost" or host.endswith(".localhost"):
        return True
    try:
        address = ipaddress.ip_address(host)
    except ValueError:
        return False
    address = getattr(address, "ipv4_mapped", None) or address
    return address.is_loopback or address.is_unspecified


def agent_token(stand_file):
    token = os.environ.get("FORMA_AGENT_TOKEN")
    if token:
        return token
    if stand_file:
        return json.loads(Path(stand_file).read_text()).get("approver_token")
    return None


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--server-url", required=True)
    parser.add_argument("--stand-file")
    parser.add_argument("--model", default=os.environ.get("OPENAI_SMOKE_MODEL", "gpt-4.1"))
    args = parser.parse_args()

    env_file = Path(os.environ.get("GENUI_ENV_FILE", ROOT / ".env"))
    try:
        key = read_key(env_file)
    except read_env.EnvError as error:
        print(f"mcp openai smoke refused: {error}", file=sys.stderr)
        return 2
    if not key:
        print(f"mcp openai smoke skipped: no OPENAI_API_KEY line in {env_file}", file=sys.stderr)
        return 0
    token = agent_token(args.stand_file)
    if not token or not token.startswith("fmat_"):
        print("mcp openai smoke skipped: no Forma agent token (FORMA_AGENT_TOKEN or --stand-file)", file=sys.stderr)
        return 0
    parsed = urllib.parse.urlparse(args.server_url)
    if parsed.scheme != "https":
        print("mcp openai smoke refused: --server-url must be https (the agent token travels in its header)", file=sys.stderr)
        return 2
    if unreachable_from_openai(parsed.hostname):
        print("mcp openai smoke refused: OpenAI cannot reach a loopback server; use the beta URL or a tunnel", file=sys.stderr)
        return 2

    body = {
        "model": args.model,
        "input": PROMPT,
        # The response holds the member's records; OpenAI keeps no copy of it.
        "store": False,
        "tools": [{
            "type": "mcp",
            "server_label": "forma",
            "server_url": args.server_url,
            "headers": {"Authorization": f"Bearer {token}"},
            "require_approval": "never",
        }],
    }
    request = urllib.request.Request(
        ENDPOINT,
        data=json.dumps(body).encode(),
        headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json"},
        method="POST",
    )
    del key, token, body
    started = time.monotonic()
    try:
        with urllib.request.build_opener(NoRedirect).open(request, timeout=180) as response:
            status = response.status
            answer = json.loads(response.read())
    except urllib.error.HTTPError as error:
        # The error body may echo request members; report the status only.
        print(json.dumps({"passed": False, "status": error.code, "wall_ms": int((time.monotonic() - started) * 1000)}))
        return 1
    except (urllib.error.URLError, TimeoutError, socket.timeout, ConnectionError, json.JSONDecodeError) as error:
        # No answer (DNS, TLS, refused, timeout) or not JSON: the error kind only, never the request.
        print(json.dumps({"passed": False, "status": None, "error": type(error).__name__, "wall_ms": int((time.monotonic() - started) * 1000)}))
        return 1
    wall_ms = int((time.monotonic() - started) * 1000)
    listed, calls = [], []
    for item in answer.get("output", []):
        if item.get("type") == "mcp_list_tools":
            listed = sorted(tool.get("name", "") for tool in item.get("tools", []))
        elif item.get("type") == "mcp_call":
            calls.append({"tool": item.get("name"), "error": bool(item.get("error"))})
    committed = any(c["tool"] == "forma_commit" and not c["error"] for c in calls)
    report = {
        "passed": status == 200 and "forma_commit" in listed and committed,
        "model": answer.get("model"),
        "status": status,
        "wall_ms": wall_ms,
        "tools_listed": listed,
        "mcp_calls": calls,
    }
    print(json.dumps(report, indent=2))
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    sys.exit(main())
