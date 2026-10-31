#!/usr/bin/env python3
"""APP-107: the "second vendor" agent loop on the local stand (APP-098 DS-69:
a Claude Haiku model stands in for OpenAI until its key is added; Victor,
2026-10-30: "for now just mock [OpenAI] with lower-level models").

The script gives Claude Haiku exactly the tools Forma's MCP server lists
(`tools/list` over `POST /v1/mcp` with the member's agent token), asks it, as
Sarah, "what's waiting for my approval? approve the Figma seat purchase",
and executes each tool call it makes against the server (`tools/call`),
returning the result to the model until it answers. The commit is the
server's audited command path; the record then carries "via <label>" (the
token's label, "Haiku" on the stand). Nothing the model says is trusted:
every argument is judged by the tool and storage.

The Anthropic key is read from a gitignored `.env` (`ANTHROPIC_API_KEY=...`;
`GENUI_ENV_FILE`, else the repository root's) by the one `.env` reader
(`read_env.py`) into this process's memory only: never printed, logged, put on
argv or written anywhere. Without it the loop is skipped (exit 0). The Forma token comes from `FORMA_AGENT_TOKEN` or
the stand file's `sarah_token` (`GENUI_WORKFLOWS_STAND_FILE` of
`storage::workflows_campaign`) and is never printed either.

The report (stdout, JSON) holds decisions and latencies only: the tool names
the model called, whether each was an error, the model's turn count and the
wall times. No token, no key, no argument values. A JSON-RPC error answer
counts as a failed call (and a failed `tools/list` fails the run). The Forma
origin must be https unless it is a loopback stand, and no request follows a
redirect, so neither credential is ever replayed to another location.

APP-115 `--scope-demo` (the governance demo moment, rehearsable, no model):
with a token scoped to the Security space (`FORMA_SCOPED_AGENT_TOKEN`, or the
stand file's `security_token` from `storage::mcp_campaign`'s stand), it lists
the apps the token sees and then tries to draft a vacation request in the
People space's Time off app. Forma refuses with the uniform "Not found." and
the canvas's Connect an agent -> Activity panel shows the call as "refused:
outside scope (People)". The report says whether Time off was listed (it must
not be) and whether the draft was refused (it must be); exit 0 only then.

Usage:
  GENUI_ENV_FILE=<clone>/.env app/scripts/mcp_haiku_agent.py --stand-file <scratch>/workflows-stand.json
  app/scripts/mcp_haiku_agent.py --scope-demo --stand-file <scratch>/mcp-stand.json
"""
import argparse
import importlib.util
import ipaddress
import json
import os
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MESSAGES = "https://api.anthropic.com/v1/messages"
MODEL = "claude-haiku-4-5-20251001"
PROMPT = (
    "I'm Sarah Chen. What's waiting for my approval? Approve the Figma seat purchase: I confirm "
    "the approval now, so draft the decision and commit it. Answer in one short sentence."
)
MAX_TURNS = 8


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
    return read_env.read(env_file, ["ANTHROPIC_API_KEY"]).get("ANTHROPIC_API_KEY")


class NoRedirect(urllib.request.HTTPRedirectHandler):
    """Never follow a redirect: the request carries a credential (x-api-key or Authorization)."""

    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise urllib.error.HTTPError(req.full_url, code, "redirect refused: " + msg, headers, fp)


OPENER = urllib.request.build_opener(NoRedirect)


def post(url, body, headers, timeout=60):
    request = urllib.request.Request(url, data=json.dumps(body).encode(), headers=headers, method="POST")
    with OPENER.open(request, timeout=timeout) as response:
        return json.loads(response.read())


def is_loopback(host):
    host = (host or "").lower()
    if host == "localhost" or host.endswith(".localhost"):
        return True
    try:
        address = ipaddress.ip_address(host)
    except ValueError:
        return False
    mapped = getattr(address, "ipv4_mapped", None)
    return (mapped or address).is_loopback


def origin_allowed(origin):
    """https anywhere; plain http only to a loopback stand."""
    parsed = urllib.parse.urlparse(origin)
    return parsed.scheme == "https" or (parsed.scheme == "http" and is_loopback(parsed.hostname))


class Forma:
    def __init__(self, origin, token):
        self.url = origin.rstrip("/") + "/v1/mcp"
        self.headers = {"authorization": "Bearer " + token, "content-type": "application/json",
                        "accept": "application/json, text/event-stream"}
        self.next = 1

    def rpc(self, method, params):
        """The JSON-RPC result, or raise RpcError for an error answer (never a silent empty result)."""
        body = {"jsonrpc": "2.0", "id": self.next, "method": method, "params": params}
        self.next += 1
        reply = post(self.url, body, self.headers)
        if not isinstance(reply, dict) or "error" in reply or not isinstance(reply.get("result"), dict):
            error = reply.get("error") if isinstance(reply, dict) else None
            message = error.get("message") if isinstance(error, dict) else None
            raise RpcError(str(message or "no result")[:500])
        return reply["result"]


class RpcError(Exception):
    pass


TIME_OFF_DRAFT = {"app_hint": "Time off",
                  "values": {"leave_type": "vacation", "start": "2026-11-10", "end": "2026-11-12", "days": 3}}


