"""APP-077 operator package entry (ADR-005, ADR-007 D7.19, ADR-009 B1; plan Task B4).

Closed subcommands for the separate migration identity, never an HTTP,
application or signup path, and never generic SQL:

  migrate          runtime.apply_migrations for every landed migration (one
                   transaction; a repeat is unchanged)
  verify-catalog   read-only proof that the database equals the reviewed
                   latest catalog
  seed             tenant_seed.seed with validated envelopes, after the catalog
                   proof, in one caller-owned transaction (exact-match
                   idempotent; a changed seed refuses whole)
  admission ...    passed unchanged to admission_operator.main: whatever closed
                   subcommands that module defines (APP-073 and later), after
                   the same pre-connect environment checks; `issue` and
                   `reconcile` are refused inside a managed (ECS) task
  upload-sources   the closed fixture-source table below: original bytes
                   normalized through the reviewed APP-004/005 adapter path of
                   fixtures/qualify.py (parser-host and its WASM guest, then
                   the trusted asset selection and DuckDB Parquet), written to
                   ADR-009 B1's layout o/{org}/s/{source}/r/{rev}/original and
                   .../part.parquet with SSE-KMS under the tenant key, at a NEW
                   revision derived from the original (never the seeded
                   placeholder revision); prints one registration record for
                   APP-012's register-source-revision; `--plan` makes no AWS call

Every database command connects as genui_migration over verify-full TLS using
only the libpq environment (PGHOST, PGPORT, PGUSER, PGPASSWORD, PGSSLROOTCERT,
PGDATABASE=genui); the password is never read into an argument, printed or
logged. The host must be one DNS name or address (never a socket path, an
abstract socket or a host list) and the trust root a CA file, inside a managed
task exactly the image's pinned RDS bundle. The task holds no application
role-assumption grant: no subcommand refreshes or assumes the runtime's
database or tenant roles. Refusals print a fixed reason code on standard error
and never echo an argument or environment value. Run as
`python3 -I -u operator.py ...`: isolated mode so this file's directory never
shadows the standard library's `operator` module, unbuffered so a receipt
printed after a commit is not lost to a later kill.
"""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import uuid

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
for _path in (ROOT / "app" / "runtime", ROOT / "contracts", ROOT / "fixtures"):
    if str(_path) not in sys.path:
        sys.path.append(str(_path))

COMMANDS = ("migrate", "verify-catalog", "seed", "admission", "upload-sources")
OVERRIDES_FILE = HERE / "overrides.json"
# The image places the parser bytes built from parser/ (ADR-009 D9.1) here, with
# the manifest the build wrote for them; a checkout has none.
PARSER_MANIFEST = ROOT / "bin" / "parser-manifest.json"
FIXTURES = ROOT / "fixtures" / "phase-one" / "v1"
NORTHSTAR = "00000000-0000-4000-8000-000000000001"
# The closed fixture-source table. Phase 1 has one: the Northstar organization's
# events source as seeded by tenant_seed. The seeded revision row carries
# placeholder digests and is immutable (FK-chained; ADR-009 D9.3: nothing is
# rewritten), so the real objects go to a NEW revision of the same source whose
# id is derived from the original (derived_revision_id); APP-012's
# register-source-revision records its row from the printed registration record
# (APP-077 review M2, coordinated decision). The CSV digest is pinned here and
# must also equal the fixture manifest's.
SOURCES = (
    {"organization_id": NORTHSTAR, "app_id": "00000000-0000-4000-8000-000000000065", "input_id": "events", "fixture_id": "northstar-events-csv",
     "original_digest": "sha256:6e0f0ed7f182509d7cbadb70405c21d2e25f24e89d80c2137297e411fc034647", "asset": "build-worker-17"},
)
PARSER_CONFIG = {"profile": "demo", "format": "csv"}
OBJECT_NAMES = ("original", "part.parquet")
CONTENT_TYPES = {"original": "text/csv", "part.parquet": "application/vnd.apache.parquet"}
UUID = re.compile(r"[0-9a-f]{8}-[0-9a-f]{4}-[47][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}")
BUCKET = re.compile(r"[a-z0-9][a-z0-9.-]{1,61}[a-z0-9]")
KMS_KEY = re.compile(r"arn:aws:kms:[a-z0-9-]+:[0-9]{12}:key/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")
DIGEST = re.compile(r"sha256:[0-9a-f]{64}")
# One DNS name or IPv4 address: no '/' socket path, no '@' abstract socket, no
# comma-separated host list (libpq tries each; sslmode does not apply to a
# socket). APP-077 review m2.
HOSTNAME = re.compile(r"[A-Za-z0-9](?:[A-Za-z0-9.-]{0,251}[A-Za-z0-9])?")
# The image's pinned RDS CA bundle (Dockerfile; check_image.py pins its digest).
PINNED_CA = "/etc/ssl/certs/rds-global-bundle.pem"
# Fargate always sets these in a task's containers (review M3).
MANAGED_TASK_VARIABLES = ("ECS_CONTAINER_METADATA_URI_V4", "ECS_CONTAINER_METADATA_URI")
# admission subcommands never run in a managed task: `issue` prints the raw
# code to standard output, which there is the awslogs group; `reconcile` (like
# a provider invitation) needs the management key and a private directory;
# `register-source-revision` reads a local record file that a command-only
# override cannot deliver (APP-012).
MANAGED_REFUSED = ("issue", "reconcile", "register-source-revision")
# The record APP-012's register-source-revision consumes, one per uploaded revision.
REGISTRATION_FIELDS = ("organization_id", "source_id", "revision_id", "content_digest", "normalized_digest", "object_digest",
                       "row_count", "contract_digest")


