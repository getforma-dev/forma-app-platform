# Release B scan disposition (2026-11-14): CVE-2025-6020 in pam

## Scope

This disposition covers only the APP-194 schema release, 22 → 24. It is bound to:
- that release's `to` deployment contract, `88d17cb53bf77c2f2bfa0b4345953f9f74c0aa39b81b677d6105807a54bcc356`, which equals both the candidate `c40d4535` and this branch;
- the three live images of release A as predecessors (`live.registry.<role>.manifest_digest`);
- an expiry of **2026-11-26 00:00 UTC**, the schema release's own.

It accepts one finding per image: **CVE-2025-6020**, HIGH (CVSS 3.1 7.8, `AV:L/AC:L/PR:L/UI:N`), in `pam` 1.5.2-6+deb12u2. None is CRITICAL and none is removed. The exact fingerprints are in [`scan-disposition.json`](https://github.com/getforma-dev/forma-app-platform/blob/140a5a9f7bfcbea8545ca3410f7c9b6a5ace6d6f/ops/promotion/scan-disposition.json). The source pin in `ops/promotion/registry.py` names this record, the three roles, the expiry and the finding's severity and package.

Any other finding, severity, package, predecessor, contract or later date is refused. Once release B succeeds, the finding becomes an ordinary baseline finding.

This is a temporary acceptance. It is not a package fix and not a scanner suppression.

## What happened

| When (UTC) | Event |
| --- | --- |
| 15:24 | Release B's first preparation started: run [43610002004](https://github.com/getforma-dev/forma-app-platform/actions/runs/43610002004) for the merge `c40d4535` (PR #124). |
| ~16:07 | It published the three images. |
| 16:07:51 | It refused before any template or change set. The lock is held in `preparing` with zero unresolved operations. |
| — | `PROMOTION_ENABLED=false`. |

Each candidate reports 39 findings against the canonical baseline's 38. The baseline is release A's 16 accepted findings on top of its earlier 22. The one introduced finding is the same in every role.

## Method

The sanitized [delta](scan-delta-2026-11-14.json) and the disposition were generated with `ops/promotion/scan_renewal.py` (runbook §8), and nothing was retyped. The inputs were:
- each role's canonical `live.registry.<role>`, read privately from the promotion state;
- each candidate manifest's `describe-image-scan-findings`, with the digests taken from the journal's manifest uploads.

`scan_renewal check` prints `bound`, with 1 finding and `disposition_sha256 e35d6b45…`. Running `registry.guard_scan_delta` itself on the private inputs and this branch's contract gives, for every role: 39 findings, 1 introduced, 1 reviewed, 0 unreviewed, disposition `e35d6b4528c4…`, expiry 2026-11-26.

## Debian status

Read from the security tracker on 2026-11-14 at about 16:15 UTC:
- **bookworm `1.5.2-6+deb12u2`:** vulnerable.
- **trixie `1.7.0-5`:** fixed.
- **forky and sid `1.7.0-8`:** fixed.
- **bullseye:** fixed by DLA-4306-1.

The tracker notes "Incomplete fix in pam/1.5.2-6+deb12u2: Patch not applied correctly during build", which is why deb12u2 is listed as vulnerable. There is no bookworm-security row and no DSA. No bookworm fix exists today, so waiting is not an option before the expiry. Pinning a newer `pam` would change the Dockerfiles and therefore the deployment contract.

## Applicability

The flaw is in `pam_namespace`. A *local* user can escalate to root through symlink races in paths that module handles. It is reached only when a PAM-aware program (`login`, `su`, `sshd` and similar) runs a PAM stack that loads `pam_namespace`.

The pinned base of all three final stages, `python:3.11.14-slim-bookworm@sha256:65a93d69…`, was pulled for linux/arm64 (Debian 12.13) and inspected. It was found by the independent review of PR #125:
- it carries `libpam-modules`, `libpam0g` and `libpam-runtime` 1.5.2-6+deb12u2, and `pam_namespace.so`;
- its 16 `/etc/pam.d` files are Debian's defaults, and **none references `pam_namespace`**;
- `login` and a setuid `su` exist; there is no `sshd` and no `sudo`.

None of the three Dockerfiles installs or upgrades a pam package, or touches `/etc/pam.d`, `/etc/security` or user accounts. Their `--only-upgrade` lists contain no `libpam*`. So the final images' PAM configuration is the base's.

The three images run as `USER 65532:65532`. Their entrypoints are `python3 /app/runtime.py serve`, the operator's `python3 -I -u …/operator.py` and the qualification supervisor. None of them invokes PAM or starts `login` or `su`.

Every container also has a read-only root filesystem (`infra/src`). There is no shell or login service, and no interactive access path.

## Limits

- The pinned base image was inspected, not the ECR candidate images. Their PAM configuration comes from that base unchanged, per the Dockerfiles.
- The check covers static configuration and entrypoints, not `dlopen`.
- **Correction.** An earlier draft of this record cited `forma248-local:qualification` as "a local build of the qualification image". That local image is a GateWasm server build, not this repo's image. The same misidentification appears in APP-175's records; see APP-175's Log.

## Approval

For each image, the beta approval summary must show:
- **1 introduced, 1 reviewed, 0 unreviewed**, with CVE-2025-6020;
- this disposition's content hash, `e35d6b4528c4bc02df87b54c20e1e1f7de2af69e95e8d12b0e73ab848d3bff6b`;
- the expiry, 2026-11-26.
