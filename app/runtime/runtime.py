"""Credential delivery and managed probes, never an HTTP/business API.

Cloud operations occur only when explicitly invoking serve/refresh/probe/migrate.
Local unit tests do not run them and cannot qualify a managed environment.
"""
import argparse
import hashlib
import json
import os
import re
from pathlib import Path
import signal
import stat
import subprocess
import sys
import tempfile
import threading
import time
import uuid

ROLES = ("workspace", "control", "data", "worker")
# The image's schema-version pin. The production entry points (migrate, probe)
# refuse a manifest naming any other version before connecting; runner and
# probe comparisons then use the pinned manifest itself.
# scripts/check-schema-pins.py keeps this value, the manifest, the native
# version check and both embedded catalogs on the same version.
DATABASE_SCHEMA_VERSION = 3
# Byte bound of the canonical catalog projection and of each catalog file
# (ADR-007 D7.12, moved once in 0003's slice). It must equal catalog-query.json
# max_output_bytes and its SQL octet_length literal and the native BOUND;
# scripts/check-schema-pins.py checks that agreement group.
CATALOG_BOUND = 524288
# The version whose upgrade created genui_meta.schema_migrations. A new ledger
# always starts at its historical BETWEEN 1 AND 2 bound; every later migration
# ALTERs both version CHECKs to its own number (ADR-007 D7.12).
LEDGER_VERSION = 2
INITIAL_MIGRATION_SHA256 = "f98f67b7b516483692e5873741080df8ebf023dd9af9872f958258e98e3bd963"
QUALIFICATION_JSON_ENV = "GENUI_AUTH_QUALIFICATION_CONFIG_JSON"
QUALIFICATION_FILE_ENV = "GENUI_AUTH_QUALIFICATION_CONFIG_FILE"
IDENTITY_REGISTRY_FILE_ENV = "GENUI_IDENTITY_REGISTRY_FILE"


class Refused(ValueError):
    pass


def require(condition):
    if not condition:
        raise Refused("runtime configuration refused")


def take_qualification_json(mode):
    """Remove optional injected bytes before any operation/child can inherit them.

    Only UTF-8 transport/size is checked here. Rust owns JSON/schema/expiry and
    authentication policy. No exception or logging may include the raw value.
    """
    if QUALIFICATION_JSON_ENV not in os.environ:
        return None
    raw = os.environ.pop(QUALIFICATION_JSON_ENV)
    require(mode == "serve" and QUALIFICATION_FILE_ENV not in os.environ)
    require(0 < len(raw) <= 8192 and raw.strip())
    try:
        payload = raw.encode("utf-8", errors="strict")
    except UnicodeError:
        raise Refused("runtime configuration refused") from None
    require(0 < len(payload) <= 8192)
    return payload


