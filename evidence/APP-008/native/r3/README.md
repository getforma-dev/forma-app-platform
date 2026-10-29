# R3: refuse PostgreSQL replication-mode drift

The schema review found that the previous catalog query omitted current-database settings applying to all roles (`pg_db_role_setting.setrole=0`). A committed `session_replication_role=replica` default can disable foreign-key triggers in new sessions without changing their catalog enablement flags. The prior native version/catalog checks did not detect this setting.

The native fix requires the actual connection's `session_replication_role` to equal `origin` during initial identity verification and baseline/recycle verification. Reset also checks it **before** `DISCARD ALL`, so cleanup cannot hide drift. A failure discards the connection through the existing owned-driver path. Production code never grants permission or issues a privileged replication-mode SET to repair the session. Existing acquisition, transaction, cleanup and capacity bounds are unchanged.

The schema owner's canonical query separately includes all-role current-database settings and effective fixed-role SET/ALTER SYSTEM permissions on this parameter. Native storage embeds and compares those exact resources; it does not implement an alternate catalog policy. Final query SHA-256 is `3af18e8ec8b58b52e7956dc900deae8dffc9a585bc1db9e1ce1ab4dee02ed6bc`; expected version2 catalog is `5772132f163b5490b8d39f6d1fdc26cef388100daf9fd8e86cc7aa9f8ebffe07`.

Three checks extend the existing real local PostgreSQL/TLS campaign:

1. Commit the database-wide replica default. Observe replica mode in new connections for every fixed role, refuse native acquisition/readiness, then restore the owned fixture's database default and verify readiness recovers.
2. Confirm an ordinary workspace role cannot set replication mode. The fixture operator temporarily grants permission to create actual session drift, then revokes it before the native guard runs. Reset refuses the drift and the next lease has a different backend in origin mode.
3. Use test-only direct Deadpool access to simulate an idle corrupted session after the same temporary permission is revoked. Recycle discards that backend and admits a different origin-mode connection. No production caller can select this fixture path or obtain the raw pool/client.

The original 29-check receipts and independent review remain untouched. `intermediate-catalog-setup-refusal.txt` records a failed setup attempt while the schema resources were being finalized; it reached no native checks. The final `author-checks.json` passes with unchanged before/after source inputs: all **32** local PostgreSQL/TLS checks, 18 library tests, strict lint, the production library build, and Rust1.85 native-unit/public-boundary checks. `local-postgres-tls.json` is the new full campaign result. Independent follow-up review remains required before this R3 source candidate is accepted. No managed RDS, cloud, image or Git action was performed by this author.
