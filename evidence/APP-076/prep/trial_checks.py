#!/usr/bin/env python3
"""APP-076 go-live trial: scripted HTTP checks and a sanitized-evidence scan.

Preparation, not evidence: nothing here has run against beta.getforma.dev.
Standard library only (Python 3.11). Two subcommands:

  matrix  The private-route refusal matrix (ticket item 8, ADR-006 "Protected
          routes", ADR-007 D7.9/D7.15): anonymous, forged session and
          pre-access cookies, forged bearer and organization headers, known,
          guessed-foreign, nonexistent, malformed and undecodable private ids,
          query strings on work routes. Every row of the not_found class must
          answer the same status, the same body bytes and the same headers
          (Date excluded), with no Set-Cookie and no Location. Cross-site POSTs
          (Sec-Fetch-Site / Origin) form the cross_site class, uniform in the
          same way (ADR-006 refusal order: fetch metadata 403 before anything
          else). Forged callbacks form the callback class (the neutral sign-in
          failure; nonces normalized; no session cookie set). With real session
          cookies from the private input file it adds member controls (the
          same requests succeed: 1,140,000 and 900,000) and the canary replay
          (a member of the canary organization requesting Northstar resources
          gets the not_found bytes). --check forwarded-ip (opt-in) proves a
          forged X-Forwarded-For does not open a new sign-in address class.

  scan    Walks a directory of evidence and reports files holding a
          secret-shaped string (access code, opaque cookie value, fAuth
          invitation token, JWT, callback URL with a code, request_uri, AWS
          key, private key). It never prints the match. --self-test plants a
          synthetic token first and requires that it is found.

Secrets never travel in argv: session cookie values are read only from the
--private-input file (a regular file, owned by the caller, mode 0600). Request
bodies the matrix sends to real sessions are read-only (queries) or aimed at
identifiers the session cannot write; no sign-out or command is sent with a
real session except under a cross-site refusal.
"""
import argparse
import hashlib
import http.client
import ipaddress
import json
import os
import re
import secrets
import ssl
import stat
import sys
import time
import urllib.parse
from pathlib import Path

SCHEMA = "forma:app076-trial-checks:v1"

# Seeded identifiers (app/runtime/tenant_seed.py over contracts/fixtures/
# impl-2026-10-28.1; evidence/APP-009/handles.json). Non-secret.
NORTHSTAR_ORG = "00000000-0000-4000-8000-000000000001"
CANARY_ORG = "00000000-0000-4000-8000-000000000002"
NORTHSTAR_APP = "00000000-0000-4000-8000-000000000065"
NORTHSTAR_SEEDED_WORK = "00000000-0000-4000-8000-000000000069"
NORTHSTAR_COLLECTION = "00000000-0000-4000-8000-00000000006a"
SPEND_APP = "00000000-0000-4000-8000-0000000000c9"
SPEND_WORK = "00000000-0000-4000-8000-0000000000cd"
SPEND_COLLECTION = "00000000-0000-4000-8000-0000000000ce"
SPEND_RECORD = "00000000-0000-4000-8000-0000000013b1"
EXP_002 = "00000000-0000-4000-8000-0000000000f0"
WINDOW = {"start": "2026-10-22T10:00:00Z", "end": "2026-10-22T10:10:00Z"}
DESTINATION = "198.51.100.7"
TOTAL_WINDOW = "1140000"
TOTAL_DESTINATION = "900000"

NOT_FOUND_BODY = b'{"code":"not_found","status":404}'
CROSS_SITE_BODY = b'{"code":"cross_site","status":403}'
VOLATILE_HEADERS = {"date", "connection", "keep-alive"}
RECORDED_HEADERS = {
    "content-type", "content-length", "cache-control", "x-content-type-options",
    "referrer-policy", "retry-after", "strict-transport-security", "x-frame-options",
    "permissions-policy", "cross-origin-opener-policy", "cross-origin-resource-policy",
    "cross-origin-embedder-policy", "server", "vary", "allow",
}
SESSION_COOKIES = ("__Host-genui_session", "__Host-genui_pre", "__Host-genui_prelogin")
UUID = re.compile(r"[0-9a-f]{8}-[0-9a-f]{4}-[47][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}")
CAPABILITY = re.compile(r"[A-Za-z0-9_-]{43}")
HANDLE = re.compile(r"[a-z2-7]{26}")
LABEL = re.compile(r"[a-z0-9][a-z0-9-]{0,39}")
PRIVATE_KEYS = ("member_session", "canary_session", "pre_access")
LOOPBACK = {"localhost"}
BODY_LIMIT = 2 * 1024 * 1024


class Refused(Exception):
    """An input refusal; the message is fixed text, never an input value."""


# ---------------------------------------------------------------- inputs

