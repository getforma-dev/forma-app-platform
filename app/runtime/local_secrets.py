"""Test-only AWS Secrets Manager stand-in for the provider capability secret (never packaged, never AWS).

ADR-003 amendment of 2026-10-30 (APP-074 review M1): the operator writes the
environment's provider invitation capabilities into one Secrets Manager secret
with PutSecretValue, and ECS injects that secret into the service task. This
loopback server speaks the two JSON 1.1 calls the operator makes through the
real boto3/botocore client (`secretsmanager.GetSecretValue` and
`secretsmanager.PutSecretValue`, selected by `X-Amz-Target`), so the operator
code under test is the code that runs against AWS:

- unsigned requests are refused (`AccessDeniedException`), as AWS does;
- `grants` models the caller's identity policy per access key id: a call whose
  action the key does not hold is `AccessDeniedException`;
- a secret has one AWSCURRENT version; PutSecretValue makes a new version and
  is idempotent for a repeated `ClientRequestToken` with the same value (the
  token botocore fills in once per call and reuses on retries) and refuses a
  reused token with another value (`ResourceExistsException`);
- a SecretString over 65,536 bytes is refused (`InvalidParameterException`);
- `deny_puts` turns every PutSecretValue into `AccessDeniedException` (nothing
  written) and `fail_after_put` applies the write and then answers 500 once per
  armed count (the ambiguous outcome; botocore retries it with the same token).

Every request is recorded (target, access key id, never the value). Run as a
script, it serves on 127.0.0.1, prints `{"port": N}` and answers one JSON
control line per stdin line until stdin closes (used by the Rust campaigns):
`{"op": "get"}` -> `{"secret_string": ...}`, `{"op": "put", "value": text}` (a new
AWSCURRENT version, as the operator's `aws secretsmanager put-secret-value` from the
composed file writes it; APP-078 B1), `{"op": "deny_puts", "value": bool}`,
`{"op": "counts"}` -> `{"get": n, "put": n}`.
"""
import argparse
import json
import re
import sys
import threading
import time
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

SECRET_STRING_MAX = 65536
CREDENTIAL = re.compile(r"Credential=([A-Za-z0-9_-]+)/")
ACTIONS = {"secretsmanager.GetSecretValue": "secretsmanager:GetSecretValue",
           "secretsmanager.PutSecretValue": "secretsmanager:PutSecretValue"}


