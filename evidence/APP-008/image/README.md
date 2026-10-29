# APP-008 local service image evidence

The initial R4 build used the exact 36 source inputs in [context.json](context.json), independently exported through a scratch `COPY` and compared byte-for-byte with the allowlist. [Build metadata](build-metadata.json) and [local image checks](local-r4.json) retain the actual ARM64 image identity and successful confined volume/HTTP checks, including the corrected missing-registry precondition.

That candidate predates the later temporary-schema and Python readiness-probe ordering repairs. It is retained as historical local evidence and is **not** the deployment candidate. A fresh build, input binding and affected image checks must follow the final repairs. No image here has been published or deployed.

[PostgreSQL registry evidence](postgres-registry-index.json) independently hashes the pinned official registry index and identifies both Linux AMD64 and ARM64 child manifests. The existing PostgreSQL pin is a multi-architecture index; the local Docker inspection's selected architecture does not limit the CI fixture to ARM64.

## Final repaired candidate

The [R5 context](r5/context.json) exports and hashes all 36 actual build inputs after the independent schema/native repairs. [The after-build check](r5/source-after.json) confirms those bytes did not change. The resulting local ARM64 image is `sha256:160296acae0418f0ac46b71fe21e27c19f8635d4494bef785a806769c2e19e46`; [build metadata](r5/build-metadata.json) retains its build identity. It is not published or deployed.

Root independently reviewed the new supervisor smoke and its narrow image-runner integration, then reran the [actual confined image qualification](r5/independent-image.json). All checks passed with network disabled, read-only root, UID65532, dropped capabilities, resource bounds and an owned runtime volume. The native HTTP check proves public rendering/security headers, unready status without native storage, and explicit missing-registry startup refusal with an otherwise accepted hostname.

The supervisor check replaces only managed credential refresh with a labeled synthetic fixture. It runs the actual supervisor and native executable, observes private0600 credential/registry/auth files and the empty deny-all registry, and reads the actual native process environment/arguments to verify raw injected auth material is absent. Disabled auth returns404; injected auth returns303 to the neutral result; malformed auth configuration exits before listening. Managed readiness remains503, no provider request is made, and all owned files/processes plus the Docker volume are removed. The original author run and exact smoke-script identities remain in [supervisor evidence](../native/image-supervisor/README.md); root did not author those two scripts.

These are local image/startup checks, not RDS IAM, hosted callback, user admission or managed acceptance. Exact source CI and later source/image registry binding remain required before a version2 deployment.