def base_url(text):
    parsed = urllib.parse.urlsplit(text)
    if parsed.scheme not in ("https", "http") or not parsed.hostname:
        raise Refused("base_url_refused")
    if parsed.username or parsed.password or parsed.query or parsed.fragment or parsed.path not in ("", "/"):
        raise Refused("base_url_refused")
    if parsed.scheme == "http":
        host = parsed.hostname
        try:
            loopback = ipaddress.ip_address(host).is_loopback
        except ValueError:
            loopback = host in LOOPBACK
        if not loopback:
            raise Refused("plaintext_base_url_refused")
    return parsed.scheme, parsed.hostname, parsed.port


def identifier(text):
    if not isinstance(text, str) or not UUID.fullmatch(text):
        raise argparse.ArgumentTypeError("identifier refused")
    return text


def handle(text):
    if not HANDLE.fullmatch(text or ""):
        raise argparse.ArgumentTypeError("handle refused")
    return text


def label(text):
    if not LABEL.fullmatch(text or ""):
        raise argparse.ArgumentTypeError("label refused")
    return text


def private_input(path):
    """Cookie values from a private file: regular, the caller's, 0600 or tighter."""
    if path is None:
        return {}
    try:
        info = os.lstat(path)
    except OSError:
        raise Refused("private_input_unreadable") from None
    if not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid() or info.st_mode & 0o077:
        raise Refused("private_input_must_be_a_0600_regular_file")
    try:
        with open(path, "rb") as handle_:
            raw = handle_.read(8193)
        document = json.loads(raw) if len(raw) <= 8192 else None
    except (OSError, ValueError):
        raise Refused("private_input_malformed") from None
    if not isinstance(document, dict) or not set(document) <= set(PRIVATE_KEYS):
        raise Refused("private_input_malformed")
    for value in document.values():
        if not isinstance(value, str) or not CAPABILITY.fullmatch(value):
            raise Refused("private_input_malformed")
    return dict(document)


# ---------------------------------------------------------------- transport

class Client:
    def __init__(self, scheme, host, port, timeout=10.0, pace=0.05):
        self.scheme, self.host, self.port = scheme, host, port
        self.timeout, self.pace = timeout, pace
        self.context = ssl.create_default_context() if scheme == "https" else None

    def origin(self):
        default = 443 if self.scheme == "https" else 80
        port = "" if self.port in (None, default) else f":{self.port}"
        return f"{self.scheme}://{self.host}{port}"

    def send(self, method, path, headers, body):
        if self.scheme == "https":
            connection = http.client.HTTPSConnection(self.host, self.port, timeout=self.timeout, context=self.context)
        else:
            connection = http.client.HTTPConnection(self.host, self.port, timeout=self.timeout)
        try:
            connection.putrequest(method, path, skip_accept_encoding=True)
            for name, value in headers:
                connection.putheader(name, value)
            if body is not None:
                connection.putheader("Content-Length", str(len(body)))
            connection.endheaders(body)
            response = connection.getresponse()
            data = response.read(BODY_LIMIT + 1)
            return response.status, [(k.lower(), v) for k, v in response.getheaders()], data[:BODY_LIMIT]
        finally:
            connection.close()
            time.sleep(self.pace)


# ---------------------------------------------------------------- probes

def forged_capability():
    return secrets.token_urlsafe(32)


def forged_bearer():
    part = lambda n: secrets.token_urlsafe(n).rstrip("=")
    return "eyJ" + part(24) + "." + part(48) + "." + part(48)


def json_body(value):
    return json.dumps(value, separators=(",", ":"), sort_keys=True).encode()


def command_body(app, work, record, key):
    return json_body({
        "kind": "command_request", "schema_version": 1, "app_id": app, "work_id": work,
        "publication_id": "00000000-0000-4000-8000-0000000000cc", "command_id": "record_decision",
        "schema_id": "decision", "schema_version_expected": 1, "record_id": record,
        # A premise no saved work holds: were any probe ever authorized, storage
        # would answer 412 stale (nothing written) and the row would fail.
        "expected_record_version": 0, "expected_work_version": 999999, "idempotency_key": key,
        "values": [{"field_id": "status", "value": {"type": "enum", "value": "needs_context"}},
                   {"field_id": "note", "value": {"type": "text", "value": "trial probe; never committed"}}],
    })


def query_body(destination=None):
    parameters = {"window": dict(WINDOW)}
    if destination:
        parameters["destination"] = destination
    return json_body({"parameters": parameters})


def run_form(csrf=""):
    return urllib.parse.urlencode({"csrf": csrf, "window_start": "2026-10-22T10:00",
                                   "window_end": "2026-10-22T10:10", "destination": ""}).encode()


SAME_ORIGIN = [("Sec-Fetch-Site", "same-origin")]
JSON = [("Content-Type", "application/json")]
FORM = [("Content-Type", "application/x-www-form-urlencoded")]