class QualificationFile:
    """One newly created secret file; never replace or clean someone else's file."""
    PREFIX = ".auth-qualification-"
    def __init__(self, runtime_path, payload):
        self.directory_fd = None
        self.file_fd = None
        self.identity = None
        self.name = None
        path = Path(runtime_path)
        require(path.is_absolute())
        self.directory = path.parent
        descriptor = None
        try:
            # Require the existing private volume. Do not follow its final
            # directory component or create a replacement on injection failure.
            self.directory_fd = os.open(self.directory, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
            self.directory_identity = os.fstat(self.directory_fd)
            self.check_directory()
            self.name = self.PREFIX + uuid.uuid4().hex + ".json"
            descriptor = os.open(self.name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC,
                                 0o600, dir_fd=self.directory_fd)
            # Record ownership immediately: even a later write/fsync failure
            # cleans only this inode, never an existing path or replacement.
            self.identity = os.fstat(descriptor)
            # Keep the inode alive through cleanup so unlink/replacement cannot
            # recycle its number and fool the ownership comparison. This fd is
            # non-inheritable and is never passed to the child.
            self.file_fd = os.dup(descriptor)
            os.set_inheritable(self.file_fd, False)
            os.fchmod(descriptor, 0o600)
            with os.fdopen(descriptor, "wb") as stream:
                descriptor = None
                stream.write(payload)
                stream.flush()
                os.fsync(stream.fileno())
            os.fsync(self.directory_fd)
            self.check_file()
        except BaseException:
            if descriptor is not None:
                os.close(descriptor)
            self.close()
            raise

    @property
    def path(self):
        return self.directory / self.name

    def check_directory(self):
        opened = os.fstat(self.directory_fd)
        named = self.directory.lstat()
        require(stat.S_ISDIR(opened.st_mode) and opened.st_uid == os.geteuid()
                and stat.S_IMODE(opened.st_mode) == 0o700
                and stat.S_ISDIR(named.st_mode)
                and (opened.st_dev, opened.st_ino) == (named.st_dev, named.st_ino)
                and (opened.st_dev, opened.st_ino) == (self.directory_identity.st_dev, self.directory_identity.st_ino))

    def check_file(self):
        self.check_directory()
        current = os.stat(self.name, dir_fd=self.directory_fd, follow_symlinks=False)
        require(stat.S_ISREG(current.st_mode) and current.st_uid == os.geteuid()
                and stat.S_IMODE(current.st_mode) == 0o600 and current.st_nlink == 1
                and (current.st_dev, current.st_ino) == (self.identity.st_dev, self.identity.st_ino))

    def close(self):
        if self.directory_fd is None:
            return
        try:
            if self.identity is not None:
                try:
                    current = os.stat(self.name, dir_fd=self.directory_fd, follow_symlinks=False)
                except FileNotFoundError:
                    return
                require(stat.S_ISREG(current.st_mode) and current.st_uid == os.geteuid()
                        and current.st_nlink == 1
                        and (current.st_dev, current.st_ino) == (self.identity.st_dev, self.identity.st_ino))
                os.unlink(self.name, dir_fd=self.directory_fd)
                os.fsync(self.directory_fd)
        finally:
            try:
                if self.file_fd is not None:
                    os.close(self.file_fd)
                    self.file_fd = None
            finally:
                os.close(self.directory_fd)
                self.directory_fd = None


class EmptyIdentityRegistryFile(QualificationFile):
    """Private deny-all registry using the same owned-inode file lifecycle."""
    PREFIX = ".identity-registry-"

    def __init__(self, runtime_path):
        payload = json.dumps({"schema_version": 1, "registry_version": str(uuid.uuid4()), "bindings": []},
                             separators=(",", ":")).encode("utf-8")
        super().__init__(runtime_path, payload)


def load(path, now=None):
    now = int(time.time()) if now is None else now
    path = Path(path)
    parent = path.parent.lstat()
    require(stat.S_ISDIR(parent.st_mode) and parent.st_uid == os.geteuid() and parent.st_mode & 0o077 == 0)
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, "rb") as stream:
        metadata = os.fstat(stream.fileno())
        require(stat.S_ISREG(metadata.st_mode) and metadata.st_uid == os.geteuid() and metadata.st_mode & 0o077 == 0)
        require(metadata.st_size <= 65536)
        raw = stream.read(65537)
        require(len(raw) <= 65536)
        value = json.loads(raw)
        value["_file_sha256"] = hashlib.sha256(raw).hexdigest()
    require(value["schema_version"] == 1 and str(uuid.UUID(value["generation"])) == value["generation"])
    require(value["issued_at"] <= now + 5 and now < value["expires_at"] <= value["issued_at"] + 600)
    db = value["database"]
    require(db["name"] == "genui" and db["port"] == 5432)
    require(db["host"] and all(c.isascii() and (c.isalnum() or c in '.-') for c in db["host"]))
    require(set(db["roles"]) == set(ROLES))
    tokens = set()
    for name in ROLES:
        role = db["roles"][name]
        require(role["username"] == "genui_" + name and role["token"] and role["expires_at"] >= value["expires_at"])
        tokens.add(role["token"])
    require(len(tokens) == 4)
    storage = value["storage"]
    require(storage["region"] == "us-east-1" and storage["bucket"])
    tenants = storage["tenants"]
    require(len(tenants) == 2 and len({x["organization_id"] for x in tenants}) == 2)
    require(len({x["kms_key_arn"] for x in tenants}) == 2 and len({x["role_arn"] for x in tenants}) == 2)
    for tenant in tenants:
        require(str(uuid.UUID(tenant["organization_id"])) == tenant["organization_id"])
        require(tenant["prefix"] == "o/" + tenant["organization_id"] + "/")
        require(tenant["expires_at"] >= value["expires_at"])
        require(all(tenant[k] for k in ("access_key_id", "secret_access_key", "session_token")))
    return value


