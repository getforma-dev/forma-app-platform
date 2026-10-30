# fAuth mail image and controlled-inbox trial received (FORMA-366), 2026-10-29

FormaStack reported FORMA-366 Done. GenUI read both receipts in place (read-only, outside this repository) and checked the CI run through the public API.

| Item | Value |
| --- | --- |
| Image | gatewasm `main` `1bf5d3b2fb515c8ed82b0c5a044f7846915e28d0` (PR 20); deployed by digest `sha256:d1365238ef1db7fb7afa759f3201c7550106d2ca79a0ebb239a06814970229a3` at `https://auth-staging.getforma.dev` (the FORMA-367 deployment) |
| CI | gatewasm run 40934743199, `success`, head `1bf5d3b` |
| Local image qualification | FormaStack `work/FORMA-367/image-qualification/local-image-qualification.json`, SHA-256 `d5d863c528b177ad70b3de79ea440920cface6e5a3928fcde7ab724579669758`: invited signup 201 with only `redirect_uri` to the browser, no refresh cookie on registration, backend exchange 200, `email_verified` false before verification, code replay 400, KMS-required startup refused without a manifest |
| Controlled-inbox trial | FormaStack `work/FORMA-367/inbox-trial-receipt.json`, SHA-256 `43cb27b16513657caba698e399e855f56051c117320c7deaf39d3827cd8b1880`: SES SMTP (sandbox) to the SES-verified recipient `victor@getforma.dev`; invitation mail suppressed (`server_only`); verification mail delivered and confirmed by Victor; management API then `email_verified` true; a fresh hosted sign-in exchanged 200 with `email_verified` true, token verified offline with kid `g0jSyT5oFq8`; replay 400; logout 204; throwaway tenant deleted 204 |

Victor accepted the SES sandbox for the MVP, so production access is not required. The verified page showed the no-link fallback because the throwaway client had no `return_entry_url`; registering it per organization waits for APP-009's sign-in handles. The three-browser return coverage stays with FORMA-365 / APP-079. No token, password or identifier other than the public kid is copied here.