def protected_targets(ids, ctx):
    """(row name, method, path, extra headers, body) aimed at private resources.

    ctx: 'outsider' (no real session), 'member' (a real Northstar member),
    'canary' (a real canary-organization member replaying Northstar ids).
    """
    ns, bound, seeded = ids["northstar_app"], ids["bound_work"], NORTHSTAR_SEEDED_WORK
    sr_app, sr_work, sr_coll, sr_rec = ids["spend_app"], ids["spend_work"], ids["spend_collection"], ids["spend_record"]
    nowhere_app, nowhere_work = "00000000-0000-4000-8000-00000000dead", "00000000-0000-4000-8000-00000000beef"
    key = secrets.token_hex(16)
    rows = []
    add = lambda *row: rows.append(row)
    # Known private ids (the bound Northstar work), foreign ids, nonexistent ids.
    if ctx != "member":
        add("work_page_known", "GET", f"/w/{ns}/{bound}", [], None)
        add("query_known_total", "POST", f"/v1/work/{ns}/{bound}/queries/total", SAME_ORIGIN + JSON, query_body())
        add("query_known_timeline", "POST", f"/v1/work/{ns}/{bound}/queries/timeline", SAME_ORIGIN + JSON, query_body())
        add("run_form_known", "POST", f"/w/{ns}/{bound}", SAME_ORIGIN + FORM, run_form())
        add("record_page_known", "GET", f"/work/{ns}/{bound}/records/{ids['northstar_collection']}/{EXP_002}", [], None)
        add("outcome_known", "GET", f"/v1/work/{ns}/{bound}/commands/record_decision/outcomes/{key}", [], None)
        # The demo's canary replay (design §4 act 6 item 8): Northstar's work id, EXP-002, a key.
        add("command_known", "POST", f"/v1/work/{ns}/{bound}/commands/record_decision", SAME_ORIGIN + JSON + [("Idempotency-Key", key)], command_body(ns, bound, EXP_002, key))
    if ctx == "outsider":
        add("workspace", "GET", "/workspace", [], None)
        add("pending", "GET", "/auth/pending", [], None)
        add("work_page_seeded", "GET", f"/w/{ns}/{seeded}", [], None)
        add("signout_all", "POST", "/auth/signout-all", SAME_ORIGIN + FORM, b"csrf=")
        add("resend", "POST", "/auth/pending/resend", SAME_ORIGIN + FORM, b"csrf=")
        add("unknown_v1", "GET", "/v1/unknown", [], None)
        add("unknown_path", "GET", "/does-not-exist", [], None)
        add("admin", "GET", "/admin", [], None)
        add("qualification_harness", "POST", "/auth/qualification/start", SAME_ORIGIN + FORM, b"")
        # Method mismatches on existing routes must not differ from an unknown path.
        add("invite_validate_get", "GET", "/invite/validate", [], None)
        add("query_route_get", "GET", f"/v1/work/{ns}/{bound}/queries/total", [], None)
        add("command_route_get", "GET", f"/v1/work/{ns}/{bound}/commands/record_decision", [], None)
        add("signout_all_get", "GET", "/auth/signout-all", [], None)
        add("workspace_post", "POST", "/workspace", SAME_ORIGIN + FORM, b"")
        add("query_known_with_query_string", "POST", f"/v1/work/{ns}/{bound}/queries/total?destination={DESTINATION}", SAME_ORIGIN + JSON, query_body())
        add("command_undecodable_segment", "POST", f"/v1/work/%FF/{bound}/commands/record_decision", SAME_ORIGIN + JSON, b"{}")
        add("record_page_undecodable_segment", "GET", f"/work/%FF/{bound}/records/{ids['northstar_collection']}/{EXP_002}", [], None)
    if ctx != "canary":
        # Guessed foreign ids: the Spend Review work of the other fixture organization.
        add("work_page_foreign", "GET", f"/w/{sr_app}/{sr_work}", [], None)
        add("query_foreign", "POST", f"/v1/work/{sr_app}/{sr_work}/queries/expenses", SAME_ORIGIN + JSON, query_body())
        add("record_page_foreign", "GET", f"/work/{sr_app}/{sr_work}/records/{sr_coll}/{sr_rec}", [], None)
        add("command_foreign", "POST", f"/v1/work/{sr_app}/{sr_work}/commands/record_decision", SAME_ORIGIN + JSON + [("Idempotency-Key", key)], command_body(sr_app, sr_work, sr_rec, key))
    add("work_page_nonexistent", "GET", f"/w/{nowhere_app}/{nowhere_work}", [], None)
    add("query_nonexistent", "POST", f"/v1/work/{nowhere_app}/{nowhere_work}/queries/total", SAME_ORIGIN + JSON, query_body())
    add("record_page_nonexistent", "GET", f"/work/{nowhere_app}/{nowhere_work}/records/{sr_coll}/{sr_rec}", [], None)
    add("command_nonexistent", "POST", f"/v1/work/{nowhere_app}/{nowhere_work}/commands/record_decision", SAME_ORIGIN + JSON + [("Idempotency-Key", key)], command_body(nowhere_app, nowhere_work, sr_rec, key))
    add("work_page_malformed_id", "GET", f"/w/not-an-id/{bound}", [], None)
    add("work_page_undecodable_segment", "GET", f"/w/%FF/{bound}", [], None)
    # Filters travel in bodies only (design §11.3): a query string on a work page is the uniform 404.
    add("work_page_query_string", "GET", f"/w/{ns}/{bound}?destination={DESTINATION}", [], None)
    add("work_page_query_string_window", "GET", f"/w/{ns}/{bound}?window_start=2026-10-22T10:00", [], None)
    if ctx == "member":
        add("outcome_unused_key", "GET", f"/v1/work/{ns}/{bound}/commands/record_decision/outcomes/{key}", [], None)
    return rows


