#!/usr/bin/env python3
"""Reset an owned disposable fixture workspace; never connect to a supplied DB.

Two fresh local PostgreSQL containers use the canonical APP-008 migration/seed.
This is neither backup restore nor resetting shared/authored rehearsal state.
"""
from __future__ import annotations

import argparse
from contextlib import contextmanager
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys

from verify import DEFAULT, ROOT, load_package, sha

ROLES = ("genui_workspace", "genui_control", "genui_data", "genui_worker")
TABLES = tuple(sorted(("organizations", "provider_tenant_bindings", "principals", "memberships", "spaces", "apps",
    "definition_revisions", "definition_inputs", "definition_record_schemas", "publication_policies", "publications",
    "publication_inputs", "publication_record_schemas", "saved_work", "sources", "source_revisions", "record_collections",
    "work_bindings", "work_source_dependencies", "internal_records", "internal_record_revisions", "record_source_dependencies",
    "organization_permissions", "app_permissions", "work_permissions", "source_permissions")))
AUTHOR = "00000000-0000-4000-8000-000000000066"
ADMIN = "00000000-0000-4000-8000-000000000f00"


class TrialError(ValueError):
    pass


class InjectedFailure(TrialError):
    """Private failure-path check; never a CLI or target-selection option."""


def require(condition, reason):
    if not condition:
        raise TrialError(reason)