class Stub:
    def __init__(self, secret_arn, initial="{}", *, grants=None):
        self.arn = secret_arn
        self.name = secret_arn.split(":secret:", 1)[1]
        self.versions = {}  # token -> value
        self.current, self.version = initial, str(uuid.uuid4())
        self.grants = None if grants is None else {key: frozenset(actions) for key, actions in grants.items()}
        self.requests, self.deny_puts, self.fail_after_put, self.get_delay = [], False, 0, 0
        self.lock = threading.Lock()
        stub = self

        class Handler(BaseHTTPRequestHandler):
            protocol_version = "HTTP/1.1"

            def log_message(self, *args):
                pass

            def reply(self, status, body):
                data = json.dumps(body).encode("utf-8")
                self.send_response(status)
                self.send_header("Content-Type", "application/x-amz-json-1.1")
                self.send_header("x-amzn-RequestId", str(uuid.uuid4()))
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                self.wfile.write(data)

            def error(self, status, kind, message="refused"):
                self.reply(status, {"__type": kind, "message": message})

            def do_POST(self):
                body = self.rfile.read(int(self.headers.get("Content-Length", "0")))
                target = self.headers.get("X-Amz-Target", "")
                authorization = self.headers.get("Authorization", "")
                credential = CREDENTIAL.search(authorization)
                key_id = credential.group(1) if credential else None
                with stub.lock:
                    stub.requests.append({"target": target, "key_id": key_id})
                if not authorization.startswith("AWS4-HMAC-SHA256 ") or key_id is None:
                    return self.error(400, "AccessDeniedException")
                action = ACTIONS.get(target)
                if action is None:
                    return self.error(400, "UnknownOperationException")
                if stub.grants is not None and action not in stub.grants.get(key_id, ()):
                    return self.error(400, "AccessDeniedException")
                try:
                    request = json.loads(body.decode("utf-8"))
                except ValueError:
                    return self.error(400, "SerializationException")
                if request.get("SecretId") not in (stub.arn, stub.name):
                    return self.error(400, "ResourceNotFoundException")
                if target == "secretsmanager.GetSecretValue":
                    if stub.get_delay:  # widens a read-modify-write window, for the lost-update test
                        time.sleep(stub.get_delay)
                    with stub.lock:
                        value, version = stub.current, stub.version
                    return self.reply(200, {"ARN": stub.arn, "Name": stub.name, "VersionId": version, "SecretString": value,
                                            "VersionStages": ["AWSCURRENT"], "CreatedDate": time.time()})
                value, token = request.get("SecretString"), request.get("ClientRequestToken")
                if not isinstance(value, str) or not isinstance(token, str) or len(value.encode("utf-8")) > SECRET_STRING_MAX:
                    return self.error(400, "InvalidParameterException")
                with stub.lock:
                    if stub.deny_puts:
                        outcome = "denied"
                    elif token in stub.versions and stub.versions[token] != value:
                        outcome = "exists"
                    else:
                        if token not in stub.versions:
                            stub.versions[token] = value
                            stub.current, stub.version = value, token
                        outcome = "ambiguous" if stub.fail_after_put > 0 else "written"
                        if outcome == "ambiguous":
                            stub.fail_after_put -= 1
                if outcome == "denied":
                    return self.error(400, "AccessDeniedException")
                if outcome == "exists":
                    return self.error(400, "ResourceExistsException")
                if outcome == "ambiguous":
                    return self.error(500, "InternalServiceError")
                self.reply(200, {"ARN": stub.arn, "Name": stub.name, "VersionId": token, "VersionStages": ["AWSCURRENT"]})

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)

    @property
    def endpoint(self):
        return "http://127.0.0.1:%d" % self.server.server_address[1]

    def start(self):
        self.thread.start()
        return self

    def stop(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=5)

    __enter__ = start

    def __exit__(self, *exc):
        self.stop()

    def value(self):
        with self.lock:
            return self.current

    def set_value(self, text):
        """Fixture-only replacement of AWSCURRENT (a new version), as a console edit would."""
        with self.lock:
            self.current, self.version = text, str(uuid.uuid4())

    def count(self, target):
        return sum(1 for r in self.requests if r["target"] == "secretsmanager." + target)

    def client(self, access_key_id=None):
        """A real botocore Secrets Manager client for this stand-in, signed with throwaway local values."""
        import boto3
        from botocore.config import Config
        session = boto3.session.Session(aws_access_key_id=access_key_id or LOCAL_ACCESS_KEY_ID,
                                        aws_secret_access_key=LOCAL_SECRET_ACCESS_KEY, region_name="us-east-1")
        return session.client("secretsmanager", endpoint_url=self.endpoint,
                              config=Config(retries={"total_max_attempts": 3, "mode": "standard"}))


# Throwaway local signing values for the stand-in; never real credentials.
LOCAL_ACCESS_KEY_ID = "APP074-LOCAL-SECRET-STUB-KEY-ID"
LOCAL_SECRET_ACCESS_KEY = "app074-local-secret-stub-test-value"
LOCAL_SECRET_ARN = "arn:aws:secretsmanager:us-east-1:000000000000:secret:genui-provider-capabilities-local-AbCdEf"


def environment(endpoint, secret_arn=LOCAL_SECRET_ARN):
    """The operator's environment for this stand-in: botocore's own service endpoint override and
    throwaway signing values (no profile, no metadata lookup)."""
    return {"AWS_ENDPOINT_URL_SECRETS_MANAGER": endpoint, "AWS_ACCESS_KEY_ID": LOCAL_ACCESS_KEY_ID,
            "AWS_SECRET_ACCESS_KEY": LOCAL_SECRET_ACCESS_KEY, "AWS_REGION": "us-east-1", "AWS_EC2_METADATA_DISABLED": "true",
            "AWS_CONFIG_FILE": "/dev/null", "AWS_SHARED_CREDENTIALS_FILE": "/dev/null",
            "GENUI_PROVIDER_CAPABILITY_SECRET_ID": secret_arn}


def _serve():
    parser = argparse.ArgumentParser()
    parser.add_argument("--secret-arn", default=LOCAL_SECRET_ARN)
    args = parser.parse_args()
    stub = Stub(args.secret_arn).start()
    print(json.dumps({"port": stub.server.server_address[1]}), flush=True)
    try:
        for line in sys.stdin:
            command = json.loads(line)
            if command["op"] == "get":
                reply = {"secret_string": stub.value()}
            elif command["op"] == "put":
                stub.set_value(command["value"])
                reply = {"ok": True}
            elif command["op"] == "deny_puts":
                stub.deny_puts = bool(command["value"])
                reply = {"ok": True}
            elif command["op"] == "counts":
                reply = {"get": stub.count("GetSecretValue"), "put": stub.count("PutSecretValue")}
            else:
                reply = {"error": "unknown_op"}
            print(json.dumps(reply), flush=True)
    finally:
        stub.stop()


if __name__ == "__main__":
    _serve()
