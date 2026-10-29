# Independent read-only schema1 preflight review

**No blocking findings** at helper `c29ea7f8fcfc105e85c9e2dcd12a2b288365732aa461f62acb71b1ba8b352804` and qualifier `cc37b32f3c0b083668fd347dff5c4392fde674a8ee7de31c14db71c4591dbe81`. The reviewer did not implement either file. [The review record](independent-review.json) pins the helper, tests, exact catalog resources and supporting fixture inputs; those inputs remained unchanged through the independent checks.

## Boundary reviewed

The helper accepts only its emission command and embeds the exact reviewed query and version1 baseline. The emitted bootstrap validates decompressed length/hash; the helper validates both resource hashes. There is no supplied SQL, dynamic baseline adoption, migration or repair path. The complete emitted compact override is **7,867 bytes**, below8,192, with SHA256 `c423cce86ea1b14f9f473795fad7520c9885c683b078058d69353708d4ee78da`. Any additional override fields still require checking the final whole document's size.

Runtime requires UID65532 and the fixed retained RDS host, port, operator, database and `verify-full` CA path. Alternate libpq `PG*` address/service/options channels are refused. The task-injected password is consumed privately and passed directly to the driver; success output contains fixed assertions and digests, while failure output has one fixed error without connection, role, catalog-difference or exception details. The reported resource ID is explicitly labeled as coming from reviewed control-plane evidence.

Safe search order, default read-only mode and finite timeouts are installed at connection startup. A repeatable-read, read-only transaction checks actual origin/TLS/operator facts, compares the bounded canonical version1 catalog, and only then reads the application-owned version table. The wrapper explicitly rolls back. A30-second process alarm bounds execution. This is point-in-time proof and requires the planned operator/schema quiescence during the later auth window.

## Exact task entry point

The existing migration task's `['python3','/app/runtime.py']` entry point cannot run this payload with a command override alone. Initial review also identified that combining an emitted `python3` prefix with a replacement `['python3']` would duplicate the interpreter argument. The final helper instead emits **`command=['-c', payload]`**, requiring a separately prepared temporary migration-derived task definition with **`entryPoint=['python3']`**. The final composition is `python3 -c <payload>`; it was exercised below. The [initial18-case receipt](initial-local-qualification.json) remains historical and does not certify the final descriptor.

Root must review/register the temporary definition, preserve the exact image/UID and intended existing secret channel, verify the required nonsecret environment and absence of alternate PG options, and inspect the final structured overrides. This review neither registers that definition nor claims the existing one works unchanged.

## Independent execution

The [independent qualification](independent-qualification.json) passed all **20 cases** against actual PostgreSQL16.13 with verified local TLS. It repeated deterministic emission/resource validation, environment refusals, sanitized process refusal, successful version1 reads, actual read-only UPDATE denial, unchanged-state role and schema2 refusals, temporary-catalog and version-view positive controls, and replica-default refusal. The owned database container was removed. Migration statements in this qualification only prepare owned local positive/negative fixtures; the emitted helper never migrates a database.

The reviewer then ran the **complete emitted bootstrap and `main()`** in the cached exact schema1 callback image `genui-synthetic@sha256:a4a47707e1e4028221cba7f5f0dfdf03ef663c6f46d0686a27632c4fd5fc5d08`, with its actual UID65532, a read-only root and the replacement Python entry point. An owned **INTERNAL Docker network** resolved the fixed RDS hostname solely to the local PostgreSQL fixture. A temporary synthetic certificate matching that alias was mounted at the fixed CA path. No image pull, real credential or external/AWS connection occurred.

That complete process returned exit0 with the expected schema1/read-only/verified-TLS receipt and empty stderr. Repeating it with the image's normal CA bundle, which does not trust the synthetic server, returned exit1 and only the fixed refusal JSON, again with empty stderr. The database logical dump remained unchanged. Both test containers and the internal network were removed. [The process receipt](independent-image-entrypoint.json) and [reproduction script](qualify-image-entrypoint.py) retain the exact conditions. The hostname in that local receipt is a **fixture alias, not an RDS observation**.

Actual ECS task registration, private secret injection, current RDS SQL/TLS identity, managed execution and cleanup remain root-owned gates. The existing image's accepted CI is separate from this new operator payload's local independent qualification; no new remote CI pass is claimed. This review does not start the auth clock, pass hosted signup or mailbox checks, complete APP-006/008, or admit beta users. The reviewer made no production-source, cloud, Git or ticket-status changes.

## Follow-up — retained foundation image

At root's request, the reviewer repeated only the full entry-point success and untrusted-CA refusal on the already cached foundation image `genui-synthetic@sha256:770bcb00395b135325ebb0955edc87a84c3c86b7ec05d0711af9a7765f03eb22`. Both passed with the same unchanged helper, payload and7867-byte override. The database dump was unchanged and all owned containers/network were removed. [The new foundation receipt](independent-foundation-image-entrypoint.json) is separate from the earlier callback-image proof.

The [source comparison](independent-foundation-source-compatibility.json) shows foundation source `8f8c34ba0b5e47a55990b273fc6d555d41369791` and callback source `18c33f538a19d06cb77f37d091c71b1a8d5e3c39` have identical runtime dependency-lock/CA hashes and the same Python runtime base. [Actual cached-image inspection](independent-image-runtime-compatibility.json) also found equal dependency-lock and CA bytes, Python3.11.14 and psycopg3.3.6. Those checks used no network and read no credentials. The helper imports neither `runtime.py` nor the application binary.

There is **no identified helper/image compatibility obstacle** to using that exact retained foundation image for the standalone read-only preflight. Root must still reobserve the current operating window, reserved standalone allowance, task definition, chosen immutable image and live RDS state before any managed task. This compatibility result does not select or start the separate auth window, reserve additional spend or establish a current managed SQL result.