class Refused(ValueError):
    """A refusal whose message is a fixed reason code, never a value."""


def require(condition, reason="refused"):
    if not condition:
        raise Refused(reason)


def sha256(data):
    return "sha256:" + hashlib.sha256(data).hexdigest()


class Parser(argparse.ArgumentParser):
    def error(self, message):
        raise Refused("usage")


def parser():
    p = Parser(prog="operator.py", add_help=False)
    sub = p.add_subparsers(dest="command", required=True)
    for name in ("migrate", "verify-catalog", "seed"):
        sub.add_parser(name, add_help=False)
    sub.add_parser("upload-sources", add_help=False).add_argument("--plan", action="store_true")
    return p


def parse(argv):
    """The closed command line. `admission` keeps its own arguments for its own parser."""
    argv = list(argv)
    require(argv and argv[0] in COMMANDS, "usage")
    if argv[0] == "admission":
        require(len(argv) > 1, "usage")
        return argparse.Namespace(command="admission", rest=argv[1:])
    return parser().parse_args(argv)


# --- the migration identity -----------------------------------------------------

def managed_task():
    return any(name in os.environ for name in MANAGED_TASK_VARIABLES)


def database_environment():
    """Every pre-connect check of a database command (migrate, verify-catalog, seed, admission)."""
    env = os.environ
    host = env.get("PGHOST", "")
    # A Unix-socket host ('/' path or '@' abstract name) never uses TLS; libpq
    # would also prefer GSSAPI encryption. A host list could fall back to one.
    require(host and not host.startswith(("/", "@")), "socket_host_refused")
    require(HOSTNAME.fullmatch(host) is not None, "host_refused")
    require(env.get("PGUSER") == "genui_migration", "migration_identity_required")
    require(env.get("PGDATABASE", "genui") == "genui", "database_refused")
    require(env.get("PGSSLMODE", "verify-full") == "verify-full", "tls_mode_refused")
    ca = env.get("PGSSLROOTCERT", "")
    require(bool(ca), "ca_required")
    # A CA file, never libpq's `system` (the OS trust store); in a managed task
    # exactly the image's pinned RDS bundle.
    require(ca != "system" and os.path.isabs(ca) and os.path.isfile(ca), "ca_refused")
    require(not managed_task() or ca == PINNED_CA, "ca_refused")
    require(re.fullmatch(r"[1-9][0-9]{0,4}", env.get("PGPORT", "5432")) is not None, "port_refused")
    return host, int(env.get("PGPORT", "5432"))


