"""Credential delivery and managed probes, never an HTTP/business API.

Cloud operations occur only when explicitly invoking serve/refresh/probe/migrate
or `cache`. Local unit tests do not run them and cannot qualify a managed
environment.

Two engine-side modes exist for APP-012 (ADR-007 D7.10), both spawned by the
Rust binary per request and never by a member: `query` is the confined DuckDB
engine child (`query_engine.py`: empty environment, its three pipes, one JSON
request on stdin and one JSON line on stdout; it holds no credential and reads
only one cached revision of one organization), and `cache` is the separate
credentialed fetch that copies one source revision's Parquet object from the
organization's own prefix with that organization's scoped storage credentials,
verifies its `normalized_digest` and renames it atomically into the read-only
revision cache. The two never run in one process.
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
DATABASE_SCHEMA_VERSION = 13
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


def refuse_qualification_injection():
    """ADR-007 D7.13 (APP-009 review m6): the operator qualification harness is
    not part of the product image, so its configuration is refused in every
    mode before any operation runs or any child is started. The raw injected
    value is removed from this process first, so nothing can inherit it; no
    exception or log line includes it.
    """
    injected = os.environ.pop(QUALIFICATION_JSON_ENV, None) is not None
    require(not injected and QUALIFICATION_FILE_ENV not in os.environ)


class QualificationFile:
    """One newly created private file; never replace or clean someone else's file.
    Used for the deny-all identity registry (the harness injection is retired)."""
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


# --- provider invitation capabilities (ADR-003 amendment 2026-10-30; APP-074 review M1) ---
#
# The operator writes every live capability of the environment into one Secrets
# Manager secret (provider_management.CapabilitySecret); ECS injects it into the
# service container only, as CAPABILITIES_JSON_ENV. serve() removes it from its
# own environment before anything else, validates it whole, writes each entry as
# a 0600 `{organization}.{invitation}.capability` file into a fresh 0700
# directory under the private /run/genui volume, and hands only that directory's
# path (CAPABILITY_DIR_ENV) to the service child, whose invitation entry
# (app/src/auth/invite.rs) reads it. The directory is removed when serve() ends.
# A new or removed capability reaches the service at its next task start.
CAPABILITIES_JSON_ENV = "GENUI_PROVIDER_CAPABILITIES_JSON"
CAPABILITY_DIR_ENV = "GENUI_PROVIDER_CAPABILITY_DIR"
CAPABILITY_FORMAT = "forma:fauth-invite-capability:v1"
# Secrets Manager's SecretString bound; each record within the service's
# per-file bound (invite.rs CAPABILITY_LIMIT); at most this many per environment.
CAPABILITY_SECRET_BYTES_MAX = 65536
CAPABILITY_RECORD_BYTES_MAX = 4096
CAPABILITIES_MAX = 128
CAPABILITY_RECORD_KEYS = frozenset({"format", "organization_id", "invitation_id", "remote_invitation_id", "provider_expires_at",
                                    "invite_token"})
_CAPABILITY_UUID = r"[0-9a-f]{8}-[0-9a-f]{4}-[47][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}"
_CAPABILITY_ID = re.compile(r"(%s)\.(%s)" % (_CAPABILITY_UUID, _CAPABILITY_UUID))
_PROVIDER_ID = re.compile(r"tinv_[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")
_CAPABILITY_EXPIRY = re.compile(r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z")
_CODE_DIGEST = re.compile(r"sha256:[0-9a-f]{64}")


def capability_record_bytes(record):
    """One capability's file bytes, exactly as the service reads them."""
    return json.dumps(record, sort_keys=True, separators=(",", ":")).encode("utf-8")