def contexts(secrets_, ids):
    """(context name, cookie header or None, extra request headers, target set)."""
    forged_session = forged_capability()
    result = [
        ("anonymous", None, [], "outsider"),
        ("forged_session_cookie", f"__Host-genui_session={forged_session}", [], "outsider"),
        ("forged_pre_access_cookie", f"__Host-genui_pre={forged_capability()}", [], "outsider"),
        ("forged_bearer", None, [("Authorization", "Bearer " + forged_bearer())], "outsider"),
        ("forged_organization_headers", f"__Host-genui_session={forged_session}",
         [("X-Organization-Id", NORTHSTAR_ORG), ("X-Forma-Organization", NORTHSTAR_ORG),
          ("X-Tenant-Id", "ten_" + NORTHSTAR_ORG), ("X-Forwarded-Host", "beta.getforma.dev")], "outsider"),
        ("forged_both_cookies", f"__Host-genui_session={forged_capability()}; __Host-genui_pre={forged_capability()}", [], "outsider"),
    ]
    if "pre_access" in secrets_:
        result.append(("real_pre_access", f"__Host-genui_pre={secrets_['pre_access']}", [], "outsider-pre"))
    if "member_session" in secrets_:
        result.append(("real_member", f"__Host-genui_session={secrets_['member_session']}", [], "member"))
    if "canary_session" in secrets_:
        result.append(("canary_replay", f"__Host-genui_session={secrets_['canary_session']}", [], "canary"))
    return result


def cross_site_targets(ids, unissued):
    ns, bound = ids["northstar_app"], ids["bound_work"]
    key = secrets.token_hex(16)
    return [
        ("query", f"/v1/work/{ns}/{bound}/queries/total", JSON, query_body()),
        ("run_form", f"/w/{ns}/{bound}", FORM, run_form()),
        ("command", f"/v1/work/{ns}/{bound}/commands/record_decision", JSON + [("Idempotency-Key", key)], command_body(ns, bound, EXP_002, key)),
        ("signout", "/auth/signout", FORM, b"csrf="),
        ("signout_all", "/auth/signout-all", FORM, b"csrf="),
        ("resend", "/auth/pending/resend", FORM, b"csrf="),
        ("signin_start", f"/signin/{unissued}", FORM, b""),
        ("invite_validate", "/invite/validate", FORM, b"code=AAAA"),
    ]


FETCH_METADATA = [
    ("cross_site", [("Sec-Fetch-Site", "cross-site"), ("Origin", "https://attacker.example")]),
    ("same_site", [("Sec-Fetch-Site", "same-site")]),
    ("foreign_origin_only", [("Origin", "https://attacker.example")]),
    ("no_fetch_metadata", []),
    ("duplicate_fetch_metadata", [("Sec-Fetch-Site", "same-origin"), ("Sec-Fetch-Site", "same-origin")]),
]


# ---------------------------------------------------------------- recording

def header_map(headers):
    merged = {}
    for name, value in headers:
        merged.setdefault(name, []).append(value)
    return merged


def fingerprint(headers):
    """The response's headers, Date and Set-Cookie excluded, CSP nonces normalized."""
    pairs = sorted((n, NONCE_TEXT.sub("nonce-N", v)) for n, v in headers if n not in VOLATILE_HEADERS and n != "set-cookie")
    return hashlib.sha256(json.dumps(pairs).encode()).hexdigest()


def cookie_effects(headers):
    """Names of cookies the response sets, and whether any sets a value (clearing is allowed)."""
    names, sets_value = [], False
    for name, value in headers:
        if name != "set-cookie":
            continue
        pair = value.split(";", 1)[0]
        cookie, _, content = pair.partition("=")
        names.append(cookie.strip())
        if content.strip() and "max-age=0" not in value.lower():
            sets_value = True
    return sorted(set(names)), sets_value


NONCE = re.compile(rb"nonce-[A-Za-z0-9+/=_-]+|nonce=\"[A-Za-z0-9+/=_-]+\"")
NONCE_TEXT = re.compile(r"nonce-[A-Za-z0-9+/=_-]+")


def normalized(body):
    return NONCE.sub(b"nonce-N", body)


def location_path(headers):
    for name, value in headers:
        if name == "location":
            parsed = urllib.parse.urlsplit(value)
            return {"origin": f"{parsed.scheme}://{parsed.netloc}" if parsed.netloc else "", "path": parsed.path,
                    "has_query": bool(parsed.query)}
    return None