def connect(read_only=False):
    import psycopg
    host, port = database_environment()
    options = "-c search_path=pg_catalog,pg_temp -c statement_timeout=30000 -c lock_timeout=5000"
    if read_only:
        options += " -c default_transaction_read_only=on"
    return psycopg.connect(host=host, port=port, dbname="genui", user="genui_migration", password=os.environ.get("PGPASSWORD"),
                           sslmode="verify-full", gssencmode="disable", sslrootcert=os.environ["PGSSLROOTCERT"],
                           connect_timeout=5, options=options)


def check_identity(connection):
    require(not connection.autocommit, "transaction_required")
    connection.execute("SET LOCAL search_path=pg_catalog, pg_temp")
    require(connection.execute("SELECT session_user::text,current_user::text,current_database()").fetchone()
            == ("genui_migration", "genui_migration", "genui"), "migration_identity_refused")


def pinned_directory():
    import runtime
    directory = runtime.migration_directory()
    require(runtime.migration_manifest(directory)["database_version"] == runtime.DATABASE_SCHEMA_VERSION, "schema_pin_refused")
    return runtime, directory


def migrate():
    runtime, directory = pinned_directory()
    sources = runtime.migration_sources(directory)
    connection = connect()
    try:
        check_identity(connection)
        runtime.apply_migrations(connection, directory)
        connection.commit()
    finally:
        connection.close()
    return {"command": "migrate", "database_version": len(sources),
            "migrations": [{"version": version, "sha256": digest} for version, digest, _ in sources]}


def verify_catalog():
    runtime, directory = pinned_directory()
    connection = connect(read_only=True)
    try:
        check_identity(connection)
        try:
            version = runtime.verify_schema_catalog(connection, directory)
        except ValueError as error:
            raise Refused("catalog_refused") from error
        # Only after the catalog proof is the version row an expected relation.
        row = connection.execute("SELECT singleton,version FROM genui_meta.schema_version").fetchall()
        require(row == [(True, version)], "schema_version_refused")
        connection.rollback()
    finally:
        connection.close()
    return {"command": "verify-catalog", "catalog_version": version, "read_only": True}


def seed():
    runtime, directory = pinned_directory()
    import tenant_seed
    bundles = tenant_seed.make_bundles()  # validates every envelope before any connection
    connection = connect()
    try:
        check_identity(connection)
        try:
            version = runtime.verify_schema_catalog(connection, directory)
        except ValueError as error:
            raise Refused("catalog_refused") from error
        try:
            tenant_seed.seed(connection, bundles)
        except ValueError as error:
            # Includes "existing seed differs": nothing is overwritten and the
            # whole transaction rolls back.
            raise Refused("seed_refused") from error
        connection.commit()
    finally:
        connection.close()
    return {"command": "seed", "catalog_version": version, "bundles": len(bundles),
            "organizations": sorted({b["context"]["organization_id"] for b in bundles}),
            "apps": sorted({(b["context"]["organization_id"], b["definition"]["app_id"]) for b in bundles})}


# --- upload-sources: the reviewed adapter path ------------------------------------