def atomic_write(path, value):
    path = Path(path)
    path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    parent = path.parent.lstat()
    require(stat.S_ISDIR(parent.st_mode) and parent.st_uid == os.geteuid() and parent.st_mode & 0o077 == 0)
    fd, temporary = tempfile.mkstemp(prefix=".credentials-", dir=path.parent)
    try:
        with os.fdopen(fd, "w") as stream:
            os.fchmod(stream.fileno(), 0o600)
            json.dump(value, stream, separators=(",", ":"))
            stream.flush(); os.fsync(stream.fileno())
        os.replace(temporary, path)
        directory = os.open(path.parent, os.O_RDONLY)
        try: os.fsync(directory)
        finally: os.close(directory)
    finally:
        if os.path.exists(temporary): os.unlink(temporary)


def sdk_config():
    from botocore.config import Config
    return Config(connect_timeout=2, read_timeout=2, retries={"total_max_attempts": 1, "mode": "standard"})


def scoped_session(credentials, region):
    import boto3
    return boto3.Session(aws_access_key_id=credentials["AccessKeyId"], aws_secret_access_key=credentials["SecretAccessKey"],
                         aws_session_token=credentials["SessionToken"], region_name=region)


def refresh(path):
    Path(path).parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    import boto3
    region = os.environ["AWS_REGION"]
    require(region == "us-east-1")
    host = os.environ["GENUI_DATABASE_HOST"]
    require(os.environ["GENUI_DATABASE_NAME"] == "genui" and host.endswith(".us-east-1.rds.amazonaws.com") and re.fullmatch(r"[a-z0-9.-]+", host))
    db_roles = json.loads(os.environ["GENUI_DATABASE_ROLES_JSON"])
    require(set(db_roles) == set(ROLES) and len(set(db_roles.values())) == 4)
    require(all(re.fullmatch(r"arn:aws:iam::877573132739:role/[A-Za-z0-9+=,.@_/-]+", arn) for arn in db_roles.values()))
    tenants = json.loads(os.environ["GENUI_TENANTS_JSON"])
    require(len(tenants) == 2 and len({t["organization_id"] for t in tenants}) == 2 and len({t["kms_key_arn"] for t in tenants}) == 2)
    require(re.fullmatch(r"[a-z0-9][a-z0-9.-]{1,61}[a-z0-9]", os.environ["GENUI_EVIDENCE_BUCKET"]))
    for tenant in tenants:
        require(set(tenant) == {"organization_id", "role_arn", "kms_key_arn", "prefix"})
        require(tenant["prefix"] == "o/" + str(uuid.UUID(tenant["organization_id"])) + "/")
        require(re.fullmatch(r"arn:aws:iam::877573132739:role/[A-Za-z0-9+=,.@_/-]+", tenant["role_arn"]))
        require(re.fullmatch(r"arn:aws:kms:us-east-1:877573132739:key/[a-f0-9-]{36}", tenant["kms_key_arn"]))
    session = boto3.Session(region_name=region)
    sts = session.client("sts", config=sdk_config())
    issued = int(time.time())
    value = {"schema_version": 1, "generation": str(uuid.uuid4()), "issued_at": issued, "expires_at": issued + 600,
             "database": {"host": host, "port": 5432, "name": "genui", "roles": {}},
             "storage": {"region": region, "bucket": os.environ["GENUI_EVIDENCE_BUCKET"], "tenants": []}}
    for name in ROLES:
        credentials = sts.assume_role(RoleArn=db_roles[name], RoleSessionName="genui-db-" + name, DurationSeconds=900)["Credentials"]
        expires = min(issued + 600, int(credentials["Expiration"].timestamp()) - 60)
        token = scoped_session(credentials, region).client("rds", config=sdk_config()).generate_db_auth_token(
            DBHostname=host, Port=5432, DBUsername="genui_" + name, Region=region)
        value["database"]["roles"][name] = {"username": "genui_" + name, "token": token, "expires_at": expires}
        value["expires_at"] = min(value["expires_at"], expires)
    for tenant in tenants:
        require(set(tenant) == {"organization_id", "role_arn", "kms_key_arn", "prefix"})
        credentials = sts.assume_role(RoleArn=tenant["role_arn"], RoleSessionName="genui-storage-" + tenant["organization_id"][-8:], DurationSeconds=900)["Credentials"]
        expires = int(credentials["Expiration"].timestamp()) - 60
        value["storage"]["tenants"].append({**tenant, "access_key_id": credentials["AccessKeyId"],
            "secret_access_key": credentials["SecretAccessKey"], "session_token": credentials["SessionToken"], "expires_at": expires})
        value["expires_at"] = min(value["expires_at"], expires)
    # Validate through the same private-file reader before publishing a generation.
    with tempfile.TemporaryDirectory(prefix=".validate-", dir=Path(path).parent) as directory:
        candidate = Path(directory) / "credentials.json"
        atomic_write(candidate, value); load(candidate)
    atomic_write(path, value)