def record(row_id, cls, context, method, path, request_headers, status, headers, body, expected, note=None):
    names, sets_value = cookie_effects(headers)
    merged = header_map(headers)
    entry = {
        "id": row_id, "class": cls, "context": context, "method": method, "path": path,
        "request_header_names": sorted({n.lower() for n, _ in request_headers if n.lower() != "cookie"}
                                       | ({"cookie"} if any(n.lower() == "cookie" for n, _ in request_headers) else set())),
        "status": status,
        "body_sha256": hashlib.sha256(body).hexdigest(), "body_length": len(body),
        "normalized_body_sha256": hashlib.sha256(normalized(body)).hexdigest(),
        "headers": {n: v if len(v) > 1 else v[0] for n, v in sorted(merged.items()) if n in RECORDED_HEADERS},
        "header_names": sorted(merged),
        "header_fingerprint": fingerprint(headers),
        "set_cookie_names": names, "sets_cookie_value": sets_value,
        "location": location_path(headers),
        "expected": expected,
    }
    if note:
        entry["note"] = note
    return entry


# ---------------------------------------------------------------- checks

def expect_not_found(entry):
    return (entry["status"] == 404 and entry["body_sha256"] == hashlib.sha256(NOT_FOUND_BODY).hexdigest()
            and entry["headers"].get("content-type") == "application/problem+json"
            and entry["headers"].get("cache-control") == "no-store"
            and entry["headers"].get("x-content-type-options") == "nosniff"
            and not entry["set_cookie_names"] and entry["location"] is None
            and "allow" not in entry["header_names"])


def expect_cross_site(entry):
    return (entry["status"] == 403 and entry["body_sha256"] == hashlib.sha256(CROSS_SITE_BODY).hexdigest()
            and entry["headers"].get("content-type") == "application/problem+json"
            and not entry["set_cookie_names"] and entry["location"] is None)


def expect_neutral_signin(entry):
    return (entry["status"] == 400 and str(entry["headers"].get("content-type", "")).startswith("text/html")
            and not entry["sets_cookie_value"] and entry["location"] is None
            and "no-store" in str(entry["headers"].get("cache-control", "")))


def matrix(client, ids, secrets_, checks, canary_revoked, unissued):
    rows, controls = [], []

    def send(row_id, cls, context, method, path, headers, body, expected, note=None):
        status, response_headers, data = client.send(method, path, headers, body)
        entry = record(row_id, cls, context, method, path, headers, status, response_headers, data, expected, note)
        return entry, data

    for context, cookie, extra, target in contexts(secrets_, ids):
        if ("canary" if target == "canary" else "matrix") not in checks:
            continue
        base = ([("Cookie", cookie)] if cookie else []) + extra
        cls = "not_found"
        set_ = "outsider" if target == "outsider-pre" else target
        for name, method, path, headers, body in protected_targets(ids, set_):
            if target == "outsider-pre" and name in ("pending", "resend"):
                continue  # a live pre-access session legitimately reads its own pending page
            # ADR-006 refusal order: a sign-out-all or resend that carries its
            # cookie is refused on the synchronizer token (403 cross_site)
            # before any lookup, whether the cookie is real or forged.
            token_bound = (name == "signout_all" and "__Host-genui_session=" in (cookie or "")) or \
                          (name == "resend" and "__Host-genui_pre=" in (cookie or ""))
            if token_bound:
                entry, _ = send(f"{context}.{name}", "cross_site", context, method, path, base + headers, body, "cross_site",
                                "synchronizer-token refusal before any lookup; independent of the cookie's validity")
                entry["passed"] = expect_cross_site(entry)
            else:
                entry, _ = send(f"{context}.{name}", cls, context, method, path, base + headers, body, "not_found")
                entry["passed"] = expect_not_found(entry)
            rows.append(entry)
        if target == "canary" and canary_revoked:
            entry, _ = send(f"{context}.workspace_after_revocation", "not_found", context, "GET", "/workspace", base, None, "not_found")
            entry["passed"] = expect_not_found(entry)
            rows.append(entry)

    if "member_session" in secrets_ and "controls" in checks:
        controls.extend(member_controls(send, ids, secrets_["member_session"]))
    if "canary_session" in secrets_ and "canary" in checks and not canary_revoked:
        cookie = [("Cookie", f"__Host-genui_session={secrets_['canary_session']}")]
        entry, _ = send("canary_replay.control_workspace", "control", "canary_replay", "GET", "/workspace", cookie, None, "200")
        entry["passed"] = entry["status"] == 200 and not entry["sets_cookie_value"]
        controls.append(entry)

    if "matrix" in checks:
        sessions = [("forged_session_cookie", [("Cookie", f"__Host-genui_session={forged_capability()}")])]
        if "member_session" in secrets_:
            sessions.append(("real_member", [("Cookie", f"__Host-genui_session={secrets_['member_session']}")]))
        if "canary_session" in secrets_ and "canary" in checks:
            sessions.append(("canary_replay", [("Cookie", f"__Host-genui_session={secrets_['canary_session']}")]))
        for context, cookie in [("anonymous", [])] + sessions:
            real = context in ("real_member", "canary_replay")
            for name, path, headers, body in cross_site_targets(ids, unissued):
                if real and name not in ("query", "run_form", "command"):
                    continue  # never aim a sign-out or resend at a real session, even cross-site
                for meta, meta_headers in FETCH_METADATA:
                    if meta == "duplicate_fetch_metadata" and name != "query":
                        continue
                    entry, _ = send(f"{context}.{name}.{meta}", "cross_site", context, "POST", path, cookie + meta_headers + headers, body, "cross_site")
                    entry["passed"] = expect_cross_site(entry)
                    rows.append(entry)
        state, code = forged_capability(), forged_capability()
        for name, headers, query in [
            ("forged_code_no_cookie", [], f"code={code}&state={state}"),
            ("forged_code_forged_prelogin", [("Cookie", f"__Host-genui_prelogin={forged_capability()}")], f"code={code}&state={state}"),
            ("repeated_code", [("Cookie", f"__Host-genui_prelogin={forged_capability()}")], f"code={code}&code={code}&state={state}"),
            ("extra_parameter", [], f"code={code}&state={state}&organization={NORTHSTAR_ORG}"),
            ("no_parameters", [], ""),
        ]:
            path = "/auth/callback" + ("?" + query if query else "")
            entry, _ = send(f"callback.{name}", "callback", "anonymous", "GET", path, headers, None, "neutral_signin",
                            "forged code/state; a replay of a real, consumed code is never handled by this script")
            entry["path"] = "/auth/callback?" + ("<forged code/state>" if query else "")
            entry["passed"] = expect_neutral_signin(entry)
            rows.append(entry)

    if "forwarded-ip" in checks:
        rows.extend(forwarded_ip(send, unissued))
    return rows, controls