class ParserBinding:
    """The parser-host executable and WASM guest under use, bound by digest."""

    def __init__(self, host, wasm, guest_sha256, manifest_sha256=None, host_sha256=None):
        self.host, self.wasm = Path(host), Path(wasm)
        self.guest_sha256, self.manifest_sha256, self.host_sha256 = guest_sha256, manifest_sha256, host_sha256

    @classmethod
    def from_manifest(cls, path=PARSER_MANIFEST):
        """The image's parser bytes must equal what its build's manifest names."""
        path = Path(path)
        require(path.is_file(), "parser_unavailable")
        raw = path.read_bytes()
        manifest = json.loads(raw)
        require(manifest.get("schema") == "genui-parser-manifest/1", "parser_manifest_refused")
        host, guest = Path(manifest["host"]["path"]), Path(manifest["guest"]["path"])
        require(host.parent == path.parent and guest.parent == path.parent, "parser_manifest_refused")
        for item, file in ((manifest["host"], host), (manifest["guest"], guest)):
            require(file.is_file() and hashlib.sha256(file.read_bytes()).hexdigest() == item["sha256"], "parser_bytes_refused")
        return cls(host, guest, "sha256:" + manifest["guest"]["sha256"], sha256(raw), "sha256:" + manifest["host"]["sha256"])

    @classmethod
    def from_files(cls, host, wasm):
        """A checkout's own build (tests): the guest digest is taken from its bytes."""
        return cls(host, wasm, sha256(Path(wasm).read_bytes()), None, sha256(Path(host).read_bytes()))

    def run(self, source, scratch):
        config = Path(scratch) / "parser-config.json"
        config.write_text(json.dumps(PARSER_CONFIG))
        try:
            completed = subprocess.run([str(self.host), str(self.wasm), str(source), str(config)], capture_output=True,
                                       timeout=180, env={}, stdin=subprocess.DEVNULL, cwd=scratch)
        except (OSError, subprocess.TimeoutExpired) as error:
            raise Refused("parser_failed") from error
        require(completed.returncode == 0, "parser_failed")
        return json.loads(completed.stdout)


def fixture_entry(entry, root=FIXTURES):
    import verify
    manifest = json.loads(verify.member(Path(root), "manifest.json").read_bytes())
    require(manifest.get("edition") == "app011-fixtures-v1" and manifest.get("synthetic") is True, "fixture_edition_refused")
    found = [item for item in manifest["sources"] if item.get("id") == entry["fixture_id"]]
    require(len(found) == 1, "fixture_refused")
    item = found[0]
    require(item["format"] == "csv" and item["sheet"] is None and item["parser_expect"] == "accepted", "fixture_refused")
    require(item["original_digest"] == entry["original_digest"], "fixture_digest_refused")
    path = verify.member(Path(root), item["path"])
    data = path.read_bytes()
    require(len(data) == item["bytes"] and sha256(data) == entry["original_digest"], "fixture_digest_refused")
    return item, path, data


def seeded_source(entry):
    """The seeded source this upload adds a revision to: ids, contract and the placeholder revision."""
    import tenant_seed
    from validate import digest
    bundles = [b for b in tenant_seed.make_bundles()
               if b["context"]["organization_id"] == entry["organization_id"] and b["definition"]["app_id"] == entry["app_id"]]
    require(len(bundles) == 1, "seed_source_refused")
    bundle = bundles[0]
    sources = [s for s in bundle["sources"] if s["input_contract"]["id"] == entry["input_id"]]
    bindings = [b for b in bundle["work"]["bindings"] if b["input_id"] == entry["input_id"]]
    require(len(sources) == 1 and len(bindings) == 1, "seed_source_refused")
    source, binding = sources[0], bindings[0]
    contract = source["input_contract"]
    contract_digest = digest("input-contract", contract)
    require(binding["contract_digest"] == contract_digest and binding["source_id"] == source["source_id"]
            and binding["source_revision_id"] == source["revision_id"], "seed_source_refused")
    require([f["id"] for f in contract["fields"]] == ["time", "destination", "bytes"], "contract_refused")
    return source, contract_digest