def scope_demo(stand):
    """APP-115: a Security-scoped token tries the People space's Time off app (module notes)."""
    token = os.environ.get("FORMA_SCOPED_AGENT_TOKEN") or stand.get("security_token")
    origin = stand.get("origin") or os.environ.get("FORMA_ORIGIN")
    if not (token and origin):
        print(json.dumps({"skipped": "no scoped Forma token or origin"}))
        return 0
    if not origin_allowed(origin):
        print(json.dumps({"refused": "the Forma origin must be https (plain http only for a loopback stand)"}))
        return 2
    forma = Forma(origin, token)
    started = time.monotonic()
    try:
        listed = forma.rpc("tools/call", {"name": "forma_list_apps", "arguments": {}})
        titles = [a.get("title", "") for a in (listed.get("structuredContent") or {}).get("library", [])]
        t0 = time.monotonic()
        drafted = forma.rpc("tools/call", {"name": "forma_draft_record", "arguments": TIME_OFF_DRAFT})
        draft_ms = round((time.monotonic() - t0) * 1000)
    except RpcError as error:
        print(json.dumps({"passed": False, "error": str(error)}))
        return 1
    text = "\n".join(c.get("text", "") for c in drafted.get("content", []) if c.get("type") == "text")
    report = {"scope_demo": True, "listed_titles": titles, "time_off_listed": "Time off" in titles,
              "time_off_refused": bool(drafted.get("isError")) and text.strip() == "Not found.",
              "draft_ms": draft_ms, "total_ms": round((time.monotonic() - started) * 1000),
              "see": "Connect an agent -> Activity: refused: outside scope (People)"}
    report["passed"] = report["time_off_refused"] and not report["time_off_listed"]
    print(json.dumps(report, indent=2))
    return 0 if report["passed"] else 1


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--stand-file", default=os.environ.get("GENUI_WORKFLOWS_STAND_FILE"))
    parser.add_argument("--model", default=MODEL)
    parser.add_argument("--scope-demo", action="store_true",
                        help="APP-115: a Security-scoped token tries the Time off app (no model call)")
    args = parser.parse_args()
    if args.scope_demo:
        return scope_demo(json.loads(Path(args.stand_file).read_text()) if args.stand_file else {})
    try:
        key = read_key(Path(os.environ.get("GENUI_ENV_FILE", ROOT / ".env")))
    except read_env.EnvError as error:
        print(json.dumps({"refused": str(error)}))
        return 2
    if not key:
        print(json.dumps({"skipped": "no ANTHROPIC_API_KEY in the env file"}))
        return 0
    stand = json.loads(Path(args.stand_file).read_text()) if args.stand_file else {}
    token = os.environ.get("FORMA_AGENT_TOKEN") or stand.get("sarah_token")
    origin = stand.get("origin") or os.environ.get("FORMA_ORIGIN")
    if not (token and origin):
        print(json.dumps({"skipped": "no Forma token or origin"}))
        return 0
    if not origin_allowed(origin):
        print(json.dumps({"refused": "the Forma origin must be https (plain http only for a loopback stand)"}))
        return 2
    forma = Forma(origin, token)
    started = time.monotonic()
    try:
        listed = forma.rpc("tools/list", {})["tools"]
    except RpcError as error:
        print(json.dumps({"passed": False, "tools_list_error": str(error)}))
        return 1
    tools = [{"name": t["name"], "description": t.get("description", "")[:1000], "input_schema": t["inputSchema"]} for t in listed]
    headers = {"x-api-key": key, "anthropic-version": "2023-06-01", "content-type": "application/json"}
    messages = [{"role": "user", "content": PROMPT}]
    calls, turns, model_ms, answer = [], 0, [], ""
    while turns < MAX_TURNS:
        turns += 1
        t0 = time.monotonic()
        reply = post(MESSAGES, {"model": args.model, "max_tokens": 1024, "tools": tools, "messages": messages,
                                "system": "You act for the user through Forma's tools. Never invent ids: use the tools' results."}, headers)
        model_ms.append(round((time.monotonic() - t0) * 1000))
        messages.append({"role": "assistant", "content": reply["content"]})
        uses = [block for block in reply["content"] if block.get("type") == "tool_use"]
        if not uses:
            answer = " ".join(block.get("text", "") for block in reply["content"] if block.get("type") == "text")
            break
        results = []
        for use in uses:
            t1 = time.monotonic()
            try:
                result = forma.rpc("tools/call", {"name": use["name"], "arguments": use.get("input", {})})
                failed = bool(result.get("isError"))
                text = "\n".join(c.get("text", "") for c in result.get("content", []) if c.get("type") == "text")
            except RpcError as error:
                # A JSON-RPC error is a failed call, never a success with an empty result.
                failed, text = True, "error: " + str(error)
            calls.append({"tool": use["name"], "is_error": failed, "ms": round((time.monotonic() - t1) * 1000)})
            results.append({"type": "tool_result", "tool_use_id": use["id"], "content": text[:8000], "is_error": failed})
        messages.append({"role": "user", "content": results})
    report = {"model": args.model, "tools_listed": [t["name"] for t in tools], "calls": calls, "model_turns": turns,
              "model_ms": model_ms, "total_ms": round((time.monotonic() - started) * 1000),
              "committed": any(c["tool"] == "forma_commit" and not c["is_error"] for c in calls),
              "answer_chars": len(answer)}
    print(json.dumps(report, indent=2))
    return 0 if report["committed"] else 1


if __name__ == "__main__":
    sys.exit(main())