def member_controls(send, ids, session):
    ns, bound = ids["northstar_app"], ids["bound_work"]
    cookie = [("Cookie", f"__Host-genui_session={session}")]
    out = []
    entry, _ = send("real_member.control_workspace", "control", "real_member", "GET", "/workspace", cookie, None, "200")
    entry["passed"] = entry["status"] == 200
    out.append(entry)
    entry, _ = send("real_member.control_work_page", "control", "real_member", "GET", f"/w/{ns}/{bound}", cookie, None, "200")
    entry["passed"] = entry["status"] == 200
    out.append(entry)
    for name, view, destination, total in [("total_window", "total", None, TOTAL_WINDOW),
                                           ("total_destination", "total", DESTINATION, TOTAL_DESTINATION),
                                           ("timeline_window", "timeline", None, None)]:
        entry, data = send(f"real_member.control_{name}", "control", "real_member", "POST",
                           f"/v1/work/{ns}/{bound}/queries/{view}", cookie + SAME_ORIGIN + JSON, query_body(destination), "200")
        ok = entry["status"] == 200
        try:
            document = json.loads(data)
        except ValueError:
            document, ok = {}, False
        receipt = document.get("receipt") if isinstance(document, dict) else None
        entry["result"] = {
            "view_id": document.get("view_id") if isinstance(document, dict) else None,
            "columns": document.get("columns") if isinstance(document, dict) else None,
            "row_count": len(document.get("rows") or []) if isinstance(document, dict) else None,
            "receipt_digest": receipt.get("digest") if isinstance(receipt, dict) else None,
            "result_digest": receipt.get("result_digest") if isinstance(receipt, dict) else None,
        }
        if total is not None:
            value = (document.get("rows") or [[None]])[0][0] if isinstance(document, dict) and document.get("rows") else None
            entry["result"]["total"] = value
            ok = ok and value == total
        entry["passed"] = ok and isinstance(receipt, dict)
        out.append(entry)
    entry, _ = send("real_member.control_query_string", "control", "real_member", "POST",
                    f"/v1/work/{ns}/{bound}/queries/total?destination={DESTINATION}", cookie + SAME_ORIGIN + JSON, query_body(), "422 invalid_parameters")
    entry["passed"] = entry["status"] == 422 and entry["body_sha256"] == hashlib.sha256(b'{"code":"invalid_parameters","status":422}').hexdigest()
    out.append(entry)
    return out


def forwarded_ip(send, unissued):
    """ADR-007 D7.14: only the rightmost X-Forwarded-For hop counts. Eleven
    sign-in starts, each with a different forged leftmost hop, must meet the
    10-per-minute address-class limit (429) by the eleventh."""
    rows = []
    limited = False
    for index in range(11):
        forged = f"203.0.113.{index + 10}"
        entry, _ = send(f"forwarded_ip.attempt_{index + 1}", "forwarded_ip", "anonymous", "POST", f"/signin/{unissued}",
                        SAME_ORIGIN + FORM + [("X-Forwarded-For", forged)], b"", "400 then 429")
        rows.append(entry)
        if entry["status"] == 429:
            limited = True
            break
    for entry in rows:
        entry["passed"] = limited and entry["status"] in (400, 429)
    return rows