def normalize(rows, scratch):
    """fixtures/qualify.py's trusted adapter, written to the events_v1 contract shape.

    Rows are the asset-selected parser rows in original order. Typing uses the
    reviewed fixture oracle's strict conversions; order is time, then original
    record (ADR-009 D9.3's determinism rule); one thread, ZSTD.
    """
    import duckdb
    import verify
    typed = sorted(((verify.timestamp(row["time"]).replace(tzinfo=None), row["destination"], verify.integer(row["bytes"]), index)
                    for index, row in enumerate(rows, 1)), key=lambda item: (item[0], item[3]))
    require(all(destination for _, destination, _, _ in typed), "destination_refused")
    target = Path(scratch) / "part.parquet"
    connection = duckdb.connect()
    try:
        connection.execute("SET threads=1")
        connection.execute("CREATE TABLE events(time TIMESTAMP NOT NULL, destination VARCHAR NOT NULL, bytes BIGINT NOT NULL, original INTEGER NOT NULL)")
        connection.executemany("INSERT INTO events VALUES (?, ?, ?, ?)", typed)
        connection.execute("COPY (SELECT time, destination, bytes FROM events ORDER BY time, original) TO ? "
                           "(FORMAT PARQUET, COMPRESSION ZSTD, ROW_GROUP_SIZE 100000)", [str(target)])
        back = connection.execute("SELECT time, destination, bytes FROM read_parquet(?)", [str(target)]).fetchall()
        columns = [row[0] for row in connection.execute("DESCRIBE SELECT * FROM read_parquet(?)", [str(target)]).fetchall()]
    finally:
        connection.close()
    require(columns == ["time", "destination", "bytes"] and back == [item[:3] for item in typed], "normalized_round_trip_refused")
    return target.read_bytes(), duckdb.__version__, len(typed)


def derived_revision_id(organization, content_digest):
    """The upload's revision id: uuid5 over the organization id and the original's digest.

    RFC 9562 name-based (SHA-1) with the organization id as namespace and the
    `sha256:<hex>` content digest as name, then stamped version 4: the contract
    (contracts/schema.json source_revision.revision_id) and the operator
    channels accept only version 4 or 7 identifiers. Deterministic, so a rerun
    targets the same keys, and one original has one revision per organization:
    a different normalization of the same original meets the existing
    part.parquet and refuses (existing_object_differs) instead of overwriting.
    """
    require(isinstance(organization, str) and UUID.fullmatch(organization) is not None
            and isinstance(content_digest, str) and DIGEST.fullmatch(content_digest) is not None, "revision_refused")
    return str(uuid.UUID(bytes=uuid.uuid5(uuid.UUID(organization), content_digest).bytes, version=4))


def object_key(organization, source, revision, name):
    require(UUID.fullmatch(organization) and UUID.fullmatch(source) and UUID.fullmatch(revision) and name in OBJECT_NAMES, "key_refused")
    return "o/%s/s/%s/r/%s/%s" % (organization, source, revision, name)


def plan_sources(binding=None, fixtures=FIXTURES):
    """Every object and digest of the closed source table; no AWS call."""
    import qualify
    import verify
    binding = binding or ParserBinding.from_manifest()
    plans = []
    for entry in SOURCES:
        item, path, original = fixture_entry(entry, fixtures)
        source, contract_digest = seeded_source(entry)
        with tempfile.TemporaryDirectory(prefix="genui-operator-") as scratch:
            report = binding.run(path, scratch)
            require(report.get("input_sha256") == entry["original_digest"] and report.get("wasm_sha256") == binding.guest_sha256
                    and report.get("interface_imports") == [], "parser_binding_refused")
            result = report.get("result") or {}
            require(result.get("status") == "parsed" and result["result"].get("mapping_status") == "inspection_required", "parser_refused")
            rows = qualify.parser_rows(report)
            # The independent fixture reader must agree with the parser (as in qualify.py).
            require(rows == verify.table(Path(fixtures), item), "parser_reader_disagreement")
            selected = [row for row in rows if row["asset"] == entry["asset"]]
            require(selected, "asset_selection_empty")
            normalized, duckdb_version, row_count = normalize(selected, scratch)
        # Review M2: never the seeded placeholder revision's immutable keys.
        revision = derived_revision_id(entry["organization_id"], sha256(original))
        require(revision != source["revision_id"], "placeholder_revision_refused")
        objects = []
        for name, data in (("original", original), ("part.parquet", normalized)):
            objects.append({"name": name, "key": object_key(entry["organization_id"], source["source_id"], revision, name),
                            "sha256": sha256(data), "bytes": len(data), "content_type": CONTENT_TYPES[name], "data": data})
        plans.append({
            "organization_id": entry["organization_id"], "source_id": source["source_id"], "revision_id": revision,
            "input_id": entry["input_id"], "contract_id": source["input_contract"]["contract_id"],
            "content_digest": sha256(original), "normalized_digest": sha256(normalized), "contract_digest": contract_digest,
            "row_count": row_count, "parsed_rows": len(rows), "selected_asset": entry["asset"],
            "seeded_placeholder_not_written": {"revision_id": source["revision_id"], "content_digest": source["content_digest"],
                                               "normalized_digest": source["normalized_digest"], "row_count": source["row_count"]},
            "adapter": {"path": "parser-host + WASM guest -> fixtures/qualify.py parser_rows -> asset selection -> DuckDB Parquet ZSTD",
                        "parser_guest_sha256": binding.guest_sha256, "parser_host_sha256": binding.host_sha256,
                        "parser_manifest_sha256": binding.manifest_sha256, "duckdb_version": duckdb_version,
                        "fixture_manifest_sha256": sha256((Path(fixtures) / "manifest.json").read_bytes())},
            "objects": objects})
    return plans


