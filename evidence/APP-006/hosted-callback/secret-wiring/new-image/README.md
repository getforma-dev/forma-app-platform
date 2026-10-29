# Combined callback/runtime local image

**Passed local qualification.** Image tag `genui-app006-wiring:local-20261029`; local image ID **`sha256:a4a47707e1e4028221cba7f5f0dfdf03ef663c6f46d0686a27632c4fd5fc5d08`**. This is Linux **ARM64**, UID65532, built from the reviewed callback plus optional secret-file runtime handoff. There was no registry publication, push, cloud operation, app/source edit, staging or commit in this image task.

The repository-root Docker context uses the independently verified exact24-file allowlist. `source-before.json` and `source-after.json` record all24 hashes and match exactly. The ignore file matches the successful independent full-context export, no Dockerfile-specific override exists, and the runtime input matches the reviewed secret-handoff source. This is a working-tree/content-hash candidate build, not an immutable commit or AMD64 deployment claim.

The exact commands, input-script hashes, outputs and image metadata are in [image-receipt.json](image-receipt.json). Local checks passed:

- `docker build --platform linux/arm64 --tag genui-app006-wiring:local-20261029 --file app/Dockerfile .`.
- The existing `app/runtime/qualify_image.py --image ... --output .../new-image/foundation-image.json` verified real UID65532/mode0700 runtime volume ownership, mode0600 atomic writes, native SSR/security behavior, and readiness503 without a managed probe. Its owned temporary volume was removed.
- The unchanged callback HTTP probe ran against the image's actual `/app/genui` with network disabled and a read-only root. It checked default callback404, explicitly configured callback303, unauthorized/malformed start403, protected-route404, restart and absence of synthetic capabilities in native output.
- The reviewed secret-wiring supervisor probe ran inside the image using its actual `/app/runtime.py` and `/app/genui`. Only managed refresh was replaced by the existing synthetic credential fixture; a temporary module link selected the real image runtime. Absent injection gave404, valid injection gave303, and malformed JSON was refused by Rust before listening. All owned secret/credential files and the temporary fixture tree were removed, and no synthetic secret appeared in output.

The HTTP probes used `--network=none`, a read-only root, dropped capabilities, no-new-privileges, two CPUs, bounded memory/PIDs and a small temporary filesystem. `cleanup-checks.json` confirms their exact owned containers are absent. The image is retained for inspection.

All prior callback `local-checks.json`, image-input receipts and earlier image evidence remain untouched; their hashes were checked before and after. New outputs are confined to this directory. The earlier image IDs retain their original meaning and are not rewritten to this combined image.

These checks establish local image inclusion and behavior. They do **not** establish ECS secret retrieval/delivery, real beta DNS/TLS/callback availability, hosted auth exchange, KMS custody, mailbox delivery, refresh revocation, membership, sessions or public beta readiness. Root owns combined-source CI, the eventual deployment image/ARN binding and any later managed trial or readiness signal.