def check_capability(capability_id, record):
    """One entry of the capability map: its id names the record's organization and invitation."""
    match = _CAPABILITY_ID.fullmatch(capability_id) if isinstance(capability_id, str) else None
    require(match is not None and isinstance(record, dict))
    require(set(record) in (CAPABILITY_RECORD_KEYS, CAPABILITY_RECORD_KEYS | {"code_digest"}))
    require(record["format"] == CAPABILITY_FORMAT and record["organization_id"] == match.group(1)
            and record["invitation_id"] == match.group(2))
    for name in ("remote_invitation_id", "invite_token"):
        require(isinstance(record[name], str) and _PROVIDER_ID.fullmatch(record[name]) is not None)
    require(record["remote_invitation_id"] != record["invite_token"])
    require(isinstance(record["provider_expires_at"], str) and _CAPABILITY_EXPIRY.fullmatch(record["provider_expires_at"]) is not None)
    require("code_digest" not in record
            or (isinstance(record["code_digest"], str) and _CODE_DIGEST.fullmatch(record["code_digest"]) is not None))
    require(len(capability_record_bytes(record)) <= CAPABILITY_RECORD_BYTES_MAX)
    return record


def _unique_object(pairs):
    require(len({key for key, _ in pairs}) == len(pairs))
    return dict(pairs)


def capability_map(text):
    """The whole secret, validated whole: a JSON object of at most CAPABILITIES_MAX entries keyed by
    `{organization}.{invitation}`. Any malformed entry refuses the whole map (never a partial set)."""
    require(isinstance(text, str) and len(text.encode("utf-8")) <= CAPABILITY_SECRET_BYTES_MAX)
    value = json.loads(text, object_pairs_hook=_unique_object)
    require(isinstance(value, dict) and len(value) <= CAPABILITIES_MAX)
    return {key: check_capability(key, record) for key, record in value.items()}