def public(plans):
    return [dict(plan, objects=[{k: v for k, v in item.items() if k != "data"} for item in plan["objects"]]) for plan in plans]


def load_tenants(text):
    """GENUI_OPERATOR_TENANTS_JSON: exactly [{organization_id, kms_key_arn, prefix}]."""
    try:
        value = json.loads(text)
    except (TypeError, ValueError) as error:
        raise Refused("tenants_refused") from error
    require(isinstance(value, list) and value, "tenants_refused")
    tenants = {}
    for item in value:
        require(isinstance(item, dict) and set(item) == {"organization_id", "kms_key_arn", "prefix"}, "tenants_refused")
        organization = item["organization_id"]
        require(isinstance(organization, str) and UUID.fullmatch(organization) is not None and organization not in tenants, "tenants_refused")
        require(item["prefix"] == "o/%s/" % organization, "tenant_prefix_refused")
        require(isinstance(item["kms_key_arn"], str) and KMS_KEY.fullmatch(item["kms_key_arn"]) is not None, "tenant_key_refused")
        tenants[organization] = item
    return tenants


def _status(error):
    response = getattr(error, "response", None) or {}
    return str(response.get("Error", {}).get("Code", "")), response.get("ResponseMetadata", {}).get("HTTPStatusCode")


def upload(client, bucket, tenants, plans):
    """Object-only, original before part.parquet; an equal object is left unchanged.

    Each PUT is conditional (`If-None-Match: *`), so a revision key is written
    at most once even under a concurrent run, and no ListBucket is needed. On
    412 the existing object is read back by HeadObject: other bytes, another
    key or no SSE-KMS refuse (a revision key is immutable). A partial run
    completes on rerun. No database row is written here.
    """
    from botocore.exceptions import BotoCoreError, ClientError
    require(isinstance(bucket, str) and BUCKET.fullmatch(bucket) is not None, "bucket_refused")
    receipts = []
    for plan in plans:
        tenant = tenants.get(plan["organization_id"])
        require(tenant is not None, "tenant_missing")
        written = []
        for item in plan["objects"]:
            require(item["key"].startswith(tenant["prefix"]), "tenant_prefix_refused")
            checksum = base64.b64encode(hashlib.sha256(item["data"]).digest()).decode()
            try:
                try:
                    response = client.put_object(Bucket=bucket, Key=item["key"], Body=item["data"], ContentType=item["content_type"],
                                                 ServerSideEncryption="aws:kms", SSEKMSKeyId=tenant["kms_key_arn"], ChecksumSHA256=checksum,
                                                 IfNoneMatch="*")
                    state = "created"
                except ClientError as error:
                    code, status = _status(error)
                    if status != 412 and code != "PreconditionFailed":
                        raise
                    response, state = client.head_object(Bucket=bucket, Key=item["key"], ChecksumMode="ENABLED"), "unchanged"
                    require(response.get("ContentLength") == item["bytes"] and response.get("ChecksumSHA256") == checksum, "existing_object_differs")
            except (ClientError, BotoCoreError) as error:
                raise Refused("object_write_refused") from error
            require(response.get("ServerSideEncryption") == "aws:kms" and response.get("SSEKMSKeyId") == tenant["kms_key_arn"]
                    and response.get("ChecksumSHA256") == checksum, "object_encryption_refused")
            written.append({"name": item["name"], "key": item["key"], "sha256": item["sha256"], "bytes": item["bytes"], "state": state,
                            "version_id": response.get("VersionId"), "server_side_encryption": "aws:kms", "kms_key_arn": tenant["kms_key_arn"]})
        receipts.append(dict({k: v for k, v in plan.items() if k != "objects"}, bucket=bucket, objects=written))
    return receipts