def uniformity(rows):
    classes = {}
    for entry in rows:
        if entry["class"] not in ("not_found", "cross_site", "callback"):
            continue
        key = "normalized_body_sha256" if entry["class"] == "callback" else "body_sha256"
        group = classes.setdefault(entry["class"], {"rows": 0, "statuses": set(), "bodies": set(), "headers": set()})
        group["rows"] += 1
        group["statuses"].add(entry["status"])
        group["bodies"].add(entry[key])
        group["headers"].add(entry["header_fingerprint"])
    summary = {}
    for name, group in classes.items():
        summary[name] = {
            "rows": group["rows"],
            "statuses": sorted(group["statuses"]),
            "distinct_bodies": len(group["bodies"]),
            "distinct_header_sets": len(group["headers"]),
            "uniform": len(group["statuses"]) == 1 and len(group["bodies"]) == 1 and len(group["headers"]) == 1,
        }
    return summary


def own_digest():
    return hashlib.sha256(Path(__file__).read_bytes()).hexdigest()


def run_matrix(args):
    scheme, host, port = base_url(args.base_url)
    secrets_ = private_input(args.private_input)
    checks = set(args.check.split(","))
    if not checks <= {"matrix", "canary", "controls", "forwarded-ip"}:
        raise Refused("check_refused")
    ids = {"northstar_app": args.northstar_app, "bound_work": args.bound_work,
           "northstar_collection": args.northstar_collection, "spend_app": args.spend_app,
           "spend_work": args.spend_work, "spend_collection": args.spend_collection, "spend_record": args.spend_record}
    client = Client(scheme, host, port, timeout=args.timeout, pace=args.pace)
    started = time.time()
    rows, controls = matrix(client, ids, secrets_, checks, args.canary_revoked, args.unissued_handle)
    classes = uniformity(rows)
    not_run = []
    if "member_session" not in secrets_:
        not_run.append("real_member rows and member controls: no member_session in the private input")
    if "canary_session" not in secrets_:
        not_run.append("canary replay: no canary_session in the private input")
    if "pre_access" not in secrets_:
        not_run.append("real pre-access rows: no pre_access in the private input (forged pre-access rows ran)")
    if "forwarded-ip" not in checks:
        not_run.append("forwarded-ip: opt-in, not requested")
    failures = [entry["id"] for entry in rows + controls if not entry.get("passed")]
    failures += [f"class:{name}" for name, item in classes.items() if not item["uniform"]]
    report = {
        "schema": SCHEMA,
        "scope": "APP-076 scripted HTTP checks; sanitized (no cookie, token, code or full callback URL)",
        "label": args.label,
        "target": client.origin(),
        "started_at_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(started)),
        "duration_seconds": round(time.time() - started, 3),
        "script_sha256": own_digest(),
        "python": sys.version.split()[0],
        "checks": sorted(checks),
        "inputs": {"private_input_keys": sorted(secrets_), "canary_revoked": args.canary_revoked, "identifiers": ids,
                   "unissued_handle": args.unissued_handle},
        "expected_bodies_sha256": {"not_found": hashlib.sha256(NOT_FOUND_BODY).hexdigest(),
                                   "cross_site": hashlib.sha256(CROSS_SITE_BODY).hexdigest()},
        "classes": classes,
        "rows": rows,
        "controls": controls,
        "not_run": not_run,
        "failures": failures,
        "passed": not failures,
    }
    text = json.dumps(report, indent=2, sort_keys=True) + "\n"
    for value in secrets_.values():
        if value in text:
            raise Refused("report_would_disclose_a_secret")
    if args.output:
        Path(args.output).write_text(text)
    summary = {"passed": report["passed"], "rows": len(rows), "controls": len(controls), "classes": classes,
               "failures": failures, "not_run": not_run}
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0 if report["passed"] else 1


# ---------------------------------------------------------------- scan

SCAN_PATTERNS = {
    "access_code_base32": re.compile(r"(?<![A-Z2-7])[A-Z2-7]{52}(?![A-Z2-7])"),
    "opaque_capability": re.compile(r"(?<![A-Za-z0-9_-])(?=[A-Za-z0-9_-]*[0-9])(?=[A-Za-z0-9_-]*[A-Z])(?=[A-Za-z0-9_-]*[a-z])[A-Za-z0-9_-]{43}(?![A-Za-z0-9_-])"),
    "fauth_invitation_token": re.compile(r"tinv_[0-9a-f]{8}-[0-9a-f]{4}-"),
    "jwt": re.compile(r"eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}"),
    "callback_with_code": re.compile(r"/auth/callback\?[^\s\"'<>]*code=[A-Za-z0-9_-]{8,}"),
    # Character classes keep this source line from matching itself.
    "request_uri": re.compile(r"request_uri=ur[n]:|ur[n]:ietf:params:oauth:request_uri:[A-Za-z0-9_-]"),
    "session_cookie": re.compile(r"__Host-genui_(?:session|pre|prelogin)=[A-Za-z0-9_-]{20,}"),
    "aws_access_key": re.compile(r"(?<![A-Z0-9])(?:AKIA|ASIA)[0-9A-Z]{16}(?![A-Z0-9])"),
    "private_key": re.compile(r"-----BEGIN [A-Z ]*PRIVATE KEY-----"),
}
SCAN_FILE_LIMIT = 8 * 1024 * 1024