def migration_directory():
    directory = Path(__file__).parent / "migrations"
    return directory if directory.exists() else Path(__file__).parent.parent / "migrations"


def probe_database(connection, username):
    """Catalog proof precedes every application metadata/version read."""
    require(username in tuple("genui_" + name for name in ROLES))
    directory = migration_directory()
    verify_schema_catalog(connection, directory)
    latest = migration_manifest(directory)["database_version"]
    row = connection.execute("""SELECT session_user::pg_catalog.text,current_user::pg_catalog.text,pg_catalog.current_database(),
      r.rolsuper,r.rolcreatedb,r.rolcreaterole,r.rolbypassrls,r.rolcanlogin,
      pg_catalog.pg_has_role(current_user,'genui_owner','MEMBER'),pg_catalog.pg_has_role(current_user,'rds_iam','MEMBER'),
      (SELECT ssl FROM pg_catalog.pg_stat_ssl WHERE pid=pg_catalog.pg_backend_pid()),
      (SELECT version FROM genui_meta.schema_version WHERE singleton=true)
      FROM pg_catalog.pg_roles r WHERE r.rolname=current_user""").fetchone()
    require(row == (username,username,"genui",False,False,False,False,True,False,True,True,latest))
    memberships = connection.execute("""SELECT r.rolname::pg_catalog.text FROM pg_catalog.pg_roles r
      WHERE r.rolname NOT IN (current_user,'rds_iam')
      AND pg_catalog.pg_has_role(current_user,r.oid,'MEMBER') ORDER BY r.rolname""").fetchall()
    require(memberships == [])


def probe(path, generation):
    import psycopg
    value = load(path)
    require(value["generation"] == generation)
    require(migration_manifest(migration_directory())["database_version"] == DATABASE_SCHEMA_VERSION)
    db = value["database"]
    for name in ROLES:
        role = db["roles"][name]
        with psycopg.connect(host=db["host"], port=5432, dbname="genui", user=role["username"], password=role["token"],
                             sslmode="verify-full", sslrootcert=os.environ["GENUI_RDS_CA_FILE"], connect_timeout=3,
                             options="-c search_path=pg_catalog,pg_temp -c default_transaction_read_only=on -c statement_timeout=3000") as connection:
            probe_database(connection, role["username"])
    for tenant in value["storage"]["tenants"]:
        session = scoped_session({"AccessKeyId": tenant["access_key_id"], "SecretAccessKey": tenant["secret_access_key"], "SessionToken": tenant["session_token"]}, value["storage"]["region"])
        session.client("s3", config=sdk_config()).list_objects_v2(Bucket=value["storage"]["bucket"], Prefix=tenant["prefix"], MaxKeys=1)
        client = session.client("kms", config=sdk_config())
        context = {"organization_id": tenant["organization_id"]}
        ciphertext = client.encrypt(KeyId=tenant["kms_key_arn"], Plaintext=b"genui-foundation-readiness-v1", EncryptionContext=context)["CiphertextBlob"]
        require(client.decrypt(KeyId=tenant["kms_key_arn"], CiphertextBlob=ciphertext, EncryptionContext=context)["Plaintext"] == b"genui-foundation-readiness-v1")
    # A refresh/revocation during probes cannot certify the previous generation.
    final = load(path)
    require(final["generation"] == generation and final["expires_at"] == value["expires_at"] and final["_file_sha256"] == value["_file_sha256"])
    return {"generation": generation, "file_sha256": value["_file_sha256"], "expires_at": value["expires_at"], "checked_at": int(time.time())}


MIGRATION_FILE = re.compile(r"([0-9]{4})_[a-z0-9]+(?:_[a-z0-9]+)*\.sql")