def s3_client():
    """The task's own credentials (its task role); no endpoint, profile or role override."""
    env = os.environ
    require(not any(name.startswith("AWS_ENDPOINT_URL") or name in ("AWS_PROFILE", "AWS_ROLE_ARN", "AWS_WEB_IDENTITY_TOKEN_FILE")
                    for name in env), "aws_override_refused")
    region = env.get("AWS_REGION", "")
    require(re.fullmatch(r"[a-z]{2}(-[a-z]+)+-[0-9]", region) is not None, "region_refused")
    import boto3
    from botocore.config import Config
    return boto3.session.Session(region_name=region).client("s3", config=Config(
        connect_timeout=5, read_timeout=15, retries={"total_max_attempts": 2, "mode": "standard"},
        request_checksum_calculation="when_required", response_checksum_validation="when_required"))


def registration(plan, objects):
    """APP-012's register-source-revision input: exactly REGISTRATION_FIELDS.

    `object_digest` is the SHA-256 of the stored part.parquet object, which
    D7.10 verifies on fetch; under ADR-009 D9.3 (option a) it equals
    `normalized_digest`, and both are checked equal here.
    """
    stored = {item["name"]: item["sha256"] for item in objects}
    require(stored.get("original") == plan["content_digest"] and stored.get("part.parquet") == plan["normalized_digest"],
            "registration_refused")
    record = dict(plan, object_digest=stored["part.parquet"])
    return {name: record[name] for name in REGISTRATION_FIELDS}


def upload_sources(args):
    if args.plan:
        plans = plan_sources()
        return {"command": "upload-sources", "plan": True, "sources": public(plans),
                "registration": [registration(plan, plan["objects"]) for plan in plans]}
    bucket = os.environ.get("GENUI_EVIDENCE_BUCKET", "")
    require(BUCKET.fullmatch(bucket) is not None, "bucket_refused")
    tenants = load_tenants(os.environ.get("GENUI_OPERATOR_TENANTS_JSON"))
    client = s3_client()
    receipts = upload(client, bucket, tenants, plan_sources())
    # The closed table has one source: one record, printed only after both objects are stored.
    require(len(receipts) == 1, "sources_refused")
    return registration(receipts[0], receipts[0]["objects"])


def admission(rest):
    """The same pre-connect checks as every database command (review m1), and no
    secret-printing or key-holding subcommand inside a managed task (review M3)."""
    import admission_operator
    try:
        # admission_operator's own parser decides the subcommand; an argv it
        # refuses never runs (it refuses again, before connecting).
        command = admission_operator.parser().parse_args(rest).command
    except admission_operator.Refused:
        command = None
    require(not (managed_task() and command in MANAGED_REFUSED), "refused_in_managed_task")
    if command not in (None, "defaults"):
        database_environment()
    return admission_operator.main(rest)


def main(argv=None):
    argv = sys.argv[1:] if argv is None else argv
    try:
        args = parse(argv)
        if args.command == "admission":
            # admission_operator prints its own JSON result or fixed refusal.
            return admission(args.rest)
        result = {"migrate": lambda: migrate(), "verify-catalog": lambda: verify_catalog(), "seed": lambda: seed(),
                  "upload-sources": lambda: upload_sources(args)}[args.command]()
    except Refused as refusal:
        print(json.dumps({"refused": str(refusal)}), file=sys.stderr)
        return 1
    except Exception:
        print(json.dumps({"refused": "operator_refused"}), file=sys.stderr)
        return 1
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