def write_capabilities(directory, text):
    """Write each entry of the capability map as a 0600 file into `directory`, an existing 0700
    directory of this user (never a link), refusing any existing name. Returns the names written."""
    capabilities = capability_map(text)
    directory_fd = os.open(directory, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
    written = []
    try:
        info = os.fstat(directory_fd)
        require(stat.S_ISDIR(info.st_mode) and info.st_uid == os.geteuid() and stat.S_IMODE(info.st_mode) == 0o700)
        for capability_id, record in sorted(capabilities.items()):
            name = capability_id + ".capability"
            descriptor = os.open(name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600,
                                 dir_fd=directory_fd)
            written.append(name)
            try:
                os.fchmod(descriptor, 0o600)
                os.write(descriptor, capability_record_bytes(record))
                os.fsync(descriptor)
            finally:
                os.close(descriptor)
        os.fsync(directory_fd)
    finally:
        os.close(directory_fd)
    return written


class CapabilityDirectory:
    """A fresh 0700 directory under the private runtime volume holding the injected capabilities;
    close() removes it and the capability files in it."""
    PREFIX = "provider-capabilities-"

    def __init__(self, runtime_path, text):
        parent = Path(runtime_path).parent
        info = parent.lstat()
        require(stat.S_ISDIR(info.st_mode) and info.st_uid == os.geteuid() and info.st_mode & 0o077 == 0)
        self.path = Path(tempfile.mkdtemp(prefix=self.PREFIX, dir=parent))
        try:
            os.chmod(self.path, 0o700)
            write_capabilities(str(self.path), text)
        except BaseException:
            self.close()
            raise

    def close(self):
        if self.path is None:
            return
        try:
            for name in os.listdir(self.path):
                if name.endswith(".capability"):
                    Path(self.path, name).unlink(missing_ok=True)
            self.path.rmdir()
        except FileNotFoundError:
            pass
        finally:
            self.path = None


def take_capability_injection():
    """Remove the injected capability secret from this process's environment (every mode), so no
    later child can inherit it. Only serve() uses the value."""
    return os.environ.pop(CAPABILITIES_JSON_ENV, None)


# --- hosted sign-in configuration (APP-078 B1; ADR-006 routing, ADR-007 D7.2) ---
#
# The service reads its provider routing (AUTH_CONFIG_FILE_ENV, app/src/auth/mod.rs)
# and the closed identity registry (IDENTITY_REGISTRY_FILE_ENV, app/src/storage/
# registry.rs) from private files, and the task's root filesystem is read-only, so
# the deployment delivers both through one Secrets Manager secret per environment
# (`AuthConfiguration` in infra/src/stack.ts), injected into the service container
# only as AUTH_CONFIGURATION_JSON_ENV. Nothing in it is secret beyond what ADR-006
# allows in a client configuration: the auth origin, the token issuer and audience,
# per-organization tenant slugs and public client ids, and the `ten_` -> organization
# bindings. serve() removes it from its own environment first, validates the whole
# document with the service's own bounds, writes `auth-config.json` and
# `identity-registry.json` as 0600 files into a fresh 0700 directory under the
# private /run/genui volume, and hands only those two paths to the service child.
# The initial value `{}` means "not configured": the child starts as before, with
# no routing and the private deny-all registry. A new value reaches the service at
# its next task start (force a new deployment after PutSecretValue).
AUTH_CONFIGURATION_JSON_ENV = "GENUI_AUTH_CONFIGURATION_JSON"
AUTH_CONFIG_FILE_ENV = "GENUI_AUTH_CONFIG_FILE"
AUTH_CONFIGURATION_FORMAT = "forma:genui-auth-configuration:v1"
AUTH_CONFIG_FILE_NAME = "auth-config.json"
IDENTITY_REGISTRY_FILE_NAME = "identity-registry.json"
# Secrets Manager's SecretString bound; each document within the service's own
# 16 KiB file limit (auth CONFIG_LIMIT, registry private_bytes); at most 32
# organizations and 32 bindings, as the service decodes.
AUTH_CONFIGURATION_BYTES_MAX = 65536
AUTH_DOCUMENT_BYTES_MAX = 16384
AUTH_ORGANIZATIONS_MAX = 32
AUTH_CONFIG_KEYS = frozenset({"schema_version", "auth_origin", "app_origin", "issuer", "audience",
                              "max_access_lifetime_seconds", "organizations"})
AUTH_ROUTE_KEYS = frozenset({"organization_id", "tenant_slug", "client_id"})
IDENTITY_REGISTRY_KEYS = frozenset({"schema_version", "registry_version", "bindings"})
IDENTITY_BINDING_KEYS = frozenset({"issuer", "provider_tenant_id", "organization_id"})
_TENANT_SLUG = re.compile(r"[a-z0-9-]{1,64}")
_CLIENT_ID = re.compile(r"[A-Za-z0-9_-]{1,128}")
# B1 review m1: the service's `storage::valid_id` (lowercase, RFC variant, version 4 or 7), not any hex UUID,
# so a value this reader accepts is one the service's decoders accept too.
_VALID_ID = re.compile(_CAPABILITY_UUID)
# A DNS host as the service's `url` crate serializes it (lowercase labels, no percent-encoding, no space),
# or an IPv4 literal; the url crate refuses `exa mple.com` and `ex%41mple.com`, so this reader does too.
_IPV4_HOST = r"(?:(?:25[0-5]|2[0-4][0-9]|1[0-9]{2}|[1-9]?[0-9])\.){3}(?:25[0-5]|2[0-4][0-9]|1[0-9]{2}|[1-9]?[0-9])"
_IPV4 = re.compile(_IPV4_HOST)
_ORIGIN_HOST = re.compile(r"(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)(?:\.[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)*\.?"
                          r"|" + _IPV4_HOST)
# APP-078 review M1: WHATWG's "ends in a number" rule. A host whose last label (ignoring one trailing dot) is
# decimal digits or `0x` hex is parsed as IPv4 by the url crate and then normalized (`127.1`, `0x7f.1`,
# `2130706433`, `012.0.0.1` all become `127.0.0.1`/`10.0.0.1`) or refused (`example.0`, `1.2.3.4.5`,
# `09.1.1.1`); either way the serialization differs from the value, so only a strict canonical dotted quad passes.
_NUMERIC_LABEL = re.compile(r"[0-9]+|0x[0-9a-f]*")


def _graphic(value, bound):
    """`bounded`/`label` in the service: 1..bound bytes, every one printable ASCII, no space."""
    return isinstance(value, str) and 1 <= len(value) <= bound and all(0x21 <= ord(ch) <= 0x7E for ch in value)


def _exact_origin(value):
    """The service's `exact_origin`: an https origin whose ASCII serialization is the value itself.

    B1 review m1: the host must also be one the service's `url` crate would serialize unchanged
    (no space, no percent-encoding, no uppercase), so nothing this reader accepts can make the
    child refuse its configuration file and crash-loop the task. APP-078 review M1: that includes
    WHATWG's "ends in a number" rule, so a numeric last label passes only as a canonical dotted quad."""
    if not isinstance(value, str) or not value.isascii() or "%" in value or len(value) > 512:
        return False
    from urllib.parse import urlsplit
    try:
        url = urlsplit(value)
        port = url.port
    except ValueError:
        return False
    if url.scheme != "https" or not url.hostname or url.username is not None or url.password is not None \
            or url.path or url.query or url.fragment or _ORIGIN_HOST.fullmatch(url.hostname) is None:
        return False
    labels = url.hostname[:-1].split(".") if url.hostname.endswith(".") else url.hostname.split(".")
    if _NUMERIC_LABEL.fullmatch(labels[-1]) is not None and _IPV4.fullmatch(url.hostname) is None:
        return False
    return value == "https://%s%s" % (url.hostname, "" if port in (None, 443) else ":%d" % port)


def _organizations_of(entries, keys):
    seen = []
    for entry in entries:
        require(isinstance(entry, dict) and set(entry) == keys)
        require(isinstance(entry["organization_id"], str) and _VALID_ID.fullmatch(entry["organization_id"]) is not None)
        seen.append(entry["organization_id"])
    return seen


def auth_config_document(value):
    """The service's auth configuration file (`AuthConfig::decode`), checked with its bounds."""
    require(isinstance(value, dict) and set(value) == AUTH_CONFIG_KEYS)
    require(value["schema_version"] == 1 and _exact_origin(value["auth_origin"]) and _exact_origin(value["app_origin"]))
    require(_graphic(value["issuer"], 512) and _graphic(value["audience"], 512))
    lifetime = value["max_access_lifetime_seconds"]
    require(type(lifetime) is int and 1 <= lifetime <= 86400)
    routes = value["organizations"]
    require(isinstance(routes, list) and 1 <= len(routes) <= AUTH_ORGANIZATIONS_MAX)
    organizations = _organizations_of(routes, AUTH_ROUTE_KEYS)
    require(len(set(organizations)) == len(organizations))
    for route in routes:
        require(isinstance(route["tenant_slug"], str) and _TENANT_SLUG.fullmatch(route["tenant_slug"]) is not None)
        require(isinstance(route["client_id"], str) and _CLIENT_ID.fullmatch(route["client_id"]) is not None
                and not route["client_id"].endswith("-hosted"))
    return value


def identity_registry_document(value):
    """The service's closed identity registry (`Registry::load`), checked with its bounds."""
    require(isinstance(value, dict) and set(value) == IDENTITY_REGISTRY_KEYS)
    require(value["schema_version"] == 1)
    require(isinstance(value["registry_version"], str) and _VALID_ID.fullmatch(value["registry_version"]) is not None)
    bindings = value["bindings"]
    require(isinstance(bindings, list) and len(bindings) <= AUTH_ORGANIZATIONS_MAX)
    _organizations_of(bindings, IDENTITY_BINDING_KEYS)
    pairs = set()
    for binding in bindings:
        require(_graphic(binding["issuer"], 256) and _graphic(binding["provider_tenant_id"], 128))
        pair = (binding["issuer"], binding["provider_tenant_id"])
        require(pair not in pairs)
        pairs.add(pair)
    return value


def document_bytes(document):
    """One file's bytes, exactly as the service reads them."""
    return json.dumps(document, sort_keys=True, separators=(",", ":")).encode("utf-8")


def auth_configuration(text):
    """The whole injected secret, validated whole: `{}` (not configured) gives None; otherwise the
    two documents the service reads, consistent with each other (the registry binds exactly the
    routed organizations, every binding under the configured token issuer), as file bytes."""
    require(isinstance(text, str) and len(text.encode("utf-8")) <= AUTH_CONFIGURATION_BYTES_MAX)
    value = json.loads(text, object_pairs_hook=_unique_object)
    require(isinstance(value, dict))
    if not value:
        return None
    require(set(value) == {"format", "auth_config", "identity_registry"} and value["format"] == AUTH_CONFIGURATION_FORMAT)
    config = auth_config_document(value["auth_config"])
    registry = identity_registry_document(value["identity_registry"])
    routed = {route["organization_id"] for route in config["organizations"]}
    require({binding["organization_id"] for binding in registry["bindings"]} == routed)
    # B1 review m2: exactly one binding per routed organization, so `Registry::tenant_for` is never
    # ambiguous (two tenants for one organization would deny that organization's every sign-in silently).
    require(len(registry["bindings"]) == len(routed))
    require(all(binding["issuer"] == config["issuer"] for binding in registry["bindings"]))
    files = {AUTH_CONFIG_FILE_NAME: document_bytes(config), IDENTITY_REGISTRY_FILE_NAME: document_bytes(registry)}
    require(all(len(data) <= AUTH_DOCUMENT_BYTES_MAX for data in files.values()))
    return files


def write_auth_configuration(directory, text):
    """Write the two documents as 0600 files into `directory`, an existing 0700 directory of this
    user (never a link), refusing any existing name. Returns the names written ([] when `{}`)."""
    files = auth_configuration(text)
    if files is None:
        return []
    directory_fd = os.open(directory, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
    written = []
    try:
        info = os.fstat(directory_fd)
        require(stat.S_ISDIR(info.st_mode) and info.st_uid == os.geteuid() and stat.S_IMODE(info.st_mode) == 0o700)
        for name, data in sorted(files.items()):
            descriptor = os.open(name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600,
                                 dir_fd=directory_fd)
            written.append(name)
            try:
                os.fchmod(descriptor, 0o600)
                os.write(descriptor, data)
                os.fsync(descriptor)
            finally:
                os.close(descriptor)
        os.fsync(directory_fd)
    finally:
        os.close(directory_fd)
    return written


class AuthConfigurationDirectory:
    """A fresh 0700 directory under the private runtime volume holding the two configuration files;
    close() removes them and it."""
    PREFIX = "auth-configuration-"

    def __init__(self, runtime_path, text):
        parent = Path(runtime_path).parent
        info = parent.lstat()
        require(stat.S_ISDIR(info.st_mode) and info.st_uid == os.geteuid() and info.st_mode & 0o077 == 0)
        self.path = Path(tempfile.mkdtemp(prefix=self.PREFIX, dir=parent))
        try:
            os.chmod(self.path, 0o700)
            require(write_auth_configuration(str(self.path), text) == sorted([AUTH_CONFIG_FILE_NAME, IDENTITY_REGISTRY_FILE_NAME]))
        except BaseException:
            self.close()
            raise

    @property
    def config_path(self):
        return self.path / AUTH_CONFIG_FILE_NAME

    @property
    def registry_path(self):
        return self.path / IDENTITY_REGISTRY_FILE_NAME

    def close(self):
        if self.path is None:
            return
        try:
            for name in (AUTH_CONFIG_FILE_NAME, IDENTITY_REGISTRY_FILE_NAME):
                Path(self.path, name).unlink(missing_ok=True)
            self.path.rmdir()
        except FileNotFoundError:
            pass
        finally:
            self.path = None


def take_auth_configuration_injection():
    """Remove the injected auth configuration from this process's environment (every mode), so no
    later child can inherit it. Only serve() uses the value."""
    return os.environ.pop(AUTH_CONFIGURATION_JSON_ENV, None)


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


def serve(path, capabilities=None, auth_configuration_text=None):
    # ADR-003 amendment 2026-10-30: out of this process's environment before anything else.
    injected = take_capability_injection()
    capabilities = capabilities if capabilities is not None else injected
    # APP-078 B1: the hosted sign-in configuration leaves the environment the same way.
    injected_auth = take_auth_configuration_injection()
    auth_configuration_text = auth_configuration_text if auth_configuration_text is not None else injected_auth
    refuse_qualification_injection()
    # Only serve() names the capability directory, and only for the injected secret.
    require(CAPABILITY_DIR_ENV not in os.environ)
    # Validated whole before any file, credential or child: a malformed secret never launches.
    auth_files = auth_configuration(auth_configuration_text) if auth_configuration_text is not None else None
    # A configured secret is the only source of both files: a plain environment
    # entry naming either file beside it is refused, never preferred or merged.
    require(auth_files is None or (AUTH_CONFIG_FILE_ENV not in os.environ and IDENTITY_REGISTRY_FILE_ENV not in os.environ))
    stop = threading.Event()
    for sig in (signal.SIGTERM, signal.SIGINT): signal.signal(sig, lambda *_: stop.set())
    child = None
    registry_file = None
    capability_directory = None
    auth_directory = None
    credentials_started = False
    try:
        Path(path).parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        child_environment = os.environ.copy()
        if auth_files is not None:
            auth_directory = AuthConfigurationDirectory(path, auth_configuration_text)
            child_environment[AUTH_CONFIG_FILE_ENV] = str(auth_directory.config_path)
            child_environment[IDENTITY_REGISTRY_FILE_ENV] = str(auth_directory.registry_path)
        if IDENTITY_REGISTRY_FILE_ENV not in child_environment:
            registry_file = EmptyIdentityRegistryFile(path)
            child_environment[IDENTITY_REGISTRY_FILE_ENV] = str(registry_file.path)
        if capabilities is not None:
            # A malformed secret is a terminal refusal: never a partial set, never a launch.
            capability_directory = CapabilityDirectory(path, capabilities)
            child_environment[CAPABILITY_DIR_ENV] = str(capability_directory.path)
        capabilities = injected = auth_configuration_text = injected_auth = auth_files = None
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
                if registry_file is not None:
                    registry_file.close()
            finally:
                try:
                    if capability_directory is not None:
                        capability_directory.close()
                finally:
                    try:
                        if auth_directory is not None:
                            auth_directory.close()
                    finally:
                        if credentials_started:
                            Path(path).unlink(missing_ok=True)
    return 0


# --- APP-012 revision cache fetch (ADR-007 D7.10 fetch/execute separation) ---
CACHE_REQUEST_KEYS = {"organization_id", "source_id", "revision_id", "normalized_digest", "cache_root"}
# A Phase 1 revision is a few KiB; the design ceiling is about 1 million rows.
MAX_REVISION_BYTES = 256 * 1024 * 1024
_UUID = re.compile(r"[0-9a-f]{8}-[0-9a-f]{4}-[47][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}")


def cache_request(raw):
    require(len(raw) <= 4096)
    request = json.loads(raw)
    require(isinstance(request, dict) and set(request) == CACHE_REQUEST_KEYS)
    for key in ("organization_id", "source_id", "revision_id"):
        require(isinstance(request[key], str) and _UUID.fullmatch(request[key]) is not None)
    require(isinstance(request["normalized_digest"], str) and re.fullmatch(r"sha256:[0-9a-f]{64}", request["normalized_digest"]))
    require(isinstance(request["cache_root"], str) and Path(request["cache_root"]).is_absolute())
    return request


def revision_key(request):
    """ADR-009 B1: the normalized object of one revision under its organization's prefix."""
    return "o/%s/s/%s/r/%s/part.parquet" % (request["organization_id"], request["source_id"], request["revision_id"])


def fetch_revision(client, bucket, request):
    """One bounded GetObject, digest-checked, renamed atomically into the cache."""
    directory = Path(request["cache_root"]) / "o" / request["organization_id"]
    directory.mkdir(mode=0o700, parents=True, exist_ok=True)
    metadata = directory.lstat()
    require(stat.S_ISDIR(metadata.st_mode) and metadata.st_uid == os.geteuid() and metadata.st_mode & 0o077 == 0)
    target = directory / ("%s.%s.parquet" % (request["source_id"], request["revision_id"]))
    response = client.get_object(Bucket=bucket, Key=revision_key(request))
    require(0 < int(response["ContentLength"]) <= MAX_REVISION_BYTES)
    body = response["Body"]
    digest = hashlib.sha256()
    fd, temporary = tempfile.mkstemp(prefix=".fetch-", dir=directory)
    try:
        size = 0
        with os.fdopen(fd, "wb") as stream:
            while block := body.read(1024 * 1024):
                size += len(block)
                require(size <= MAX_REVISION_BYTES)
                digest.update(block)
                stream.write(block)
            stream.flush(); os.fsync(stream.fileno())
        require(size == int(response["ContentLength"]) and "sha256:" + digest.hexdigest() == request["normalized_digest"])
        os.chmod(temporary, 0o400)
        os.replace(temporary, target)
    finally:
        if os.path.exists(temporary): os.unlink(temporary)
    return {"cached": True, "bytes": size}


def cache(path, stdin=None):
    request = cache_request((stdin or sys.stdin.buffer).read(4097))
    value = load(path)
    tenants = [t for t in value["storage"]["tenants"] if t["organization_id"] == request["organization_id"]]
    require(len(tenants) == 1)
    tenant = tenants[0]
    require(revision_key(request).startswith(tenant["prefix"]))
    credentials = {"AccessKeyId": tenant["access_key_id"], "SecretAccessKey": tenant["secret_access_key"], "SessionToken": tenant["session_token"]}
    client = scoped_session(credentials, value["storage"]["region"]).client("s3", config=sdk_config())
    return fetch_revision(client, value["storage"]["bucket"], request)


# --- APP-108 box objects (the credentialed write/read child) and the normalizer ---
OBJECT_REQUEST_KEYS = {"operation", "organization_id", "key", "path", "sha256"}
MAX_OBJECT_BYTES = 64 * 1024 * 1024
_OBJECT_KEY = re.compile(r"o/[0-9a-f-]{36}/(b/[0-9a-f-]{36}/f/[0-9a-f-]{36}|s/[0-9a-f-]{36}/r/[0-9a-f-]{36}/(original|part\.parquet))")


def object_request(raw):
    require(len(raw) <= 4096)
    request = json.loads(raw)
    require(isinstance(request, dict) and set(request) == OBJECT_REQUEST_KEYS)
    require(request["operation"] in ("put", "get"))
    require(isinstance(request["organization_id"], str) and _UUID.fullmatch(request["organization_id"]) is not None)
    require(isinstance(request["key"], str) and _OBJECT_KEY.fullmatch(request["key"]) is not None)
    require(request["key"].startswith("o/" + request["organization_id"] + "/"))
    require(isinstance(request["sha256"], str) and re.fullmatch(r"[0-9a-f]{64}", request["sha256"]))
    require(isinstance(request["path"], str) and Path(request["path"]).is_absolute())
    return request


def put_object(client, bucket, tenant, request):
    """One write-once PutObject under the tenant key: SSE-KMS, If-None-Match, SHA-256 checksum."""
    import base64
    path = Path(request["path"])
    metadata = path.lstat()
    require(stat.S_ISREG(metadata.st_mode) and metadata.st_uid == os.geteuid() and metadata.st_size <= MAX_OBJECT_BYTES)
    body = path.read_bytes()
    require(hashlib.sha256(body).hexdigest() == request["sha256"])
    try:
        client.put_object(Bucket=bucket, Key=request["key"], Body=body, ServerSideEncryption="aws:kms",
                          SSEKMSKeyId=tenant["kms_key_arn"], ChecksumSHA256=base64.b64encode(bytes.fromhex(request["sha256"])).decode(),
                          IfNoneMatch="*")
    except Exception as error:
        code = getattr(error, "response", {}).get("Error", {}).get("Code", "")
        if code in ("PreconditionFailed", "412"):
            raise SystemExit(3)
        raise
    return {"stored": True, "bytes": len(body)}


def get_object(client, bucket, request):
    """One bounded GetObject, digest-checked, written to the caller's private path."""
    path = Path(request["path"])
    require(not path.exists())
    response = client.get_object(Bucket=bucket, Key=request["key"])
    require(0 < int(response["ContentLength"]) <= MAX_OBJECT_BYTES)
    body = response["Body"].read(MAX_OBJECT_BYTES + 1)
    require(len(body) == int(response["ContentLength"]) and hashlib.sha256(body).hexdigest() == request["sha256"])
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, "wb") as stream:
        stream.write(body)
    return {"read": True, "bytes": len(body)}


def object_operation(path, stdin=None):
    request = object_request((stdin or sys.stdin.buffer).read(4097))
    value = load(path)
    tenants = [t for t in value["storage"]["tenants"] if t["organization_id"] == request["organization_id"]]
    require(len(tenants) == 1)
    tenant = tenants[0]
    require(request["key"].startswith(tenant["prefix"]))
    credentials = {"AccessKeyId": tenant["access_key_id"], "SecretAccessKey": tenant["secret_access_key"], "SessionToken": tenant["session_token"]}
    client = scoped_session(credentials, value["storage"]["region"]).client("s3", config=sdk_config())
    if request["operation"] == "put":
        return put_object(client, value["storage"]["bucket"], tenant, request)
    return get_object(client, value["storage"]["bucket"], request)


def normalize():
    """The box normalizer child (no credential); imported by path as `query`."""
    import importlib.util
    spec = importlib.util.spec_from_file_location("box_normalize", Path(__file__).resolve().with_name("box_normalize.py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.main()


def query():
    """The confined engine child; imported by path because `python3 -I` drops the script directory."""
    import importlib.util
    spec = importlib.util.spec_from_file_location("query_engine", Path(__file__).resolve().with_name("query_engine.py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.main()


if __name__ == "__main__":
    # ADR-003 amendment 2026-10-30: the capability secret leaves the environment in every
    # mode, first; only serve() writes it out, for its service child alone.
    capabilities = take_capability_injection()
    auth_configuration_text = take_auth_configuration_injection()
    parser = argparse.ArgumentParser(); parser.add_argument("mode", choices=("refresh", "probe", "serve", "migrate", "qualify", "cleanup", "query", "cache", "object", "normalize")); parser.add_argument("--generation"); parser.add_argument("--report")
    args = parser.parse_args()
    try:
        # D7.13: a harness injection in any mode, serve included, is removed
        # and refused before credential refresh, managed probes, migration,
        # qualification or the native child runs (APP-009 review m6).
        refuse_qualification_injection()
        path = os.environ.get("GENUI_RUNTIME_CONFIG_FILE", "/run/genui/credentials.json")
        # The engine child answers every request with its own one JSON line.
        if args.mode == "query": raise SystemExit(query())
        if args.mode == "normalize": raise SystemExit(normalize())
        if args.mode == "refresh": refresh(path)
        elif args.mode == "cache": print(json.dumps(cache(path)))
        elif args.mode == "object": print(json.dumps(object_operation(path)))
        elif args.mode == "probe": print(json.dumps(probe(path, args.generation)))
        elif args.mode == "migrate": migrate()
        elif args.mode == "qualify": qualification(path)
        elif args.mode == "cleanup": cleanup(args.report)
        else: raise SystemExit(serve(path, capabilities, auth_configuration_text))
    except Exception:
        print("managed_dependency_operation_refused", file=sys.stderr)
        raise SystemExit(1)