def migration_manifest(directory):
    """Exact manifest shape; the latest version N is learned only from it."""
    manifest = json.loads((Path(directory) / "manifest.json").read_text())
    require(isinstance(manifest, dict) and set(manifest) == {"schema_version", "database_version", "migrations", "catalog"})
    latest = manifest["database_version"]
    # LEDGER_VERSION introduced the ledger; four digits number every file.
    require(manifest["schema_version"] == 1 and type(latest) is int and LEDGER_VERSION <= latest <= 9999)
    require(isinstance(manifest["migrations"], list) and len(manifest["migrations"]) == latest)
    require(isinstance(manifest["catalog"], list) and len(manifest["catalog"]) == latest + 1)
    return manifest


def migration_sources(directory):
    """Only the manifest's N reviewed image-local migrations; no caller SQL or downgrade."""
    directory = Path(directory)
    manifest = migration_manifest(directory)
    result = []
    for version, item in enumerate(manifest["migrations"], 1):
        require(isinstance(item, dict) and set(item) == {"version", "file", "sha256"} and item["version"] == version)
        named = MIGRATION_FILE.fullmatch(item["file"]) if isinstance(item["file"], str) else None
        require(named is not None and int(named.group(1)) == version)
        raw = (directory / item["file"]).read_bytes()
        require(0 < len(raw) <= 262144 and hashlib.sha256(raw).hexdigest() == item["sha256"])
        if version == 1: require(item["sha256"] == INITIAL_MIGRATION_SHA256)
        result.append((version, item["sha256"], raw.decode("utf-8")))
    require({p.name for p in directory.glob("*.sql")} == {item["file"] for item in manifest["migrations"]})
    catalog_resources(directory)
    return result


def catalog_resources(directory):
    """Image-pinned, readable logical catalogs; never learn expected state live."""
    directory = Path(directory)
    manifest = migration_manifest(directory)
    names = ("catalog-query.json", *("catalog-v%d.json" % version for version in range(1, manifest["database_version"] + 1)))
    documents = []
    for item, name in zip(manifest["catalog"], names):
        require(isinstance(item, dict) and set(item) == {"file", "sha256"} and item["file"] == name)
        raw = (directory / name).read_bytes()
        require(0 < len(raw) <= CATALOG_BOUND and hashlib.sha256(raw).hexdigest() == item["sha256"])
        documents.append(json.loads(raw))
    query = documents[0]
    require(set(query) == {"schema_version", "search_path", "max_output_bytes", "section_row_limit", "sql"})
    require(query["schema_version"] == 1 and query["search_path"] == "pg_catalog, pg_temp"
            and query["max_output_bytes"] == CATALOG_BOUND and query["section_row_limit"] == 1025
            and isinstance(query["sql"], str) and 0 < len(query["sql"].encode()) <= 65536)
    for expected in documents[1:]:
        require(isinstance(expected, dict) and expected and all(isinstance(rows, list) and len(rows) < 1025 for rows in expected.values()))
    require(all(set(expected) == set(documents[1]) for expected in documents[2:]))
    return query, dict(enumerate(documents[1:], 1))


def verify_schema_catalog(connection, directory, version=None):
    """Same app-readable projection is embedded by native admission/readiness.

    Caller owns a transaction; catalog-first deparsing is transaction-local.
    Compare exact logical facts before reading untrusted metadata relations.
    Without a version, the manifest's latest version is required.
    """
    return proven_catalog_version(connection, directory, None if version is None else (version,))


def proven_catalog_version(connection, directory, versions=None):
    """The one candidate version whose reviewed catalog exactly matches; default latest."""
    require(not connection.autocommit)
    require(connection.execute("SHOW session_replication_role").fetchone() == ("origin",))
    query, expected = catalog_resources(directory)
    versions = (max(expected),) if versions is None else tuple(versions)
    require(versions and all(version in expected for version in versions))
    # An omitted pg_temp is implicitly searched first for relations/types.
    # List it explicitly last, including on operator connections with TEMP.
    connection.execute("SET LOCAL search_path=pg_catalog, pg_temp")
    rows = connection.execute(query["sql"]).fetchall()
    require(len(rows) == 1 and isinstance(rows[0][0], str) and 0 < len(rows[0][0].encode()) <= query["max_output_bytes"])
    actual = json.loads(rows[0][0])
    matches = [version for version in versions if actual == expected[version]]
    require(len(matches) == 1)
    return matches[0]


