# Implementation phases

Each phase groups sprints around a concrete outcome. Tickets remain in their status folders; these pages are views, not duplicate boards. Complete a phase only after all its tickets are Done and its exit evidence is accepted in `plan.json`.

Dependencies control individual ticket selection. Independent work may proceed while an earlier integration gate waits on FormaStack; phase completion still requires predecessor phase gates.

| Phase | Sprints | Tickets | Done | Blocked | Progress | Outcome |
|---|---|---|---|---|---|---|
| [PH01: Foundations and first saved workflow](phases/PH01.md) | [S01](sprints/S01.md), [S02](sprints/S02.md) | 14 | 2 | 3 | active | One authenticated user can query authorized data, inspect its receipt, save a decision and reopen it safely. |
| [PH02: Working data and workspace](phases/PH02.md) | [S03](sprints/S03.md), [S04](sprints/S04.md), [S05](sprints/S05.md) | 17 | 0 | 0 | planned | People import real files, get correct traceable results, use accessible views/forms, and return to saved work. |
| [PH03: App creation, governance and AI](phases/PH03.md) | [S06](sprints/S06.md), [S07](sprints/S07.md), [S08](sprints/S08.md) | 16 | 0 | 0 | planned | Users create, change, review, publish and generate supported apps without platform code changes. |
| [PH04: Complete two-app demonstration](phases/PH04.md) | [S09](sprints/S09.md), [S10](sprints/S10.md) | 10 | 0 | 0 | planned | Northstar and Spend Review complete the designed shared-work, independent-reuse and recovery journey on one unchanged build. |
| [PH05: Managed pilot controls](phases/PH05.md) | [S11](sprints/S11.md) | 9 | 0 | 1 | planned | The hosted platform has controlled deployment, service identities, audit, support access and data lifecycle mechanisms. |
| [PH06: Qualification and pilot learning](phases/PH06.md) | [S12](sprints/S12.md) | 6 | 0 | 0 | planned | Measured operational evidence supports a controlled pilot, followed by observed customer use and independent reuse. |
