# APP-167 live acceptance, 2026-11-04

The closing release is the GitHub-triggered promotion of `35751a23a41bca0247fd3c067dabae5e754ff55a`.

| Fact | Value |
| --- | --- |
| Run | https://github.com/getforma-dev/forma-app-platform/actions/runs/41884785719 |
| Result | All five jobs succeeded, including Promote to beta and the seal |
| Tag | `beta/20261104-35751a2` |
| Promotion | `37f4ebfc-82e2-4a89-8e09-62df9175e139` |
| Packet | `9852c3749bc57dd56bb072309dea2f8606e3e99ef5d605cba2b7f6cb013e6d36` |
| Activated | `2026-11-04T19:08:36Z` |
| App task definition | revision 23 |
| Broker task definition | revision 12 |
| Canonical status | `verified`, lease idle afterward |

The prepared review preserved schema, template, trial, aliases, and secrets. Its ten changes were six task-definition replacements, two service updates, and two stop schedules. The database was not one of them. The review sequence was stop both services, execute the change set, verify the stopped boundaries, activate both services, then verify health and images. The execute receipt returned `verified` with the revisions and tag above.

After the seal, both services were desired 1, running 1, and rollout `COMPLETED`. `https://beta.getforma.dev/` returned HTTP 200 and showed the canvas home page. `https://beta.getforma.dev/health/ready` returned HTTP 200.

The promotion review still reports policy `schema_version` 19. The database itself was moved to schema 20 earlier on this date, outside this promotion, and this change set did not alter it.
