# GenUI beta: brief for a UI/UX designer

Date: 2026-10-30. Product: the Forma app platform ("GenUI"), live at `https://beta.getforma.dev` as the November 2 MVP ([ADR-010](../architecture/ADR-010-NOV-2-MVP-CUT.md)). The source of truth for every screen is the server-rendered TSX in `app/admin/src/*/app.tsx`, the Rust handlers under `app/src/`, and the stylesheet `app/admin/src/styles/app.css`. The pages work today but have had no visual design: there are no mocks and no style guide. Screenshots of the current anonymous pages are in [`current/`](current/).

## 1. The product in one paragraph

Forma lets a company build, run and share small internal apps around its own data. The beta is **invite-only**: a company team (in the demo story, the synthetic organization **Northstar**, with its Security and Finance spaces) gets invitation codes, creates accounts and signs in. The beta proves three things, and the design has to make each one easy to see. **(1) Verified sign-in:** an invitation code, sign-up with a confirmed mailbox and an organization sign-in link lead to a workspace that lists only what this member may open. **(2) A governed query with a visible receipt:** the member opens a saved work (Northstar's outbound investigation), picks a time window and runs two views, `total` and `timeline`. Every number on screen comes with a receipt naming the app, source revision, operator version, parameters, policy and record-snapshot identities, and the result digest. When a view fails, it says so and shows no number. **(3) An explicit command with an atomic, audited outcome:** in a second app, Spend Review, the member records a decision (`status`, `note`) on an expense. The decision commits once with its version and audit. A retry returns the stored outcome, and a stale edit is a visible conflict. Sources and definitions are **seeded by the operator** and labeled that way. Nothing on screen is presented as AI-generated, and the beta has no import, authoring or sharing screens yet (acts 2, 3 and 5 of the six-act demo are out of scope).

## 2. Screens to design

Below, "notice" means the existing `.notice` block, `role="alert"` for errors and `role="status"` for confirmations. Copy in quotes is current copy. Copy marked **(keep)** carries a product or security claim: change its typography if you like, but not its meaning.

### 2.1 Landing `/`
- **Purpose:** the public front door. It explains that the beta is by invitation and offers two ways in.
- **Data:** none per user. Title "Forma app platform", a lede ("…The beta is by invitation only: enter the code you were sent to create your account, or sign in with your organization's link." **(keep the invitation-only meaning)**), links **Enter an invitation code** → `/invite` and **Sign in** → `/signin`, and a footer link to `/about`.
- **States:** one.
- **Note:** the page still carries a framework demo counter ("Count: 7", Increment/Reset). It is the only interactive island in the build and the only page that ships JavaScript (28,168 B gzip). **Do not design it.** We expect to remove it (see §6), so treat the landing as a static marketing page.
- Screenshots: `current/landing-desktop.png`, `current/landing-mobile.png`.

### 2.2 Invitation entry `/invite` (form posts to `/invite/validate`)
- **Purpose:** redeem an invitation code before sign-up.
- **Data:** heading "Enter your invitation code", a lede (**keep:** a code works only for the organization it was issued for), one text field labeled "Invitation code" (`name="code"`, max 128 characters, autofocus, monospace today), help text "Dashes and spaces are ignored. After this step you create your account with the email address the invitation names.", a **Continue** button, and links to sign in and back home.
- **States (at most one notice at a time, placed above the form, wired to the field by `aria-describedby`):**
  - default
  - refused (400): "This code could not be used. It may be mistyped, expired, revoked or already used…" **(keep, uniform).** The same message covers every reason, and the code the user typed is **never echoed back**.
  - limit, minute: "Too many attempts from your network in the last minute…"
  - limit, day: "Too many attempts from your network today…"
  - limit, service: "Too many attempts across the service today…"
  - unavailable (503): "The invitation entry is unavailable right now…"
- **Primary action:** Continue. On success, the server redirects to sign-up with the identity provider, which is an external page outside this brief.
- Screenshots: `current/invite-*.png`, `current/invite-refused-*.png`.

### 2.3 Sign-in `/signin/{handle}` (and `/signin`)
- **Purpose:** start sign-in for an organization. The handle is an opaque 26-character code inside the organization's sign-in link, for example `/signin/aaaaaaaaaaaaaaaaaaaaaaaaaa`.
- **Data:** heading "Sign in". When the handle is well-formed, the page shows one button, **Continue to sign in** (a form posting to the same URL). Otherwise it shows "Open your organization's sign-in link to continue." The page **looks identical for known and unknown organizations**, and no organization name is ever shown.
- **States:** form shown; no handle (`/signin` with no hint); neutral failure (400) with the notice "Sign-in could not continue. Start again from your organization's sign-in link." **(keep, uniform)**.
- **There is no organization picker and no email field.** Organization comes only from the link.
- Screenshots: `current/signin-handle-*.png`, `current/signin-failed-*.png`. The button is unstyled today.

### 2.4 Verification pending `/auth/pending` (session required, so no screenshot)
- **Purpose:** the identity is verified but the mailbox is not yet confirmed. This page reads nothing from the workspace.
- **Data:** heading "Confirm your email address", lede "Your sign-in worked, but your email address is not confirmed yet. Open the confirmation link…". The following elements are optional:
  - "Your invitation is held for **N** more hours." (`held`, `hoursRemaining`)
  - the resend form button **Request a new confirmation message**, or, during the cooldown, "You can request another message in **N** seconds."
- **States (notices are `role="status"`):** default; held; resend available or cooling down; "A new confirmation message was requested. The previous link no longer works." **(keep: it says "requested", never "sent" or "delivered")**; "A new message could not be requested right now. Try again later."
- **Actions:** Request a new confirmation message; **Sign out** (always shown).

### 2.5 Denied and neutral errors
- **Denied, no access (403, HTML; rendered by `/auth/callback`):** heading "No access", "This account has no current access here. Sign out to continue with another account." and a **Sign out** button. **(keep)** It must not say why access is missing (no membership, no entitlement, expired or revoked) and must not name an organization.
- **Not found (404):** any unknown path, and **every protected route without a valid session**, returns the same response. That covers anonymous, expired, revoked, foreign-organization and nonexistent resources alike. Today this is a raw `application/problem+json` body, `{"code":"not_found","status":404}`, with no HTML (`current/not-found-404-*.png`). **Please design a neutral HTML 404:** one message, no hint about whether the thing exists or whether the user is signed in, and a link home. A **503 "unavailable"** page in the same style is optional. These pages are new work on our side (see §6).

### 2.6 Workspace home `/workspace` (session required)
- **Purpose:** after sign-in, list the saved works this member may open now.
- **Data:** heading "Workspace", "You are signed in.", and the section "Saved work". Each item shows:
  - its title, linking to `/w/{app}/{work}`
  - the label "Seeded definition, retrieved from the library. Sources seeded by the operator." **(keep, verbatim meaning)**
  - when the member may write records, sub-links such as **New decision record** (label pattern "New {schema} record"), each opening a fresh record page
- **States:** has items; empty ("Nothing to open yet." with no hint at what exists elsewhere).
- **Actions:** open a work; open a new record; **Sign out**; **Sign out everywhere** (two separate forms).
- **Not in scope:** spaces, search, recents, thumbnails, app creation and an organization switcher. They come in later phases, so leave room in the header, but do not show them.

### 2.7 Work page `/w/{app}/{work}` (session required): Northstar run
- **Purpose:** run the saved work's two views over a window and show each result with its receipt.
- **Header:** the work's title and the same seeded label as on the workspace **(keep)**.
- **Window form (POST to the same URL):** fields, names fixed:
  - **Window start (UTC)** `window_start`, `datetime-local`, seconds step
  - **Window end (UTC, excluded)** `window_end` **(keep "excluded": the window is half-open)**
  - **Destination (optional)** `destination`, free text
  - hidden `csrf` and the button **Run views**
  - Form error notice: "The window was not accepted. Give a start before the end, as UTC date and time. Nothing ran."
- **View cards:** one `<section>` per view, `data-state` is set on each. The views are `total` (a summary: one large number, for example **1,140,000** total bytes) and `timeline` (a table). Each card has a title, a **badge**, a message line and, depending on its state, a value and a receipt:

| State (badge) | Message (server copy) | Value | Receipt |
|---|---|---|---|
| idle (`idle`) | "Not run yet. Choose a window and run the views." | none | none |
| busy (`busy`; internally `loading`) | "Busy: the query engine has no room for this run yet. Run again in a few seconds. Nothing is shown until the result is computed." | none | none |
| empty (`empty`) | "No rows matched this window and filter." | none (**never "0"**) | yes |
| ready (`ready`) | none, or "Showing the first page of rows." | summary number or table | yes |
| failed (`failed`) | one of six reasons, each ending "**No number is shown.**" (for example past budget, source revision unavailable, operation not qualified, result larger than one page, store unavailable) | none | none |
| denied (`denied`) | "You do not have access to this result. Nothing is shown." | none | none |

  - **Rule (keep):** a failed, denied, busy or idle card must show nothing that reads as a number: no zero, no placeholder digits, no skeleton bars shaped like values. The engineering name for this rule is "never a fabricated zero"; the user-facing copy is "No number is shown." Empty must look different from failed, and denied must look different from failed.
  - A table cell with no value reads "unknown", never 0. Integer columns are digit-grouped by the server.
- **Receipt (ready and empty only):** today it is a collapsed `<details>` whose summary reads "Receipt `<digest>`". Expanded, it lists these rows in fixed order: Receipt digest, Result digest, Mode, App, Saved work, Publication, Definition digest, View, Operator version, then one "Source revision (input)" row per source ("source … revision … digest …"), then one "Parameter {field}" row per parameter, then Record snapshot sequence, Principal, Membership epoch, Policy epoch, Filter epoch, Query digest, Effective query digest, Total scope, Partial and Truncated page, followed by the raw JSON document. Values are long hex digests and UUIDs, so plan for wrapping and monospace. The digest must stay visible without expanding. Whether the rows are open by default is your call (we suggest collapsed rows with the digest and key identities always visible).
- **Behavior:** after a successful run the server redirects back to this page, so a reload restores the cards without a resubmit. Link back to **Workspace**.

### 2.8 Record page (Spend Review) `/work/{app}/{work}/records/{collection}/{record}` (session required)
- **Purpose:** show one expense decision record (for example EXP-002) and offer the one command that applies (`record_decision`).
- **Data:** heading "Record" (we would welcome a better title, such as the schema plus record id, if it uses data already on the page). A lede with the seeded label, the schema and version, and the record id **(keep the seeded label)**. Then a version line, one of:
  - "Record version **N**; work version **M**."
  - "No record committed yet (version 0); work version **M**." This covers the absent record; the Northstar scenario calls it "Pending".
- **Read-only table:** one row per field: field id, current value, type (for example `status`: `needs_context`, enum; `note`: text).
- **Decision form** (posts to `/v1/work/{app}/{work}/commands/{command}`, only when the member may write):
  - per field: a label with the field id and its allowed options ("needs_context | reviewed")
  - a text input named `value.{field}` if this command writes the field; otherwise the muted text "not changed by this command"
  - the button **Submit {command}**
  - hidden fields that must survive any redesign: `csrf`, `idempotency_key`, `collection_id`, `publication_id`, `schema_id`, `schema_version_expected`, `record_id`, `expected_record_version`, `expected_work_version`, and `spec.{field}` per writable field
  - The enum is a free-text input today. A native `<select>` or radio group with the **same `name` and option values** is welcome.
- **Read-only member:** no form, and the muted line "You can read this record but not change it."
- **Outcomes** (the server redirects to this page with `?notice=…`; at most one notice; the record is always shown at its current version):
  - committed (status): "The command was committed. The record is shown at its new version."
  - replayed (status): "This command had already been committed; its stored outcome was returned." This must not look like a second success. Nothing new happened.
  - conflict (alert, the 409 case): "The record changed before your command: it is shown as it is now. Review it and submit again." On this page the **recovery view is the record itself, re-rendered at its current version** with a fresh form, so design the notice plus the refreshed record as one screen. The JSON API also returns `recovery_view` with the 409; the page does not show that body.
  - refused (alert): "The command was refused: a value is not one this command accepts. Nothing was changed." **(keep "Nothing was changed")**
  - stale: "The saved work moved to a newer revision…"
  - in flight: "The same command is still being committed. Reload in a moment."
  - busy: "The record was busy…"
  - unknown: "The result of your last command could not be confirmed. The record is shown as it is now."
- **Link:** Back to the workspace.

## 3. Hard constraints

- **Server-rendered only, with no client JavaScript.** Every page is HTML from the server, every action is a normal `<form method="post">`, and every outcome is a full-page redirect. No client-side validation, toasts, modals that need JS, live spinners, date pickers beyond the native input, or optimistic UI. Use `<details>`, `:focus-visible` and `:target` freely, because they need no JS.
- **Size budgets** (the build's size report, from `npm run build` in `app/admin`): Forma code **28,420 B** and a page's JavaScript **76,800 B** (gzip), and **none** for a route with no interactive region. Every page in this brief except the landing currently ships 0 B of JS and must stay at 0. CSS has no stated budget: keep it to one small hand-written stylesheet, with no CSS framework.
- **Content-Security-Policy** (sent on every page by `forma-server` 3.2, `src/csp.rs`):
  - `default-src 'none'`
  - `font-src 'self'`, so **no Google Fonts or other external fonts**. A custom font must be self-hosted under `/_assets`, and a system font stack is preferred within budget.
  - `style-src` nonce plus `'self'`, so **no external stylesheets and no `<style>` blocks**. All CSS goes in `app.css`.
  - `script-src` nonce plus `'strict-dynamic'`, so no third-party scripts, analytics or embeds.
  - `img-src 'self' data:`, so images and illustrations are self-hosted files or inline data URIs, and inline SVG markup is fine.
  - `frame-ancestors 'none'` and `object-src 'none'`, so no iframes or embeds.
  - `form-action 'self'`, so forms post only to the same origin. Sign-in hands off to the identity provider by a server redirect.
- **Accessibility:** WCAG 2.2 AA.
  - Visible `:focus-visible` on every link, input and button. Today only the invite form and the landing links have it.
  - A keyboard order that follows reading order.
  - Every input with a visible `<label>`.
  - Text contrast of 4.5:1 or more, and **3:1 or more for input borders and other UI edges**. The current borders measure 1.37:1 (light) and 1.34:1 (dark), and the failed/denied red `#b3261e` is 2.72:1 on the dark background. All three fail.
  - State must never be shown by color alone: the badge text stays.
  - Respect `prefers-color-scheme` (both themes exist today) and `prefers-reduced-motion`.
- **Neutral errors:** no screen may reveal whether an account, invitation code, organization, work or record exists, or why access was refused. One message per class, and the user's input is never echoed back.
- **No organization picker** and no organization name on anonymous pages. The organization comes only from the invitation or the sign-in link.
- **Receipts and denials must stay visible and distinct:** a card's state badge and message are always visible. Ready, empty, failed, denied, busy and idle must be distinguishable at a glance, in grayscale too, and the receipt digest is visible on every ready and empty card.
- **Honest labeling:** keep "Seeded definition, retrieved from the library" and "Sources seeded by the operator." Use no "AI", "generated" or sparkle motifs.

## 4. What can change and what cannot

| You can change freely | You cannot change |
|---|---|
| Layout, grid, widths and responsive behavior | Route structure and URLs (§2 headings) |
| Typography: system stack or a self-hosted font within budget; type scale | Form `action`s, `method="post"`, and every field `name` (`code`, `window_start`, `window_end`, `destination`, `value.*`, `spec.*`, all hidden fields) |
| Color, theme and tokens (light and dark) | The uniform 404, neutral denial and uniform invite and sign-in refusals |
| Spacing, borders, radius, card styling, badge design | Which states exist per screen, and that each shows at most one notice |
| Empty-state illustrations (self-hosted SVG or PNG, small) | "No number is shown" on failed, busy, denied and idle cards, and no placeholder digits |
| Ordering and grouping of the receipt rows' presentation (keep the rows) | The meaning of copy marked **(keep)** |
| Swapping the enum text input for a `<select>` or radios (same name and values) | Client-side JavaScript; external fonts, CSS or scripts |
| A site header or footer (logo as a self-hosted SVG) with no navigation to unbuilt features | Adding an organization picker, account lookup or "did you mean" hints |
| Better non-load-bearing copy (titles, ledes) | |

## 5. Deliverables requested

1. **Frames:** desktop (1280) and mobile (390) for every screen in §2 and **every state** listed. That means, at minimum: invite ×6; sign-in ×3; pending ×5; denied; 404; workspace ×2; work page with each card state (ready-summary, ready-table, empty, failed, denied, busy, idle) plus the form error and an expanded receipt; record page as absent, existing, read-only and each of the 8 notices. Light and dark for at least the work page and the record page.
2. **Token set** that maps one-to-one to CSS custom properties on `:root` (today: `--bg`, `--fg`, `--muted`, `--accent`, `--border`):
   - colors, light and dark, including semantic tokens for success, error, warning and info notices and for each badge state, with contrast ratios noted
   - a type scale of 5–6 steps, with family, weights and a monospace for digests
   - a spacing scale
   - radius, border width and the focus ring
3. **Component specs:** button (primary, secondary, quiet), text input, `datetime-local`, select, notice (alert and status), badge (6 states), card, receipt row, key-value list, data table, link.
4. **Format:** Figma with components and variables for the tokens, plus exported specs (Dev Mode or a PDF/PNG export) with redlines for spacing and states. Export illustrations as optimized SVG.

## 6. Known gaps the design will expose (engineering, not designer, work)

- The landing still ships the framework demo counter island (28 KB of JS). We expect to remove it so that the landing ships no JS either.
- 404 and 503 are JSON bodies, with no HTML page.
- Buttons outside `.entry` and `.run` (sign-in, pending, denied, workspace sign-out, record submit) have no styles or focus ring.
- The record page heading reads "Record", and the enum field is a text input.
- On the record page, the 409 recovery view is the re-rendered record, not a separate view.

## Current screenshots (`current/`)

The screenshots were captured on 2026-10-30 from a local build of this tree (`npm ci && npm run build` in `app/admin`, `cargo build` in `app`). The binary was run as `app/runtime/smoke_http.py` runs it: a synthetic credential file, no database, and no identity provider. Headless Chrome followed the system dark color scheme. The mobile frames are 390×844.

| File | Shows |
|---|---|
| `landing-desktop.png`, `landing-mobile.png` | `/` |
| `invite-desktop.png`, `invite-mobile.png` | `/invite`, default |
| `invite-refused-desktop.png`, `invite-refused-mobile.png` | `/invite/validate` uniform refusal (400) |
| `signin-handle-desktop.png`, `signin-handle-mobile.png` | `/signin/{well-formed handle}` with its form |
| `signin-failed-desktop.png`, `signin-failed-mobile.png` | sign-in neutral failure (400) |
| `not-found-404-desktop.png`, `not-found-404-mobile.png` | uniform 404 (raw JSON today; mobile frame clipped) |

The authenticated pages (pending, denied, workspace, work, record) need a database session. They are described from their TSX and the Rust view models (`app/src/query/page.rs`, `app/src/commands/mod.rs`, `app/src/auth/routes.rs`) instead of being captured.