def encoded(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()


def digest(value):
    return "sha256:" + hashlib.sha256(encoded(value)).hexdigest()


def save(path, value):
    # Every receipt is append-only within a newly created evidence directory.
    with path.open("x") as stream:
        stream.write(json.dumps(value, indent=2, sort_keys=True) + "\n")


def docker(args, *, check=True):
    result = subprocess.run(["docker", *args], capture_output=True, text=True, timeout=60)
    if check and result.returncode:
        raise TrialError("local Docker operation failed")
    return result


def local_environment():
    # Explicit psycopg arguments are not enough if libpq inherits PGSERVICE or
    # PGHOSTADDR. Refuse ambient overrides without logging any values.
    require(not any(name.startswith("PG") for name in os.environ), "PostgreSQL environment overrides refused")
    require(not any(name in os.environ for name in ("DOCKER_HOST", "DOCKER_CONTEXT", "DOCKER_TLS_VERIFY", "DOCKER_CERT_PATH")),
            "Docker endpoint overrides refused")
    context = json.loads(docker(["context", "inspect"]).stdout)
    require(len(context) == 1 and context[0]["Endpoints"]["docker"]["Host"].startswith("unix:///"),
            "a local Unix-socket Docker context is required")
    return {"docker_transport": "local Unix socket", "database_transport": "fresh random loopback port; no ambient PostgreSQL overrides"}


def dependencies():
    sys.path.insert(0, str(ROOT / "app/runtime"))
    import runtime
    import tenant_seed
    import qualify_tenant_schema
    import psycopg
    from psycopg import sql
    return runtime, tenant_seed, qualify_tenant_schema, psycopg, sql


def context(c, organization, principal=AUTHOR, epoch=1):
    c.execute("SELECT set_config('genui.organization_id',%s,true),set_config('genui.principal_id',%s,true),set_config('genui.membership_epoch',%s,true)",
              (organization, principal, str(epoch)))


@contextmanager
def owned_workspace(helper, receipt):
    pg = helper.LocalPostgres()  # No arguments, profiles, URLs or target selector.
    receipt.update(name=pg.name, created=False, removed=False)
    identity = None
    entered = False
    try:
        pg.__enter__()
        entered = True
        observed = json.loads(docker(["inspect", pg.name]).stdout)[0]
        identity = observed["Id"]
        image = json.loads(docker(["image", "inspect", helper.IMAGE]).stdout)[0]
        bindings = observed["NetworkSettings"]["Ports"]["5432/tcp"]
        require(observed["Image"] == image["Id"] and observed["Name"] == "/" + pg.name, "owned container identity mismatch")
        require(bindings == [{"HostIp": "127.0.0.1", "HostPort": str(pg.port)}], "database binding is not the owned loopback port")
        require(observed["HostConfig"]["AutoRemove"] is True, "owned container autoremove missing")
        require(not any(mount["Type"] == "bind" for mount in observed["Mounts"]), "owned database unexpectedly binds host files")
        receipt.update(created=True, container_id=identity, image_id=image["Id"], image_architecture=image["Architecture"],
                       loopback_port=pg.port, postgres_image=helper.IMAGE)
        yield pg
    finally:
        # Never redirect cleanup to an arbitrary name or remove a replacement
        # that reuses it. The helper already cleans up failed __enter__ attempts.
        if entered:
            current = docker(["inspect", pg.name], check=False)
            if current.returncode == 0:
                current_id = json.loads(current.stdout)[0]["Id"]
                require(identity is None or current_id == identity, "refuse cleanup of replacement container")
                pg.__exit__(None, None, None)
            # A successful exact-name lookup is required even if the container
            # auto-removed itself. Docker daemon failure cannot prove absence.
            remaining = docker(["ps", "--all", "--quiet", "--no-trunc", "--filter", "name=^/" + pg.name + "$"]).stdout.strip()
            require(not remaining, "owned container cleanup incomplete")
            receipt["removed"] = True
        else:
            receipt["removed"] = pg.removed


def snapshot(pg, runtime, migration_dir, organizations, sql):
    rows = {}
    with pg.connect() as c:
        runtime.verify_schema_catalog(c, migration_dir)
        found = tuple(row[0] for row in c.execute("SELECT tablename FROM pg_tables WHERE schemaname='genui' ORDER BY tablename").fetchall())
        require(found == TABLES, "unexpected fixture relation inventory")
        version = c.execute("SELECT singleton,version FROM genui_meta.schema_version").fetchall()
        history = c.execute("SELECT version,sha256,applied_at::text FROM genui_meta.schema_migrations ORDER BY version").fetchall()
        c.execute("SET LOCAL ROLE genui_owner")
        for org in organizations:
            context(c, org)
            rows[org] = {}
            for table in TABLES:
                query = sql.SQL('SELECT to_jsonb(t)::text FROM genui.{} AS t ORDER BY to_jsonb(t)::text COLLATE "C" LIMIT 1001').format(sql.Identifier(table))
                values = [json.loads(row[0]) for row in c.execute(query).fetchall()]
                require(len(values) <= 1000, "fixture row bound exceeded")
                require(all(row["organization_id"] == org for row in values), "snapshot escaped tenant scope")
                rows[org][table] = values
    logical = {"tenant_relations": rows, "schema_version": version,
               "migration_history": [[row[0], row[1]] for row in history],
               "catalog_digest": sha(migration_dir / "catalog-v2.json")}
    # Local disposable superuser is read-only here: prove no third tenant or
    # otherwise invisible rows were omitted by the two scoped snapshots.
    with pg.connect("postgres") as c:
        for table in TABLES:
            total = c.execute(sql.SQL("SELECT count(*) FROM genui.{}").format(sql.Identifier(table))).fetchone()[0]
            require(total == sum(len(rows[org][table]) for org in organizations), "snapshot omitted out-of-scope rows")
    require(len(encoded(logical)) <= 10 * 1024 * 1024, "fixture logical snapshot too large")
    return {"logical": logical, "digest": digest(logical),
            "row_counts": {org: {table: len(value) for table, value in tables.items()} for org, tables in rows.items()},
            "observed_migration_times": [{"version": row[0], "applied_at": row[2]} for row in history],
            "excluded_from_logical_comparison": ["migration applied_at is an observed installation time, retained separately", "container IDs, loopback ports and execution timing"]}


def exact_rows(actual, expected, reason):
    require(sorted(actual, key=encoded) == sorted(expected, key=encoded), reason)


def verify_seed_manifest(state, setup, bundles, seed_module):
    organizations = setup["organizations"]
    require([item["organization_id"] for item in organizations] == list(seed_module.ORGANIZATIONS), "setup organization inventory differs")
    expected_apps = {(org["organization_id"], item["app_id"]) for org in organizations for item in org["contract_shape_seeds"]}
    require({(b["context"]["organization_id"], b["definition"]["app_id"]) for b in bundles} == expected_apps, "seed bundle inventory differs")
    for org in organizations:
        oid = org["organization_id"]
        tables = state["logical"]["tenant_relations"][oid]
        binding = org["provider_binding"]
        exact_rows(tables["organizations"], [{"organization_id": oid, "display_name": "Synthetic organization " + oid[-4:], "status": "active"}], "organization manifest mismatch")
        exact_rows(tables["provider_tenant_bindings"], [{"organization_id": oid, **binding, "active": True}], "provider binding mismatch")
        exact_rows(tables["principals"], [{"organization_id": oid, **binding, "principal_id": p["principal_id"], "provider_subject": p["provider_subject"]} for p in org["principals"]], "principal inventory mismatch")
        exact_rows(tables["memberships"], [{"organization_id": oid, "principal_id": p["principal_id"], "status": p["status"], "membership_epoch": p["membership_epoch"]} for p in org["principals"]], "membership inventory mismatch")
        exact_rows(tables["organization_permissions"], [{"organization_id": oid, "principal_id": ADMIN, "permission": "admin_policy"}], "organization grants mismatch")
        apps, work, records, app_grants, work_grants, source_grants = [], [], [], [], [], []
        for fixture in org["contract_shape_seeds"]:
            app = fixture["app_id"]
            bundle = next(b for b in bundles if b["context"]["organization_id"] == oid and b["definition"]["app_id"] == app)
            author, reviewer = fixture["author_principal_id"], fixture["reviewer_principal_id"]
            apps.append({"organization_id": oid, "app_id": app, "space_id": seed_module.SPACE, "owner_principal_id": author})
            work.append({"organization_id": oid, "app_id": app, "work_id": fixture["work_id"], "publication_id": fixture["publication_id"], "version": fixture["work_version"], "context": bundle["work"]["context"]})
            require(len(fixture["collection_ids"]) == 1, "unexpected seeded collection count")
            records.append({"organization_id": oid, "app_id": app, "work_id": fixture["work_id"], "collection_id": fixture["collection_ids"][0], "record_id": fixture["record_id"], "current_version": fixture["record_version"]})
            for purpose, principal in (("author_app", author), ("reviewer_app", reviewer)):
                app_grants.extend({"organization_id": oid, "principal_id": principal, "app_id": app, "permission": permission} for permission in fixture["seeded_grants"][purpose])
            work_grants.extend({"organization_id": oid, "principal_id": author, "app_id": app, "work_id": fixture["work_id"], "permission": permission} for permission in fixture["seeded_grants"]["author_work"])
            source_grants.extend({"organization_id": oid, "principal_id": author, "source_id": source["source_id"], "permission": permission} for source in bundle["sources"] for permission in fixture["seeded_grants"]["author_sources"])
            actual_definition = next(row for row in tables["definition_revisions"] if row["app_id"] == app)
            actual_publication = next(row for row in tables["publications"] if row["app_id"] == app)
            require(bytes.fromhex(actual_definition["canonical_content"][2:]) == seed_module.canonical(bundle["definition"]["content"]), "seeded definition bytes differ")
            require(bytes.fromhex(actual_publication["canonical_envelope"][2:]) == seed_module.canonical(bundle["publication"]), "seeded publication bytes differ")
        for name, expected in (("apps", apps), ("saved_work", work), ("internal_records", records), ("app_permissions", app_grants), ("work_permissions", work_grants), ("source_permissions", source_grants)):
            exact_rows(tables[name], expected, name + " manifest mismatch")
    return {"organizations": len(organizations), "principals": sum(len(o["principals"]) for o in organizations), "bundles": len(bundles), "table_count_per_organization": len(TABLES),
            "scope": "exact seeded identities, bindings, memberships, app/work/record identities, grants and canonical definitions/publications; all 26 relation rows retained for reset equality"}


def role_trials(pg, initial, organizations, sql, psycopg):
    results = []
    for role in ROLES:
        with pg.connect(role) as c:
            require(c.execute("SELECT session_user::text,current_user::text").fetchone() == (role, role), "database role mismatch")
            require(c.execute("SELECT count(*) FROM genui.organizations").fetchone() == (0,), "unscoped role saw tenant data")
            allowed = [table for table in TABLES if c.execute("SELECT has_table_privilege(current_user,%s,'SELECT')", ("genui." + table,)).fetchone()[0]]
            for org in organizations:
                context(c, org)
                other = next(item for item in organizations if item != org)
                for table in allowed:
                    own = c.execute(sql.SQL("SELECT count(*) FROM genui.{}").format(sql.Identifier(table))).fetchone()[0]
                    foreign = c.execute(sql.SQL("SELECT count(*) FROM genui.{} WHERE organization_id=%s").format(sql.Identifier(table)), (other,)).fetchone()[0]
                    require(own == initial["row_counts"][org][table] and own > 0 and foreign == 0, "role positive/canary negative failed")
                results.append({"role": role, "organization_id": org, "own_rows_match_manifest": True, "foreign_rows": 0, "granted_tables": allowed})
            context(c, "")
            for table in allowed:
                require(c.execute(sql.SQL("SELECT count(*) FROM genui.{}").format(sql.Identifier(table))).fetchone() == (0,), "cleared scope retained visibility")
        for statement in ("SET ROLE genui_owner", "INSERT INTO genui.organizations DEFAULT VALUES"):
            try:
                with pg.connect(role) as c:
                    context(c, organizations[0])
                    c.execute(statement)
            except psycopg.Error as exc:
                require(exc.sqlstate == "42501", "wrong role-escalation refusal")
            else:
                raise TrialError("application role acquired mutation/owner authority")
    return {"role_organization_checks": results, "missing_cleared_scope_denied": True, "role_escalation_and_direct_write_denied": True,
            "scope": "actual SQL-role privileges and organization RLS; not provider authentication or native subject authorization"}


def mutate_owned_work(pg, setup):
    org = setup["organizations"][0]
    fixture = org["contract_shape_seeds"][0]
    oid, app, work = org["organization_id"], fixture["app_id"], fixture["work_id"]
    with pg.connect() as c:
        require(c.execute("SELECT session_user::text,current_user::text,current_database()").fetchone() == ("genui_migration", "genui_migration", "genui"), "mutation operator identity mismatch")
        c.execute("SET LOCAL ROLE genui_owner")
        context(c, oid, fixture["author_principal_id"])
        # Preserve APP-008's organization -> member -> work ordering. This is an
        # explicit operator test mutation, never an application command receipt.
        require(c.execute("SELECT organization_id FROM genui.organizations WHERE organization_id=%s FOR SHARE", (oid,)).fetchone() is not None, "mutation organization missing")
        require(c.execute("SELECT status,membership_epoch FROM genui.memberships WHERE organization_id=%s AND principal_id=%s FOR SHARE", (oid, fixture["author_principal_id"])).fetchone() == ("active", 1), "mutation membership changed")
        before = c.execute("SELECT version,context FROM genui.saved_work WHERE organization_id=%s AND app_id=%s AND work_id=%s FOR UPDATE", (oid, app, work)).fetchone()
        require(before is not None and before[0] == fixture["work_version"], "mutation work pin changed")
        after = c.execute("UPDATE genui.saved_work SET version=version+1,context=%s WHERE organization_id=%s AND app_id=%s AND work_id=%s AND version=%s RETURNING version,context",
                          (before[1] + " | APP-011 disposable operator mutation", oid, app, work, before[0])).fetchall()
        require(len(after) == 1 and after[0][0] == before[0] + 1, "mutation did not advance exact work")
    return {"organization_id": oid, "app_id": app, "work_id": work, "before": {"version": before[0], "context": before[1]}, "after": {"version": after[0][0], "context": after[0][1]}, "lock_order": ["organization SHARE", "member SHARE", "work UPDATE"], "application_command": False}


def run_trial(output, *, _fail_after_mutation=False):
    output = Path(output).resolve()
    output.mkdir(parents=True, exist_ok=False)
    report = {"schema_version": 1, "passed": False, "started_at": datetime.now(timezone.utc).isoformat(), "workspaces": [],
              "scope": "owned disposable local synthetic fixture setup/reset, not backup restore or a shared managed reset",
              "not_exercised": ["RDS IAM/TLS", "actual imported/normalized sources", "authored applications", "provider identities/sessions", "native per-principal authorization", "APP-057 shared rehearsal reset", "backup recovery"]}
    try:
        report["environment"] = local_environment()
        load_package()
        setup = json.loads((DEFAULT / "setup.json").read_text())
        runtime, seed_module, helper, psycopg, sql = dependencies()
        organizations = list(seed_module.ORGANIZATIONS)
        require([o["organization_id"] for o in setup["organizations"]] == organizations, "setup organizations are not the fixed synthetic pair")
        bundles = seed_module.make_bundles()
        migration_dir = ROOT / "app/migrations"
        inputs = [Path(__file__), ROOT / "fixtures/verify.py", DEFAULT / "manifest.json", DEFAULT / "setup.json", ROOT / "app/runtime/runtime.py", ROOT / "app/runtime/tenant_seed.py", ROOT / "app/runtime/qualify_tenant_schema.py", ROOT / "app/runtime/requirements.lock", ROOT / "contracts/requirements.lock", ROOT / "contracts/validate.py", ROOT / "contracts/implementation-lock.json", *sorted(migration_dir.glob("*"))]
        report["source_pins"] = {str(p.relative_to(ROOT)): sha(p) for p in inputs if p.is_file()}
        report["seed_bundle_digest"] = "sha256:" + hashlib.sha256(seed_module.canonical(bundles)).hexdigest()
        for ordinal in (1, 2):
            require(all(sha(ROOT / name) == value for name, value in report["source_pins"].items()), "reset inputs changed during qualification")
            receipt = {"ordinal": ordinal}
            report["workspaces"].append(receipt)
            try:
                with owned_workspace(helper, receipt) as pg:
                    with pg.connect() as c:
                        runtime.apply_migrations(c, migration_dir)
                    with pg.connect() as c:
                        seed_module.seed(c, bundles)
                    state = snapshot(pg, runtime, migration_dir, organizations, sql)
                    receipt["seed_manifest"] = verify_seed_manifest(state, setup, bundles, seed_module)
                    save(output / f"workspace-{ordinal}-initial.json", state)
                    receipt["initial_state_digest"] = state["digest"]
                    with pg.connect() as c:
                        seed_module.seed(c, bundles)
                    require(snapshot(pg, runtime, migration_dir, organizations, sql)["logical"] == state["logical"], "repeat seed changed logical state")
                    receipt["repeat_seed_identical"] = True
                    receipt["role_trials"] = role_trials(pg, state, organizations, sql, psycopg)
                    if ordinal == 1:
                        first = state
                        mutation = mutate_owned_work(pg, setup)
                        save(output / "operator-mutation.json", mutation)
                        changed = snapshot(pg, runtime, migration_dir, organizations, sql)
                        save(output / "workspace-1-mutated.json", changed)
                        differences = [(org, table) for org in organizations for table in TABLES if first["logical"]["tenant_relations"][org][table] != changed["logical"]["tenant_relations"][org][table]]
                        require(differences == [(organizations[0], "saved_work")] and changed["digest"] != first["digest"], "mutation escaped exact first-workspace target")
                        try:
                            with pg.connect() as c:
                                seed_module.seed(c, bundles)
                        except ValueError as exc:
                            require(str(exc) == "existing seed differs; no authority overwritten", "wrong changed-seed refusal")
                        else:
                            raise TrialError("seed overwrote changed work")
                        require(snapshot(pg, runtime, migration_dir, organizations, sql)["logical"] == changed["logical"], "refused seed changed mutated state")
                        receipt.update(mutated_state_digest=changed["digest"], changed_seed_refused_without_overwrite=True)
                        if _fail_after_mutation:
                            raise InjectedFailure("intentional cleanup check after committed disposable mutation")
                    else:
                        require(first["logical"] == state["logical"], "fresh disposable reset differs from initial logical state")
                        report["reset_comparison"] = {"equal": True, "initial_digest": first["digest"], "fresh_reset_digest": state["digest"], "relations_per_org": len(TABLES), "organizations": len(organizations), "all_columns_and_rows_compared": True}
            finally:
                save(output / f"workspace-{ordinal}-receipt.json", receipt)
            require(receipt["removed"], "workspace removal not confirmed")
        require(report["workspaces"][0]["container_id"] != report["workspaces"][1]["container_id"], "reset reused the first database")
        require(all(sha(ROOT / name) == value for name, value in report["source_pins"].items()), "reset inputs changed during qualification")
        report["passed"] = True
    except BaseException as exc:
        report["failure"] = {"type": type(exc).__name__, "message": str(exc)[:500]}
        raise
    finally:
        report["finished_at"] = datetime.now(timezone.utc).isoformat()
        report["all_owned_resources_removed"] = bool(report["workspaces"]) and all(w.get("removed") for w in report["workspaces"])
        save(output / "summary.json", report)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    # Turn normal operator termination into an exception so owned cleanup and
    # failure receipts run. SIGKILL cannot execute language-level finally.
    signal.signal(signal.SIGTERM, signal.default_int_handler)
    result = run_trial(args.output)
    print(json.dumps({"passed": result["passed"], "owned_workspaces_removed": result["all_owned_resources_removed"], "output": str(args.output)}))


if __name__ == "__main__":
    main()
