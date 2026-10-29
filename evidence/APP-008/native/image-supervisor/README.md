# Confined production-supervisor image smoke

The APP-008 launcher now supplies an empty private identity registry. The historical APP-006 secret-wiring fixture used an invalid database hostname and no packaged CA, so it no longer reaches the intended constructor branch. Those earlier source/evidence files remain unchanged.

`app/runtime/smoke_supervisor.py` supplies one explicitly synthetic credential-refresh function inside an isolated test process. Everything else is the image's production `runtime.serve`, owned-file handoff, `/app/genui`, callback router and signal cleanup. The fixture uses an accepted synthetic RDS hostname and the packaged RDS CA. It creates no real provider identity, grant or managed credential. There is no new production flag, constructor or alternate verification path.

`qualify_image.py` now runs this script through stdin alongside its existing HTTP smoke, using the same no-network, read-only-root, UID65532, no-capabilities, no-new-privileges container controls and private writable volume. The runtime image is unchanged. The actual run used local ARM64 image `sha256:160296acae0418f0ac46b71fe21e27c19f8635d4494bef785a806769c2e19e46` (`genui-app008:local-r5`).

All three modes pass:

- No auth injection: real callback404 and managed readiness503.
- Valid synthetic injection: real callback303 to the fixed clean result path, without a flow/cookie or provider exchange; readiness remains503.
- Malformed injected JSON: Rust refuses before any listener is announced.

For each mode, a separate observer reads the actual native process's `/proc` environment and command line, without intercepting production process launch. It verifies no raw JSON or synthetic secrets are inherited; only private file paths are present. The credential, empty registry and applicable auth file have UID65532 ownership, mode0600 and one link. The registry has no bindings. Captured process output and callback bodies disclose no tested synthetic secret. After normal termination or malformed-child exit, every owned file is absent and the observed native child is gone. The outer qualifier removes its containers and volume.

[Author checks](author-checks.json) retain helper source hashes, exact command, unchanged image identity and unchanged production inputs relative to the R5 source receipt. [The image receipt](attempt-1.json) includes the existing HTTP checks, new supervisor results and actual container controls. The first run passed; no prior receipt was overwritten. Independent review/rerun remains required.

This is local supervisor/configuration behavior only. Credential refresh is synthetic, outbound networking is disabled, and native database readiness remains false. It does not prove ECS secret delivery, RDS IAM, S3/KMS, hosted callbacks, mailbox verification, application sessions or admission. No image build, Git mutation or cloud operation was performed.