def initialize_migration_history(connection, initial_digest):
    """Only after the verified legacy catalog, in the same upgrade transaction.

    Both version CHECKs start at the historical BETWEEN 1 AND LEDGER_VERSION
    bound for any manifest; later migrations ALTER them (ADR-007 D7.12).
    """
    connection.execute("""SET LOCAL ROLE genui_owner;
      CREATE TABLE genui_meta.schema_migrations (
        version integer PRIMARY KEY CHECK (version BETWEEN 1 AND %d),
        sha256 text NOT NULL CHECK (sha256 ~ '^[0-9a-f]{64}$'), applied_at timestamptz NOT NULL DEFAULT clock_timestamp());
      ALTER TABLE genui_meta.schema_version DROP CONSTRAINT schema_version_version_check;
      ALTER TABLE genui_meta.schema_version ADD CONSTRAINT schema_version_version_check CHECK (version BETWEEN 1 AND %d);
      RESET ROLE;""" % (LEDGER_VERSION, LEDGER_VERSION))
    connection.execute("INSERT INTO genui_meta.schema_migrations(version,sha256) VALUES (1,%s)", (initial_digest,))


def apply_migrations(connection, directory):
    """Caller owns one transaction; errors roll it all back, including history."""
    require(not connection.autocommit)
    # Replica mode suppresses ordinary FK triggers even when their catalog
    # enablement flags still look correct. Refuse before any migration writes.
    require(connection.execute("SHOW session_replication_role").fetchone() == ("origin",))
    # Database defaults may explicitly put an untrusted schema before pg_catalog.
    # Fix this transaction's lookup path before identity/advisory/metadata reads.
    connection.execute("SET LOCAL search_path=pg_catalog, pg_temp")
    sources = migration_sources(directory)
    latest = len(sources)
    require(connection.execute("SELECT session_user::text,current_user::text,current_database()").fetchone()
            == ("genui_migration", "genui_migration", "genui"))
    connection.execute("SELECT pg_advisory_xact_lock(1819409017, 2)")
    exists = connection.execute("SELECT to_regclass('genui_meta.schema_version') IS NOT NULL").fetchone()[0]
    if not exists:
        require(connection.execute("SELECT count(*) FROM pg_namespace WHERE nspname IN ('genui_meta','genui','genui_private')").fetchone() == (0,))
        connection.execute(sources[0][2])
    has_history = connection.execute("SELECT to_regclass('genui_meta.schema_migrations') IS NOT NULL").fetchone()[0]
    if has_history:
        # A committed history1 cannot be produced by this atomic runner. Never
        # treat a partial/forged ledger as permission to skip the legacy proof.
        # Versions LEDGER_VERSION..N carry the ledger: the exact reviewed catalog
        # proves the recorded version before the ledger or version row is read.
        recorded = proven_catalog_version(connection, directory, range(LEDGER_VERSION, latest + 1))
        require(connection.execute("SELECT version,sha256 FROM genui_meta.schema_migrations ORDER BY version").fetchall()
                == [(version, digest) for version, digest, _ in sources[:recorded]])
        require(connection.execute("SELECT singleton,version FROM genui_meta.schema_version").fetchall() == [(True, recorded)])
        if recorded == latest:
            return
    else:
        verify_schema_catalog(connection, directory, 1)
        require(connection.execute("SELECT singleton,version FROM genui_meta.schema_version").fetchall() == [(True, 1)])
        initialize_migration_history(connection, sources[0][1])
        recorded = 1
    for version, digest, source in sources[recorded:]:
        connection.execute(source)
        connection.execute("INSERT INTO genui_meta.schema_migrations(version,sha256) VALUES (%s,%s)", (version, digest))
        connection.execute("UPDATE genui_meta.schema_version SET version=%s WHERE singleton", (version,))
        # Every step, on every path, must reach its reviewed catalog before the
        # next runs: the ledger starts at its historical bound and each later
        # migration ALTERs both CHECKs, so migration k leaves catalog-vk. The
        # last step's proof is the final one below.
        if version < latest:
            verify_schema_catalog(connection, directory, version)
    require(connection.execute("SELECT version FROM genui_meta.schema_version").fetchone() == (latest,))
    verify_schema_catalog(connection, directory, latest)


