#!/usr/bin/env python3
"""Idempotent synthetic demo provisioning. Deliver this directory to GenUI.

Public config comes from --config; credentials come from environment variables
and a managed secret store. Nothing secret is printed. Local secret files are
permitted only by explicit --synthetic-secret-file with loopback auth URLs.
"""
import argparse
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import urllib.error
import urllib.parse
import urllib.request


class Failure(Exception):
    pass


class Secrets:
    def __init__(self, prefix, local_file=None, region="us-east-1"):
        self.prefix, self.local_file, self.region = prefix, local_file, region
        if local_file and local_file.exists():
            if stat.S_IMODE(local_file.stat().st_mode) & 0o077:
                raise Failure("Synthetic secret file must have mode 0600")
            self.data = json.loads(local_file.read_text())
        else:
            self.data = {}

    def aws(self, args, stdin=None):
        # Capture outputs; AWS error text and secret values never enter the receipt.
        p = subprocess.run(["aws", "secretsmanager", *args, "--region", self.region, "--output", "json"],
                           input=stdin, text=True, capture_output=True)
        if p.returncode:
            if "ResourceNotFoundException" in p.stderr:
                return None
            raise Failure("Managed secret operation failed; inspect permissions/configuration securely")
        return json.loads(p.stdout)

    def get(self, slug):
        if self.local_file:
            return self.data.get(slug)
        value = self.aws(["get-secret-value", "--secret-id", f"{self.prefix}/{slug}"])
        return json.loads(value["SecretString"]) if value else None

    def put(self, slug, value):
        if self.local_file:
            self.data[slug] = value
            # Exclusive creation prevents accidental overwrites of another operator's file.
            if not self.local_file.exists():
                fd = os.open(self.local_file, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
                os.close(fd)
            temporary = self.local_file.with_name(self.local_file.name + ".pending")
            fd = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
            with os.fdopen(fd, "w") as output:
                output.write(json.dumps(self.data))
                output.flush()
                os.fsync(output.fileno())
            os.replace(temporary, self.local_file)
            return
        serialized = json.dumps(value)
        result = self.aws(["create-secret", "--name", f"{self.prefix}/{slug}",
                           "--secret-string", "file:///dev/stdin"], serialized)
        if result is None:
            raise Failure("Unable to persist tenant management key")


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        # A management endpoint must never forward a bearer key to a Location.
        return None


OPENER = urllib.request.build_opener(NoRedirect())


def request(base, method, path, key, body=None):
    req = urllib.request.Request(base + path, method=method,
        data=json.dumps(body).encode() if body is not None else None,
        headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json"})
    try:
        with OPENER.open(req, timeout=30) as response:
            content = response.read()
            return response.status, json.loads(content) if content else {}
    except urllib.error.HTTPError as e:
        try:
            error = json.load(e).get("error", "unknown")
        except Exception:
            error = "unknown"
        raise Failure(f"{method} {path.split('?')[0]} failed: HTTP {e.code} ({error})") from None


def provision(config, secrets):
    base = config["auth_origin"].rstrip("/")
    platform_key = os.environ.get("FAUTH_PLATFORM_KEY")
    if not platform_key:
        raise Failure("Set FAUTH_PLATFORM_KEY through the approved secret environment")
    redirect = config["redirect_uri"]
    origin = config["app_origin"].rstrip("/")
    parsed_redirect = urllib.parse.urlsplit(redirect)
    if f"{parsed_redirect.scheme}://{parsed_redirect.netloc}" != origin:
        raise Failure("redirect_uri must belong to the explicitly selected app_origin")
    # Fetch every existing tenant page. Do not infer absence from a truncated list.
    tenants = []
    offset = 0
    while True:
        _, page = request(base, "GET", f"/platform/tenants?limit=100&offset={offset}", platform_key)
        batch = page["tenants"]
        tenants.extend(batch)
        if len(batch) < 100:
            break
        offset += len(batch)
    receipt = {"auth_origin": base, "app_origin": origin, "redirect_uri": redirect, "tenants": []}
    for desired in config["tenants"]:
        slug = desired["slug"]
        existing = next((t for t in tenants if t["slug"] == slug), None)
        secret = secrets.get(slug)
        if existing and not secret:
            raise Failure(f"Tenant {slug} exists but its management secret is absent; recover it explicitly")
        if not existing:
            if secret:
                raise Failure(f"Secret exists for absent tenant {slug}; reconcile environment explicitly")
            _, created = request(base, "POST", "/platform/tenants", platform_key, {
                "name": desired["name"], "slug": slug, "owner_email": desired["owner_email"],
                "allowed_providers": ["email"], "signup_mode": "invite_only"})
            existing = created["tenant"]
            secret = {"tenant_id": existing["id"], "management_key": created["api_key"]}
            secrets.put(slug, secret)
        if secret["tenant_id"] != existing["id"]:
            raise Failure(f"Management secret identity differs for {slug}")
        key = secret["management_key"]
        request(base, "PATCH", "/manage/config", key, {
            "branding_app_name": desired["name"], "branding_primary_color": desired["primary_color"],
            "branding_logo_url": desired["logo_url"], "login_redirect_url": redirect,
            "signup_mode": "invite_only", "allowed_providers": ["email"], "allowed_origins": [origin]})
        request(base, "PATCH", f"/manage/tenants/{existing['id']}/security", key, {"mfa_policy": "off"})
        client = {"client_id": desired["client_id"], "name": desired["name"] + " platform", "redirect_uri": redirect}
        entry = desired.get("return_entry_url")
        if entry is not None:
            # GenUI's per-organization fresh sign-in route (requires a server with
            # return-entry support); same origin, no query or fragment.
            parsed_entry = urllib.parse.urlsplit(entry)
            if f"{parsed_entry.scheme}://{parsed_entry.netloc}" != origin or parsed_entry.query or parsed_entry.fragment:
                raise Failure(f"return_entry_url for {slug} must be on app_origin without query or fragment")
            client["return_entry_url"] = entry
        request(base, "POST", "/manage/oauth-clients", key, client)
        user_receipts = []
        for user in desired["users"]:
            password = os.environ.get(user["password_env"])
            if not password:
                raise Failure(f"Missing synthetic password environment variable {user['password_env']}")
            offset, found = 0, None
            while True:
                _, matches = request(base, "GET", f"/manage/users?limit=100&offset={offset}&email=" + urllib.parse.quote(user["email"], safe=""), key)
                found = next((u for u in matches["users"] if u["email"].lower() == user["email"].lower()), None)
                if found or len(matches["users"]) < 100:
                    break
                offset += len(matches["users"])
            if not found:
                _, created = request(base, "POST", "/manage/users", key, {
                    "email": user["email"], "password": password, "name": user["name"], "role": user["role"]})
                found = created["user"]
            _, detail = request(base, "GET", f"/manage/users/{found['id']}", key)
            if user["role"] not in detail["roles"]:
                raise Failure(f"Existing role differs for {slug}/{user['name']}; reconcile explicitly")
            # Verify credentials without retaining the returned token or logging a response.
            _, login = request(base, "POST", f"/t/{slug}/auth/login", "", {"email": user["email"], "password": password})
            if login["user"]["id"] != found["id"]:
                raise Failure("Login returned a different subject")
            request(base, "POST", f"/t/{slug}/auth/logout", login["access_token"], {"refresh_token": login["refresh_token"]})
            user_receipts.append({"subject": found["id"], "name": user["name"], "role": user["role"], "login": "pass"})
        receipt["tenants"].append({"slug": slug, "tenant_id": existing["id"], "client_id": desired["client_id"], "users": user_receipts})
    return receipt


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--config", type=Path, required=True)
    p.add_argument("--secret-prefix", default="forma-demo/management")
    p.add_argument("--region", default="us-east-1")
    p.add_argument("--synthetic-secret-file", type=Path)
    args = p.parse_args()
    config = json.loads(args.config.read_text())
    auth = urllib.parse.urlsplit(config["auth_origin"])
    if (auth.scheme not in ("http", "https") or not auth.hostname or auth.username or auth.password
            or auth.query or auth.fragment or auth.path not in ("", "/")):
        raise Failure("auth_origin must be an HTTP(S) origin without credentials, path, query or fragment")
    for tenant in config["tenants"]:
        for user in tenant["users"]:
            if not os.environ.get(user["password_env"]):
                raise Failure(f"Missing synthetic password environment variable {user['password_env']}")
    if args.synthetic_secret_file:
        if auth.hostname not in ("localhost", "127.0.0.1", "::1"):
            raise Failure("Synthetic local secrets are permitted only for loopback auth")
    elif auth.scheme != "https":
        raise Failure("Hosted provisioning requires HTTPS")
    receipt = provision(config, Secrets(args.secret_prefix, args.synthetic_secret_file, args.region))
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    try:
        main()
    except (Failure, KeyError, OSError, ValueError) as e:
        # Do not dump request objects, subprocess stderr, credentials or token bodies.
        print(f"Provisioning stopped: {e}", file=sys.stderr)
        sys.exit(1)
