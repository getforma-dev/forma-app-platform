# Release B scan disposition (2026-11-14): CVE-2025-6020 in pam

## Scope

This disposition covers only the APP-194 schema release, 22 → 24. It is bound to:
- that release's `to` deployment contract, `88d17cb53bf77c2f2bfa0b4345953f9f74c0aa39b81b677d6105807a54bcc356`, which equals both the candidate `c40d4535` and this branch;
- the three live images of release A as predecessors (`live.registry.<role>.manifest_digest`);
- an expiry of **2026-11-26 00:00 UTC**, the schema release's own.

It accepts one finding per image: **CVE-2025-6020**, HIGH (CVSS 3.1 7.8, `AV:L/AC:L/PR:L/UI:N`), in `pam` 1.5.2-6+deb12u2. None is CRITICAL and none is removed. The exact fingerprints are in [`scan-disposition.json`](../../ops/promotion/scan-disposition.json). The source pin in `ops/promotion/registry.py` names this record, the three roles, the expiry and the finding's severity and package.

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

No bookworm fix exists, so waiting is not an option before the expiry. Pinning a newer `pam` would change the Dockerfiles and therefore the deployment contract.

## Applicability

The flaw is in `pam_namespace`. A *local* user can escalate to root through symlink races in paths that module handles. It is reached only when a PAM-aware program (`login`, `su`, `sshd` and similar) runs a PAM stack that loads `pam_namespace`.

A local build of the qualification image on the same `python:3.11.14-slim-bookworm` base (`forma248-local:qualification`) shows:
- `libpam-modules` 1.5.2-6+deb12u2 and `pam_namespace.so` are present;
- **no file in `/etc/pam.d` references `pam_namespace`**, so no PAM stack loads it;
- `login` and `su` exist, from the base image;
- there is no `sshd`.

The three images run as `USER 65532:65532`. Their entrypoints are `python3 /app/runtime.py serve`, the operator's `python3 -I -u …/operator.py` and the qualification supervisor. None of them invokes PAM or starts `login` or `su`.

Every container also has a read-only root filesystem (`infra/src`). There is no shell or login service, and no interactive access path.

## Limits

- The image checked was an earlier local amd64 build, not the ECR ARM64 images. The base and package list are the same, by Dockerfile.
- The check covers static configuration and entrypoints, not `dlopen`.

## Approval

For each image, the beta approval summary must show:
- **1 introduced, 1 reviewed, 0 unreviewed**, with CVE-2025-6020;
- this disposition's content hash, `e35d6b4528c4bc02df87b54c20e1e1f7de2af69e95e8d12b0e73ab848d3bff6b`;
- the expiry, 2026-11-26.