def migrate():
    import psycopg
    require(os.environ["PGDATABASE"] == "genui" and os.environ["PGUSER"] == "genui_migration")
    require(migration_manifest(migration_directory())["database_version"] == DATABASE_SCHEMA_VERSION)
    with psycopg.connect(host=os.environ["PGHOST"], port=5432, dbname="genui", user="genui_migration", password=os.environ["PGPASSWORD"],
                         sslmode="verify-full", sslrootcert=os.environ["PGSSLROOTCERT"], connect_timeout=5,
                         options="-c statement_timeout=30000 -c lock_timeout=5000") as connection:
        apply_migrations(connection, migration_directory())
    print("tenant_foundation_migration_complete")


def qualification(path):
    """Operator-invoked, synthetic writes; never part of readiness or startup."""
    from botocore.exceptions import ClientError
    require(os.environ.get("GENUI_QUALIFY_SYNTHETIC") == "APP-007")
    refresh(path)
    value = load(path)
    report = {"schema_version": 1, "scope": "APP-007 synthetic managed qualification", "run_id": str(uuid.uuid4()),
              "passed": False, "checks": [], "cleanup": []}
    def denied(name, action, unexpected=None):
        try: result = action()
        except ClientError as error:
            require(error.response["Error"]["Code"] in ("AccessDenied", "AccessDeniedException", "KMS.AccessDeniedException"))
            report["checks"].append({"name": name, "denied": True})
        else:
            if unexpected: unexpected(result)
            if isinstance(result, dict) and "Body" in result: result["Body"].close()
            raise Refused("cross-tenant operation was not denied")
    try:
        probe(path, value["generation"])
        report["checks"].append({"name": "four distinct TLS database identities and migration", "passed": True})
        tenants = value["storage"]["tenants"]
        clients = []
        bucket = value["storage"]["bucket"]
        for tenant in tenants:
            session = scoped_session({"AccessKeyId": tenant["access_key_id"], "SecretAccessKey": tenant["secret_access_key"], "SessionToken": tenant["session_token"]}, value["storage"]["region"])
            clients.append((session.client("s3", config=sdk_config()), session.client("kms", config=sdk_config())))
            key = tenant["prefix"] + "_qualification/" + report["run_id"] + "/probe.bin"
            body = ("synthetic:" + report["run_id"]).encode()
            response = clients[-1][0].put_object(Bucket=bucket, Key=key, Body=body, ServerSideEncryption="aws:kms", SSEKMSKeyId=tenant["kms_key_arn"])
            require(response.get("VersionId") and response["VersionId"] != "null")
            report["cleanup"].append({"Bucket": bucket, "Key": key, "VersionId": response["VersionId"]})
            object_result = clients[-1][0].get_object(**report["cleanup"][-1])
            require(object_result.get("ServerSideEncryption") == "aws:kms" and object_result.get("SSEKMSKeyId") == tenant["kms_key_arn"])
            read = object_result["Body"]
            try: require(read.read(len(body) + 1) == body)
            finally: read.close()
            report["checks"].append({"name": "own encrypted object write/read", "organization_id": tenant["organization_id"], "passed": True})
        for i, tenant in enumerate(tenants):
            other = tenants[1 - i]
            s3, kms = clients[i]
            denied("other tenant existing object", lambda: s3.get_object(**report["cleanup"][1 - i]))
            denied("other tenant listing", lambda: s3.list_objects_v2(Bucket=bucket, Prefix=other["prefix"], MaxKeys=1))
            denied("wrong tenant key on own prefix", lambda: s3.put_object(Bucket=bucket, Key=report["cleanup"][i]["Key"], Body=b"synthetic", ServerSideEncryption="aws:kms", SSEKMSKeyId=other["kms_key_arn"]), lambda result: report["cleanup"].append({"Bucket": bucket, "Key": report["cleanup"][i]["Key"], "VersionId": result["VersionId"]}))
            denied("other tenant KMS key", lambda: kms.encrypt(KeyId=other["kms_key_arn"], Plaintext=b"synthetic", EncryptionContext={"organization_id": other["organization_id"]}))
        final = load(path)
        require(final["_file_sha256"] == value["_file_sha256"])
        report["passed"] = True
    finally:
        # Only synthetic object identities and verdicts leave the process. The
        # operator uses these exact versions for cleanup, even after a failure.
        print(json.dumps(report), flush=True)


