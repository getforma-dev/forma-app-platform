"""Test-only S3-compatible stub for APP-077's upload-sources (never packaged, never AWS).

A loopback HTTP server that speaks the path-style object calls the operator
makes through the real boto3/botocore client (signed PutObject, including the
conditional `If-None-Match: *` write that answers 412 for an existing key, and
HeadObject, plus GetObject for inspection) and enforces the two bucket-policy denials of
infra/src/stack.ts for each tenant prefix: a PutObject without
`x-amz-server-side-encryption: aws:kms`, or with a KMS key id other than that
prefix's tenant key, is AccessDenied. It also refuses unsigned requests and a
body whose `x-amz-checksum-sha256` does not match, as S3 does. Objects are
versioned in memory; every request is recorded for assertions.
"""
import base64
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import threading
import uuid


class Stub:
    def __init__(self, bucket, tenant_keys):
        self.bucket, self.tenant_keys = bucket, dict(tenant_keys)  # prefix -> key arn
        self.objects, self.requests, self.fail_puts, self.put_count = {}, [], set(), 0
        self.lock = threading.Lock()
        stub = self

        class Handler(BaseHTTPRequestHandler):
            protocol_version = "HTTP/1.1"

            def log_message(self, *args):
                pass

            def reply(self, status, headers=None, body=b""):
                self.send_response(status)
                for name, value in (headers or {}).items():
                    self.send_header(name, value)
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                if self.command != "HEAD":
                    self.wfile.write(body)

            def error(self, status, code):
                self.reply(status, {"Content-Type": "application/xml"},
                           ("<?xml version=\"1.0\"?><Error><Code>%s</Code><Message>%s</Message></Error>" % (code, code)).encode())

            def target(self):
                path = self.path.split("?", 1)[0]
                prefix = "/" + stub.bucket + "/"
                return path[len(prefix):] if path.startswith(prefix) else None

            def record(self, key, body=b""):
                with stub.lock:
                    stub.requests.append({"method": self.command, "key": key, "headers": {k.lower(): v for k, v in self.headers.items()},
                                          "body_sha256": hashlib.sha256(body).hexdigest()})

            def signed(self):
                return self.headers.get("Authorization", "").startswith("AWS4-HMAC-SHA256 ")

            def object_headers(self, item, checksum):
                headers = {"ETag": '"%s"' % hashlib.md5(item["body"]).hexdigest(), "x-amz-version-id": item["version"],
                           "x-amz-server-side-encryption": item["sse"], "x-amz-server-side-encryption-aws-kms-key-id": item["key_id"],
                           "Content-Type": item["content_type"]}
                if checksum:
                    headers["x-amz-checksum-sha256"] = item["checksum"]
                return headers

            def do_PUT(self):
                key = self.target()
                body = self.rfile.read(int(self.headers.get("Content-Length", "0")))
                self.record(key, body)
                if key is None:
                    return self.error(404, "NoSuchBucket")
                if not self.signed():
                    return self.error(403, "AccessDenied")
                with stub.lock:
                    stub.put_count += 1
                    failing = stub.put_count in stub.fail_puts
                if failing:  # injected server fault for the Nth signed PUT (1-based)
                    return self.error(500, "InternalError")
                sse = self.headers.get("x-amz-server-side-encryption")
                key_id = self.headers.get("x-amz-server-side-encryption-aws-kms-key-id")
                tenant = next((arn for prefix, arn in stub.tenant_keys.items() if key.startswith(prefix)), None)
                # The bucket policy's two explicit Deny statements for this prefix.
                if sse != "aws:kms" or tenant is None or key_id != tenant:
                    return self.error(403, "AccessDenied")
                checksum = base64.b64encode(hashlib.sha256(body).digest()).decode()
                if self.headers.get("x-amz-checksum-sha256") not in (None, checksum):
                    return self.error(400, "BadDigest")
                item = {"body": body, "sse": sse, "key_id": key_id, "checksum": checksum, "version": uuid.uuid4().hex,
                        "content_type": self.headers.get("Content-Type", "binary/octet-stream")}
                with stub.lock:
                    # S3 conditional write: If-None-Match: * refuses an existing key atomically.
                    if self.headers.get("If-None-Match") == "*" and stub.objects.get(key):
                        exists = True
                    else:
                        exists = False
                        stub.objects.setdefault(key, []).append(item)
                if exists:
                    return self.error(412, "PreconditionFailed")
                headers = self.object_headers(item, self.headers.get("x-amz-checksum-sha256") is not None)
                headers.pop("Content-Type")
                self.reply(200, headers)

            def do_HEAD(self):
                self.read(head=True)

            def do_GET(self):
                self.read(head=False)

            def read(self, head):
                key = self.target()
                self.record(key)
                if not self.signed():
                    return self.error(403, "AccessDenied")
                with stub.lock:
                    versions = stub.objects.get(key)
                if not versions:
                    return self.error(404, "NoSuchKey")
                item = versions[-1]
                headers = self.object_headers(item, self.headers.get("x-amz-checksum-mode") == "ENABLED")
                self.reply(200, headers, item["body"])

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)

    @property
    def endpoint(self):
        return "http://127.0.0.1:%d" % self.server.server_address[1]

    def __enter__(self):
        self.thread.start()
        return self

    def __exit__(self, *exc):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=5)

    def client(self):
        """A real botocore S3 client signed with throwaway local test values."""
        import boto3
        from botocore.config import Config
        session = boto3.session.Session(aws_access_key_id="APP077-LOCAL-STUB-KEY-ID", aws_secret_access_key="app077-local-stub-test-value",
                                        region_name="us-east-1")
        return session.client("s3", endpoint_url=self.endpoint, config=Config(
            s3={"addressing_style": "path"}, retries={"total_max_attempts": 1, "mode": "standard"},
            request_checksum_calculation="when_required", response_checksum_validation="when_required"))

    def puts(self):
        return [r for r in self.requests if r["method"] == "PUT"]
