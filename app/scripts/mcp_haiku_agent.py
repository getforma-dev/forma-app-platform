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
`GENUI_ENV_FILE`, else the repository root's) into this process's memory
only: never printed, logged, put on argv or written anywhere. Without it the
loop is skipped (exit 0). The Forma token comes from `FORMA_AGENT_TOKEN` or
the stand file's `sarah_token` (`GENUI_WORKFLOWS_STAND_FILE` of
`storage::workflows_campaign`) and is never printed either.

The report (stdout, JSON) holds decisions and latencies only: the tool names
the model called, whether each was an error, the model's turn count and the
wall times. No token, no key, no argument values.

Usage:
  GENUI_ENV_FILE=<clone>/.env app/scripts/mcp_haiku_agent.py --stand-file <scratch>/workflows-stand.json
"""
import argparse
import json
import os
import sys
import time
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


def read_key(env_file: Path):
    if not env_file.is_file():
        return None
    for line in env_file.read_text().splitlines():
        if line.startswith("ANTHROPIC_API_KEY="):
            value = line.split("=", 1)[1].strip().strip('"').strip("'")
            return value or None
    return None


def post(url, body, headers, timeout=60):
    request = urllib.request.Request(url, data=json.dumps(body).encode(), headers=headers, method="POST")
    with urllib.request.urlopen(request, timeout=timeout) as response:
        return json.loads(response.read())


class Forma:
    def __init__(self, origin, token):
        self.url = origin.rstrip("/") + "/v1/mcp"
        self.headers = {"authorization": "Bearer " + token, "content-type": "application/json",
                        "accept": "application/json, text/event-stream"}
        self.next = 1

    def rpc(self, method, params):
        body = {"jsonrpc": "2.0", "id": self.next, "method": method, "params": params}
        self.next += 1
        return post(self.url, body, self.headers)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--stand-file", default=os.environ.get("GENUI_WORKFLOWS_STAND_FILE"))
    parser.add_argument("--model", default=MODEL)
    args = parser.parse_args()
    key = read_key(Path(os.environ.get("GENUI_ENV_FILE", ROOT / ".env")))
    if not key:
        print(json.dumps({"skipped": "no ANTHROPIC_API_KEY in the env file"}))
        return 0
    stand = json.loads(Path(args.stand_file).read_text()) if args.stand_file else {}
    token = os.environ.get("FORMA_AGENT_TOKEN") or stand.get("sarah_token")
    origin = stand.get("origin") or os.environ.get("FORMA_ORIGIN")
    if not (token and origin):
        print(json.dumps({"skipped": "no Forma token or origin"}))
        return 0
    forma = Forma(origin, token)
    started = time.monotonic()
    listed = forma.rpc("tools/list", {})["result"]["tools"]
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
            result = forma.rpc("tools/call", {"name": use["name"], "arguments": use.get("input", {})}).get("result", {})
            calls.append({"tool": use["name"], "is_error": bool(result.get("isError")), "ms": round((time.monotonic() - t1) * 1000)})
            text = "\n".join(c.get("text", "") for c in result.get("content", []) if c.get("type") == "text")
            results.append({"type": "tool_result", "tool_use_id": use["id"], "content": text[:8000], "is_error": bool(result.get("isError"))})
        messages.append({"role": "user", "content": results})
    report = {"model": args.model, "tools_listed": [t["name"] for t in tools], "calls": calls, "model_turns": turns,
              "model_ms": model_ms, "total_ms": round((time.monotonic() - started) * 1000),
              "committed": any(c["tool"] == "forma_commit" and not c["is_error"] for c in calls),
              "answer_chars": len(answer)}
    print(json.dumps(report, indent=2))
    return 0 if report["committed"] else 1


if __name__ == "__main__":
    sys.exit(main())