def cleanup(report_path):
    """Separate operator identity; application roles cannot delete revisions."""
    import boto3
    require(os.environ.get("GENUI_QUALIFY_SYNTHETIC") == "APP-007")
    report = json.loads(Path(report_path).read_text())
    require(report["schema_version"] == 1 and report["scope"] == "APP-007 synthetic managed qualification")
    run_id = str(uuid.UUID(report["run_id"]))
    bucket = os.environ["GENUI_EVIDENCE_BUCKET"]
    tenants = json.loads(os.environ["GENUI_TENANTS_JSON"])
    permitted = {"o/" + str(uuid.UUID(t["organization_id"])) + "/_qualification/" + run_id + "/probe.bin" for t in tenants}
    require(0 <= len(report["cleanup"]) <= 4)
    for entry in report["cleanup"]:
        require(set(entry) == {"Bucket", "Key", "VersionId"} and entry["Bucket"] == bucket and entry["Key"] in permitted and entry["VersionId"])
    client = boto3.Session(region_name="us-east-1").client("s3", config=sdk_config())
    for entry in report["cleanup"]: client.delete_object(**entry)
    print(json.dumps({"run_id": run_id, "deleted_versions": len(report["cleanup"])}))


def serve(path):
    injected = take_qualification_json("serve")
    stop = threading.Event()
    for sig in (signal.SIGTERM, signal.SIGINT): signal.signal(sig, lambda *_: stop.set())
    child = None
    qualification_file = None
    registry_file = None
    credentials_started = False
    try:
        if injected is not None:
            qualification_file = QualificationFile(path, injected)
        injected = None
        Path(path).parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        child_environment = os.environ.copy()
        if IDENTITY_REGISTRY_FILE_ENV not in child_environment:
            registry_file = EmptyIdentityRegistryFile(path)
            child_environment[IDENTITY_REGISTRY_FILE_ENV] = str(registry_file.path)
        # Always omit the raw injected value; the file path is the only handoff.
        child_environment.pop(QUALIFICATION_JSON_ENV, None)
        if qualification_file is not None:
            child_environment[QUALIFICATION_FILE_ENV] = str(qualification_file.path)
        while not stop.is_set():
            try:
                credentials_started = True
                refresh(path)
            except Exception:
                # SDK/driver errors can contain tokens/connection strings. No raw errors.
                print("credential_refresh_unavailable", file=sys.stderr, flush=True)
            else:
                if child is None:
                    # Unsafe/replaced handoff state is a terminal refusal, not
                    # a transient managed-dependency retry.
                    if qualification_file is not None:
                        qualification_file.check_file()
                    if registry_file is not None:
                        registry_file.check_file()
                    try:
                        child = subprocess.Popen([os.environ.get("GENUI_BINARY", "/app/genui")], env=child_environment)
                    except Exception:
                        print("credential_refresh_unavailable", file=sys.stderr, flush=True)
            for _ in range(240 if child else 5):
                if child and child.poll() is not None: return child.returncode
                if stop.wait(1): break
    finally:
        try:
            if child and child.poll() is None:
                child.terminate()
                try: child.wait(timeout=5)
                except subprocess.TimeoutExpired: child.kill(); child.wait(timeout=5)
        finally:
            try:
                if qualification_file is not None:
                    qualification_file.close()
            finally:
                try:
                    if registry_file is not None:
                        registry_file.close()
                finally:
                    if credentials_started:
                        Path(path).unlink(missing_ok=True)
    return 0


if __name__ == "__main__":
    parser = argparse.ArgumentParser(); parser.add_argument("mode", choices=("refresh", "probe", "serve", "migrate", "qualify", "cleanup")); parser.add_argument("--generation"); parser.add_argument("--report")
    args = parser.parse_args()
    try:
        # Probe helpers launched by Rust inherit only the path and ignore it.
        # A raw injection in any non-serve mode is removed and refused before
        # credential refresh, managed probes, migration or qualification runs.
        if args.mode != "serve": take_qualification_json(args.mode)
        path = os.environ.get("GENUI_RUNTIME_CONFIG_FILE", "/run/genui/credentials.json")
        if args.mode == "refresh": refresh(path)
        elif args.mode == "probe": print(json.dumps(probe(path, args.generation)))
        elif args.mode == "migrate": migrate()
        elif args.mode == "qualify": qualification(path)
        elif args.mode == "cleanup": cleanup(args.report)
        else: raise SystemExit(serve(path))
    except Exception:
        print("managed_dependency_operation_refused", file=sys.stderr)
        raise SystemExit(1)