def scan_tree(root):
    findings, files = [], 0
    for path in sorted(Path(root).rglob("*")):
        if path.is_symlink() or not path.is_file() or path.stat().st_size > SCAN_FILE_LIMIT:
            continue
        files += 1
        text = path.read_bytes().decode("utf-8", "replace")
        for number, line in enumerate(text.splitlines(), 1):
            for name, pattern in SCAN_PATTERNS.items():
                if pattern.search(line):
                    findings.append({"file": str(path.relative_to(root)), "line": number, "pattern": name})
    return files, findings


def run_scan(args):
    report = {"schema": SCHEMA + ":scan", "script_sha256": own_digest(), "root": args.root}
    if args.self_test:
        import tempfile
        with tempfile.TemporaryDirectory(prefix="app076-scan-") as directory:
            planted = {
                "access_code_base32": "".join(secrets.choice("ABCDEFGHIJKLMNOPQRSTUVWXYZ234567") for _ in range(52)),
                "fauth_invitation_token": "tinv_" + "0" * 8 + "-0000-4000-8000-" + "0" * 12,
                "session_cookie": "__Host-genui_session=" + secrets.token_urlsafe(32),
                "callback_with_code": "https://beta.getforma.dev/auth/callback?code=" + secrets.token_urlsafe(16) + "&state=x",
                "request_uri": "request_uri=" + "urn" + ":ietf:params:oauth:request_uri:" + secrets.token_urlsafe(16),
            }
            for name, value in planted.items():
                Path(directory, name + ".txt").write_text("synthetic planted value: " + value + "\n")
            _, found = scan_tree(directory)
            detected = sorted({item["pattern"] for item in found})
            report["self_test"] = {"planted": sorted(planted), "detected": detected,
                                   "passed": set(planted) <= set(detected)}
            if not report["self_test"]["passed"]:
                print(json.dumps(report, indent=2, sort_keys=True))
                return 1
    files, findings = scan_tree(args.root)
    report.update({"files_scanned": files, "findings": findings, "passed": not findings})
    text = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if args.output:
        Path(args.output).write_text(text)
    print(text, end="")
    return 0 if not findings else 1


# ---------------------------------------------------------------- CLI

class Parser(argparse.ArgumentParser):
    def error(self, message):
        print(json.dumps({"refused": "usage", "detail": message.split(":")[0]}), file=sys.stderr)
        sys.exit(2)


def parser():
    p = Parser(description="APP-076 scripted HTTP checks (preparation).")
    sub = p.add_subparsers(dest="command", required=True)
    m = sub.add_parser("matrix", help="refusal matrix, cross-site and callback classes, controls, canary replay")
    m.add_argument("--base-url", required=True, help="https://beta.getforma.dev, or http on a loopback host only")
    m.add_argument("--private-input", help="0600 JSON file with optional member_session, canary_session, pre_access")
    m.add_argument("--bound-work", required=True, type=identifier, help="the bound Northstar work id (create-bound-work)")
    m.add_argument("--northstar-app", default=NORTHSTAR_APP, type=identifier)
    m.add_argument("--northstar-collection", default=NORTHSTAR_COLLECTION, type=identifier)
    m.add_argument("--spend-app", default=SPEND_APP, type=identifier)
    m.add_argument("--spend-work", default=SPEND_WORK, type=identifier)
    m.add_argument("--spend-collection", default=SPEND_COLLECTION, type=identifier)
    m.add_argument("--spend-record", default=SPEND_RECORD, type=identifier)
    m.add_argument("--unissued-handle", default="aaaaaaaaaaaaaaaaaaaaaaaaaa", type=handle)
    m.add_argument("--check", default="matrix,canary,controls", help="comma list of matrix, canary, controls, forwarded-ip")
    m.add_argument("--canary-revoked", action="store_true", help="the canary member's entitlement was revoked: its control joins not_found")
    m.add_argument("--label", default="trial", type=label)
    m.add_argument("--timeout", default=10.0, type=float)
    m.add_argument("--pace", default=0.05, type=float, help="seconds between requests")
    m.add_argument("--output")
    s = sub.add_parser("scan", help="report files holding secret-shaped strings (never the match)")
    s.add_argument("--root", required=True)
    s.add_argument("--self-test", action="store_true")
    s.add_argument("--output")
    return p


def main(argv=None):
    args = parser().parse_args(argv)
    try:
        return run_matrix(args) if args.command == "matrix" else run_scan(args)
    except Refused as refusal:
        print(json.dumps({"refused": str(refusal)}), file=sys.stderr)
        return 2
    except (OSError, http.client.HTTPException, ssl.SSLError) as error:
        print(json.dumps({"refused": "transport", "kind": type(error).__name__}), file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
