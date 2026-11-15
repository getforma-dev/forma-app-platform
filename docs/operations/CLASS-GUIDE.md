# Class guide: build day, rehearsal, known issues, code email

**The class is remote** (Victor, 2026-11-15): students join from home, school, a library or anywhere else. Where this guide says "on my screen" or "the projector", share your screen in the call; where it says "pin it in the room", send it to the students. What is left, with dates: [POST-CLASS-FOLLOW-UPS.md](../planning/POST-CLASS-FOLLOW-UPS.md).

Victor's class-prep pack for the Nov 29–30 class (25 adult students, his organization on `beta.getforma.dev`). It follows the [plan](../planning/TRIAL-25-READINESS.md) and the [runbook](TRIAL-RUNBOOK.md); it adds nothing to either. Every quoted message is the text the code shows, read on 2026-11-14 from `main` (release B, schema 24, [live on beta since Nov 14](../../evidence/APP-194/release-2026-11-14.md)) and from the Release C branch (`release/c-participation`, with the composer step from `release/c-b5-composer`). Where this guide says **Release C**, the words work only after Release C is live on beta; each one has a fallback that works on release B.

What the two releases give you, in plain words:

- **Release B (live on beta since Nov 14).** Private lists ("each person sees only their own entries"; you and your reviewers see them all), reviewer-only commands (only you can grade), names, the `/admin` page, a daily AI cap, and sharing with everyone in the organization with a role.
- **Release C (about Nov 25; not a gate).** One changeable entry per person per item (a second answer replaces the first), items you open and close, answers accepted only while an item is open, a countdown, a pick-list for "which item", and a ranking by person on your screen. Designed in ADR-020 (`docs/architecture/ADR-020-PARTICIPATION-PRIMITIVES.md` on the Release C branch). If it slips past Nov 26, the class runs on release B as the plan first said.

## Before you start

**The two accounts.** You build with a fresh alias of your own, and you check with a **helper account**: a second invitation code, redeemed in a **private (incognito) window** so the two sessions never share a cookie. One rule about codes: **a code works only after the next service restart** (the service reads its codes when it starts; runbook section 4). A Release C deploy counts as a restart. When no deploy comes before you need a code, the operator issues the code and then does one announced restart (`admission restart --announced`: everyone is signed out for a minute, and one restart covers every code issued before it). So the order before Nov 24 is: your build account's code and the helper's code are issued, then a restart, then you redeem both and sign in. The 2–3 rehearsal students' codes are issued before whichever restart precedes Nov 26. The 25 student codes stay with the Nov 28 restart, as the runbook says.

**Your admin row.** `/admin` opens only for an account that holds the admin grant (runbook section 3 step 7, `admission grant-admin`). The grant names an account that has already signed in, so the order on the morning of Nov 24 is: redeem your code and sign in once with the account you will build with, ask the agents for the grant on that account, then open `/admin`. Use that one account for building and for `/admin`: the apps belong to the account that built them. On `/admin`, your row then reads `Admin · you` with `Notice v1` and the date you redeemed; a row whose membership was seeded by the operator (the original owner account, if it is still there) reads `Not recorded` in the consent column. Every student's row shows `Notice v1` and a date.

**How you type an app.** On your canvas, the bar at the top says `Describe an app, ask a question, or type /`. Type one sentence of 3 to 500 characters and press Enter. You will see `Understanding…`, then `Composing your app…`, then `Placing “<title>” on your canvas…`. If instead a card asks you to confirm a record (`Review the record, then confirm.`), Forma read your sentence as an entry for an app you already have: press **Cancel** and reword so the sentence describes a new app, as the sentences below do. Your organization has no library apps, so a sentence always builds something new; if you ever see `A few apps fit. Pick one, or build something new.`, choose **Build something new**. (The **Create an app** dialog is the template path: it asks you to pick a template first, so don't use it for these apps.) If the model's answer is refused you see `That request could not be composed into an app. Nothing was placed.`; reword and try again. A sentence that is too short or long gets `Describe what you need in a few words (3 to 500 characters). Nothing was placed.`

**What you get.** An app card on the canvas with its forms, and an **Open app page** link. The app page is the clean view for you and for the projector; it has a **Refresh** button. Entries show in a Records card with a **Filed by** column, which is how you see who checked in or submitted; on the app page each row has an **Open record** link to that record's own page. Every form's button is named after its command, so the Close form's button reads something like **Close** or **Close question**; this guide writes **Post**, **Close**, **Reopen** and **Grade** for short. A form that changes an existing entry has a chooser above it, `<form title>: choose a record`, listing each entry as `<its first text value> · revision <n>` (with Release C's APP-211, as `<student's name> · <text> · revision <n>`), and a button **Open record in this form**. Your description is prefixed `Generated by Forma.`

**What the composer can and cannot write.** It writes typed app definitions only; nothing it returns is code. On release B it can write: private lists, reviewer-only commands, "which X" as a short text or a fixed set of choices, and typed summary views (a count, a table, a chart) over one list. It cannot write, on release B: one entry per person (a second submission is a second row), a due time or timer, a pick-list of items, or a reference from one list to another (a form would show an id box, so such content is refused). Release C adds all of those. Neither release lets a sentence name an approver, a role, a permission or a person; roles are set in the share dialog.

**Three rules for 25 open pages** (the plan's class guidance, kept here so it is at hand):

1. Anything students write goes in a private list (answers, submissions, check-ins). Their saves then update only your page and their own.
2. Shared lists are yours to write. Post or close one thing, then wait about ten seconds before the next change to the same page.
3. One live page on the projector: your own app page. Students don't need to keep a page open after submitting; a page they come back to updates when they look at it.

## 1. Build day (Nov 24, and Nov 25 for the Release C apps)

*2026-11-15:* Victor will merge a new app builder platform, so the class apps are not built on this platform now; the Nov 24 build day waits for that merge (the plan's calendar says the same). The words below stay as written for when it happens.

For each app: the words to type, what the result should contain, how to check it with the helper account, what needs Release C, and the fallback. Type the words as written; swap the session names, question labels and choices for yours. Each check is "yes" before you move on; a "no" is a rebuild with clearer words, not a fix by hand (you cannot edit a definition).

Two Release C details come from APP-211, which is being built for Release C: the entry chooser shows the student's name first, and a student who opens a card that scores entries against an answer key only reviewers can read (the quiz leaderboard) sees `Scores are shown to this app's reviewers.` instead of a number; counts of a student's own entries still show. The passages that depend on it are marked (Release C, APP-211).

### (a) Attendance: a private check-in per session, one per person per session, you see all

**Release B words (Nov 24):**

> Attendance check-in: each person checks in with the session they are attending (Saturday morning, Saturday afternoon, Sunday morning, Sunday afternoon) and an optional note, and sees only their own check-ins. Show how many check-ins there are per session.

**The result should contain:** one private list of check-ins; a form (a **Session** choice with your four sessions and a **Note** box); exactly one summary view counting check-ins per session; a Records card. There should be no id box in the form.

**Helper check:**

1. Helper (private window): open the shared app, check in to "Saturday morning". The form says `Saving…`, then `Saved · Commit <n>. Refreshing the app…`.
2. Helper: the Records card shows only the helper's own row; the count card shows 1.
3. You: your Records card shows the helper's row with the helper's name under **Filed by**, plus your own test check-in; your count card shows both.
4. Helper: check in again to the same session. On release B this is a **second row**; both count. That is the known limit, not a defect.

**Release B limits (tell the students):** a second check-in is saved too and both count; you take the newest. Nothing closes a session, so a late check-in counts.

**Release C words (Nov 25).** The composer's own worked example for this app is the short sentence `attendance check-in once per session`: it was checked against the composer's rules with sample replies (a sessions list you open and close, one check-in per person per session, a check-ins-per-person ranking), not yet against the real model. Try it once on build day and use the release B fallback if it is refused. A fuller sentence that stays inside the same shapes:

> Attendance: only I post and close each session. Each person checks in once per open session and can change it until I close the session. Show check-ins per person on my screen.

Check the result against the list below before you use it; if the shapes differ, rebuild with clearer words.

**What changes:** the form shows a **pick-list of sessions** (the newest one preselected) instead of a fixed choice; a second check-in **replaces** the first; after you press **Close** on a session, a check-in says `Not saved: the item this refers to is no longer open.`; the same check-in twice says `Already locked in: your entry already holds these values.` The check-ins per person card lives on your page and counts everyone; on the helper's own page the same card counts only the helper's own check-ins. It is not a score against an answer key, so APP-211's `Scores are shown to this app's reviewers.` does not apply here.

**Release C helper check:** post a session (its form is yours only; the helper must not see a post form), helper checks in, checks in again (one row, version 2), you close the session, helper's next check-in is refused with the message above. Close a session before you post the next one: the pick-list's default follows the item you touched last.

**Fallback if Release C slips:** the release B app above, with the two limits.

### (b) Homework: shared assignments you post, private submissions, one per assignment the student can change until the due time, reviewer-only Grade

**Release B words (Nov 24):**

> Homework: only reviewers post assignments (a title, instructions and a due date) that everyone can see. Each student submits their work for an assignment (which assignment, and the work as text) and sees only their own submissions. Only reviewers can grade a submission with a grade and a comment.

**The result should contain:** a shared assignments list with a reviewer-only **Post** form; a private submissions list with a **Submit** form whose "which assignment" is a short text box or a fixed choice (never an id box); a reviewer-only **Grade** form that writes a grade and a comment; two Records cards. A reviewer-only form is shown only to reviewers, so the helper's page must have **no Post form and no Grade form**.

**Helper check:**

1. You: post "Assignment 1". Wait ten seconds before anything else on this page.
2. Helper: sees "Assignment 1" in the assignments Records card. Submits work for it. Sees only their own submission.
3. Helper: there is no Grade form and no Post form on the helper's page. (If there is, rebuild with "only reviewers" in front of the verb.)
4. You: on your app page, above the Grade form, `Grade: choose a record` lists each submission; pick the helper's, press **Open record in this form**, enter a grade and a comment, press **Grade**. On release B the chooser labels each submission `<its first text value> · revision <n>` and shows no names, so when two submissions read alike, open the helper's row with **Open record** from the Records card on your app page (it shows **Filed by**) and grade on that record's page, whose button reads `Submit <command>` (the command's id, for example `Submit grade`); in class on release B, ask students to begin their work text with their name, so the chooser reads as a class list. With Release C the chooser reads `<student's name> · <text> · revision <n>` and that workaround is not needed (Release C, APP-211). If the chooser says `Choose a record that this command can change. No applicable record is available yet.`, no submission exists yet.
5. Helper: press **Refresh**; their own submission now shows the grade. They cannot see your test submission.
6. Helper: submit again for the same assignment. On release B this is a second row.

**Release B limits:** "change until the due time" is not enforced. A student changes a submission by submitting again; both rows count in summaries and you grade the newest. The due date is text on the assignment, not a rule.

**Release C words (Nov 25).** The composer's worked example is `homework submissions, one per assignment, reviewer-only grade`: it was checked against the composer's rules with sample replies (a tasks list with a reviewer-only post, one submission per person per task that replaces on resubmit, and a reviewer-only grade that writes only the score), not yet against the real model. Try it once on build day and use the release B fallback if it is refused. Add the closing words so the due time is enforced:

> Homework: only I post and close assignments (a title and instructions). Each student submits once per open assignment and can change the submission until I close the assignment. Only reviewers grade a submission with a grade and a comment.

**What changes:** the Submit form has a pick-list of assignments; a second submission replaces the first (same entry, new version); after you **Close** an assignment, a submission says `Not saved: the item this refers to is no longer open.` Grade is an update that writes the grade, never which assignment.

**On "the due time":** the recommended due time, and the one the composer's rules cover, is you pressing **Close** (or **Reopen**) on the assignment. The grammar can also compare an assignment's own due timestamp with the server clock, but no sentence for that was tested, so don't rely on it for the class.

**Fallback if Release C slips:** the release B app, and you say in class: "submit again to change your work; I grade the newest".

### (c) A formative quiz: private answers and a live summary you project (release B)

**Words (Nov 24):**

> A quiz: each person answers a question (q1, q2, q3, q4, q5) by choosing a, b, c or d, and sees only their own answers. Show how many people chose each answer for each question.

**The result should contain:** one private answers list; an **Answer** form with a **Question** choice and a **Choice** choice; **exactly one** summary view (a table of counts by question and choice). The plan allows at most one typed query view on a page students open; if the composer gives you two or three summary cards, rebuild with "show only one table of counts".

**Helper check:** helper answers q1 with b; helper's page shows only their row, and their table shows only their own count; your table shows every answer. Answer q1 again as the helper: a second row, both counted.

**Projecting it:** open your app page on the projector. Your table updates live when a student answers (their save reaches your page and theirs only). If it lags, press **Refresh** between questions.

**Limits:** no timer; a late answer counts; a second answer counts twice (you read the newest). The plan's known-issues list says exactly this.

### (d) A live quiz (Release C): a current question you advance, answers locked in per question that students can change until the countdown ends, your Stop button, a leaderboard on your screen

**Words (Nov 25).** This is the acceptance sentence; it was run twice against the real model on Nov 14 and both times produced the shapes below (`evidence/APP-207/smoke-composer-2026-11-14.json` on the Release C branch):

> a quiz where each student answers once per question and can change it until the question closes after 30 seconds, with a leaderboard on my screen

Change `30 seconds` to the window you want (seconds or minutes).

**The result should contain:**

- a questions list you alone write, with three reviewer-only forms: **Post a question** (opens it and stamps the time), **Close a question** (your Stop button) and **Reopen a question** (restarts the window);
- a private answers list with an **Answer the open question** form: a pick-list of questions (the newest preselected) and a choice (a, b, c, d); one answer per person per question, a later answer replacing the earlier one;
- a private answer-key list with a reviewer-only form (the real run titled it **Set the correct answer**; the composer's example says **Set the expected choice**), which only you see;
- a countdown card (**Time left**), the question list (**Questions**), a **Leaderboard** (a ranking by name, scored against your answer key, up to 50 people) and a short **How it works** note. A response-count card may or may not appear; the real run had none.

In the real-model run the app was titled `Quiz`, the forms `Answer the open question`, `Post a question`, `Close a question`, `Reopen a question` and `Set the correct answer`. Yours may differ in words; the shapes must not.

**How a round goes:**

1. **Post** the question. The countdown starts from the server's clock on every open page, not from anyone's laptop.
2. **Set the correct answer** for it (the answer-key form; the ranking is 0 for everyone until you do).
3. Students pick the question (preselected) and a choice, and press the form's button. Submitting again replaces the answer; the same answer again says `Already locked in: your entry already holds these values.`
4. The window ends, or you press **Close**. A late answer is refused with `Not saved: the item this refers to is no longer open.`; nothing late is ever saved quietly.
5. The Leaderboard on your page updates: one row per student, their name, the number of matches.
6. **Close the open question before you post the next one.** The countdown and the pick-list follow the item you touched last, so closing an old question after posting a new one would make the old one current.

**Helper check (the plan's Release C verification):** post; the helper's countdown counts down; the helper answers `a`; answers `b` (one entry, replaced); answers `b` again (`Already locked in`); your Leaderboard shows the helper with 1 against a correct answer `b`; close; the helper's countdown reads `Closed`; the helper's next answer is refused; the helper's own Leaderboard card says `Scores are shown to this app's reviewers.` (Release C, APP-211). A countdown with no question yet reads `Nothing is open yet`.

**What the students see of the leaderboard:** a card that says `Scores are shown to this app's reviewers.` (Release C, APP-211). Students' devices never receive other students' answers or names (the privacy rule). Say so once in class; a public leaderboard is a later change.

**Fallback if Release C slips:** app (c), the formative quiz, read aloud question by question.

### (e) Share each app with everyone in the organization, at the right role

On the app's card on your canvas (or on its app page) press **Share app**. The dialog is titled `Share this app`. In the section `People in your organization` (`Choose who in your organization can open this app and what they can do. They see live updates.`), find **Everyone in the organization**. It first reads `Not shared with everyone. Sharing with everyone includes people who join later.`

1. In **Access for everyone** choose the role.
2. Press **Share with everyone**. The status line reads `Shared with everyone in the organization: <role>.` and the block now reads `Shared with everyone: <role>. This includes people who join later.`

The roles, in the dialog's words:

| Role in the dialog | Who gets it | Why |
| --- | --- | --- |
| `Can add and edit entries` | **everyone, for all four apps** | students must submit; in private lists they still see only their own entries |
| `Can view` | nobody in the class | a viewer cannot submit |
| `Can review all entries` | **never for everyone** | it would show every student every private entry. The dialog warns: `Everyone in the organization will see all entries, including private ones, and can take review actions. This includes people who join later.` with the button `Let everyone review all entries`. Don't press it. |

Because your apps have private lists, the dialog also says: `This app has private lists. There, people see and change only their own entries, unless they can review all entries.` If you want one person to help you grade, share with that **person** (the `Person` and `Access` fields above the everyone block) as `Can review all entries`; the dialog confirms with `<name> will see all entries, including private ones, and can take review actions.` and `Yes, share`.

To undo: **Stop sharing with everyone** (`Stopped sharing with everyone. People you added by name keep their access.`), **Change access for everyone**, or **Remove access** on a named person (`<name> no longer has access.`). If a block says `This app uses data sources or folder files, so it can be shared one person at a time only.`, the app was built on a file; rebuild it from words alone.

**Helper check:** the helper (who you did not name) can open each app, submit, and sees no Post or Grade form. Students who join on Nov 28 get the same access without any action from you.

## 2. Rehearsal (Nov 26) with 2–3 students

The plan's verification list, as a script. Run it once on Chrome, once on Safari, and once on a phone (step 10). Each step is pass or fail; write the failures down with the exact message. A red item is fixed and re-run, or accepted in writing, before codes go out (go/no-go item 2).

1. **Code, notice, checkbox.** Student opens `beta.getforma.dev/invite`: `Enter your invitation code`. The page shows the full notice under `Before you join this Forma workspace` and a required checkbox `I have read the notice and agree to take part on these terms.` They enter the code, tick, **Continue**.
   *Look for:* a mistyped code says `This code could not be used. It may be mistyped, expired, revoked or already used. Check it and try again, or ask the person who invited you for a new code.`; many tries from one network say `Too many attempts from your network in the last minute. Wait a minute, then try again.` (the plan raised the per-network limits for the classroom).
2. **Sign up and the verification mail.** They create the account with the invited address. Signing in before confirming shows `Confirm your email address`: `Your sign-in worked, but your email address is not confirmed yet. Open the confirmation link your sign-in provider sent, then sign in again.`, with `Request a new confirmation message`.
   *Look for:* the mail arrives within five minutes at a Gmail and at an Outlook address, and not in spam (go/no-go item 2). Note the minutes.
3. **Set a name.** On first sign-in the canvas asks `What should others call you?` (a `Your name` box, up to 80 characters). After saving, open the share dialog of one app as yourself: the student appears under that name.
   *Look for:* a refused name says `That name can't be used. Use 1 to 80 characters with at least one letter, number or symbol, and no invisible or control characters.`
4. **Open the shared apps and submit.** The student finds your apps in the **Library** pane (the **Library** button at the top, or the L key), under **My apps** (check in the rehearsal that shared apps land there), and presses **Open**; or follows the **Open app page** link. They check in, submit homework, answer the quiz.
   *Look for:* `Saved · Commit <n>. Refreshing the app…` on each; the Records cards show only their own rows; no Post or Grade form on their page; on your page, their name under **Filed by** and your counts rising. Watch your own page while they submit: it should update within a few seconds without Refresh.
5. **You grade; they see their grade and nobody else's.** Grade one student's submission (`Grade: choose a record`, **Open record in this form**, then **Grade**; with Release C the chooser shows the student's name first: Release C, APP-211). That student presses **Refresh** and sees the grade on their row; the other student sees nothing new.
6. **Release C poll, end to end (only if Release C is live).** Post a question, set the correct answer; both students answer, one changes their answer, you close; the ranking on the projector shows both names and their scores. Then: a late answer says `Not saved: the item this refers to is no longer open.`
   *Look for:* the countdown shows the same seconds on both laptops (it ticks from the server's clock); the student's Leaderboard card says `Scores are shown to this app's reviewers.` (Release C, APP-211).
7. **The student builds and shares their own app.** They type a sentence in their own bar (any list: "a reading list with title, author and notes"), get a card, press **Share app**, and share it with the other student as `Can add and edit entries`. The other student opens it and adds a row.
   *Look for:* `Composing your app…` then the card; the share status `Shared with <name>: Can add and edit entries.`
8. **`/admin` shows everyone with consent v1.** Open `beta.getforma.dev/admin`. The heading is your organization's name; the line under it reads `<n> members · <n> active`. Each student's row: their name, `Member`, `Active`, `Notice v1, <date>`, a last-active time, and counts of apps, works, records and uploads. Your row: `Admin · you` with `Notice v1, <date>` (a row seeded by the operator reads `Not recorded`).
9. **Remove a test member: their session dies.** On their row tick `Yes, remove <name>` and press **Remove**. You see `The member was removed. Their sessions end at their next request.` On their laptop the next click lands on `No access` (`This account has no current access here. Sign out to continue with another account.`).
   *Look for:* pressing Remove without the tick says `Nothing was removed. Tick the confirmation box to remove a member.`; removing yourself is refused with `You can't remove yourself. Another admin of this organization can.` Remove every rehearsal account before the codes go out (go/no-go item 4), then delete their uploads with the runbook's section 5 step 3.
10. **Repeat on Safari and on a phone.** Steps 1–5 at least. On the phone, the check-in form is the one that matters.
11. **The daily cap, once.** Make sure you have seen the cap message on beta before Nov 28 (go/no-go item 3): `Today's AI limit for this workspace is reached. Building apps and Ask pause until midnight UTC; your apps and entries keep working.` It is shown wherever an AI request was refused; forms and entries keep working through it.

What a rehearsal cannot show: the 25-page herd. That is proved by the load evidence in the plan, not by two students.

## 3. For the students: what you may see, and what to do

*One page. Give it out with the code email, or pin it in the room. Keep the Release C lines only if Release C is live on Nov 28.*

**Forma is a beta.** It can change, break or go offline without notice. If it does, the class carries on on paper, and your work in it is kept unless we tell you otherwise.

**Saving a form.**

- `Saving…` then `Saved · Commit <n>. Refreshing the app…`: done. The page re-reads itself.
- `The record was busy. Submit again in a moment.` or `The result of your command could not be confirmed. Submitting again with the same form is safe: it can only return the stored outcome.`: press the button again on the **same form**. The same form can't save twice. Only a fresh form makes a new entry.
- `The command was refused: a value is not one this command accepts. Nothing was changed.`: a field is empty or the wrong kind. Fix it and submit again. *Release C:* on homework the teacher has already graded, it means the submission is locked with its grade; ask the teacher if you need to change it (APP-213).
- `The record changed before your command. Nothing was changed by this submission. Start a new record and submit again.` or `The saved work moved to a newer revision. Start a new record under it.`: press Refresh and fill in a fresh form.
- *Release C:* `Already locked in: your entry already holds these values.`: your answer was already saved as it is. Nothing to do.
- *Release C:* `Not saved: the item this refers to is no longer open.`: the question or assignment closed before your answer arrived. Late answers are never saved quietly. Tell the teacher if you think it should still count.

**Your entries.** In the class apps you see only your own check-ins, submissions and answers. The teacher sees everyone's. *Release B:* if you submit twice for the same assignment or question, both are saved and both are counted; the teacher reads the newest. *Release C:* in lists that say "once per …", a second submission replaces the first.

**Timers.** *Release B:* there is no timer; a late answer still counts. *Release C:* the countdown runs from the server's clock, so everyone's clock agrees; `Closed` means the item is closed; `Nothing is open yet` means the teacher hasn't posted one yet.

**The leaderboard** (*Release C*) is on the teacher's screen. If you open it on your own device it says `Scores are shown to this app's reviewers.` That is on purpose.

**A page that looks stale.**

- `Updates are unavailable. Your unfinished form is kept. Try Refresh.`: wait a few seconds and press **Refresh** (the button on the app page, or reload).
- A card that says `Busy. Trying again in a moment…`: do nothing; it reads again by itself. If it then says `Still busy. Try again in a few seconds.`, press Refresh.
- `Updating…` on a card: the list changed while it was counting. It re-reads by itself.
- A page may take up to ten seconds to show someone else's change. Press Refresh to see it sooner.
- `This card’s data isn’t available right now.` or `This card couldn’t load.`: press Refresh; if it stays, tell the teacher.
- `Checking current access…`, `You are offline. Reconnect to verify access and live data.`: your connection dropped; reconnect.

**Building your own app.** Type a sentence in the bar at the top of your canvas (`Describe an app, ask a question, or type /`). If you see `Today's AI limit for this workspace is reached. Building apps and Ask pause until midnight UTC; your apps and entries keep working.`, the class's daily AI allowance is used up; the class apps and your entries keep working, and building returns at midnight UTC. `You have composed a few apps recently. Please try again later. Nothing was placed.` means wait a few minutes. If an app you built yourself ever shows a message ending `was refused; the record is filed.`, your entry is saved; only the hand-off to a named approver failed. The class apps never show it.

**Your name.** The canvas asks `What should others call you?` the first time. That name is what the teacher and classmates see. You can change it from the top bar.

**Something wrong, or you want to say something:** `feedback@getforma.dev` (also the **Feedback** link in the app). If you ever see someone else's entry in a private list, tell the teacher at once.

## 4. The code email (Victor sends it on Nov 28)

*From your own mail, one per student (the plan's rule: nothing in the app sends mail). Replace the brackets. Attach or paste the one-page list above.*

> **Subject:** Your invitation to Forma for the class on Nov 29–30
>
> Hi [first name],
>
> For this weekend's class we'll use Forma, a web app I'm building. It runs at **https://beta.getforma.dev** and you'll use it for check-in, homework and quizzes, and to build an app of your own.
>
> **Please set it up at home tonight, before class.** The sign-up sends a confirmation email that can take a few minutes, and the classroom network has limits that make it slower to do all at once there.
>
> 1. Open **https://beta.getforma.dev/invite**.
> 2. Enter your code: **[CODE]**. Dashes and spaces are ignored. The code is yours alone and works once.
> 3. Read the short notice on that page and tick the box.
> 4. Create your account with **this email address** (the one the invitation names), then open the confirmation link in the email you receive and sign in again.
> 5. When it asks "What should others call you?", enter the name you'd like the class to see.
>
> **Bring a laptop** (Chrome or Safari). A phone works for check-ins, but the homework and quizzes are easier on a laptop.
>
> **This is a beta.** Forma is early software: it may change, break or go offline without notice, and there is no service guarantee. If something fails during class, we switch to paper and carry on; nothing about your grade depends on the software working.
>
> **What is collected, and when it is deleted.** The notice on the invitation page says it in full; the short version:
> - Kept: your email address for sign-in, the name you choose, what you create and enter, any files you upload, and basic activity logs. Data is stored in the United States. Forma's operators can access stored data to run and support the service.
> - I'm the admin of this workspace, so I can see who has joined, your agreement to the notice, when you were last active and how much you've created. In the class apps, I (and anyone I name as a reviewer) can see what you enter; the apps keep your entries private from other students. Anything you build is private unless you share it.
> - AI: to build apps and answer questions, your prompts, and the files and entries you ask it to work with, are sent to Anthropic, the AI provider. Please don't upload sensitive personal, health or payment information.
> - This workspace's data is deleted after the test, **by December 14, 2026**; backup copies expire about a week later. A minimal security log of account events (identifiers and times, no content) is kept. The sign-in account itself is held by the identity provider, and we ask it to delete the account after the test. Copy out anything you want to keep before then.
> - You must be 18 or older to take part. You can ask at any time (feedback@getforma.dev) to have your access removed and your uploads deleted; entries you made in shared apps stay until the end of the test.
>
> **Questions, problems, ideas:** write to **feedback@getforma.dev**, or use the Feedback link inside the app. If a code doesn't work, reply to this email and I'll send a new one.
>
> See you Saturday,
> Victor

Before sending, check the go/no-go list: the mail says "redeem at home" (item 7) and "bring a laptop", the beta statement and the fallback (item 8). Every fact in the data paragraph is from notice v1 (`app/src/auth/notice.rs`); if the notice version changes, rewrite that paragraph from the new text.

## 5. During the class: what to watch, and what to do

**No deploys, no operator commands, no AWS during class.** The last restart was Nov 28 evening. Anything that needs the back office waits for an announced window (runbook section 7). If a real emergency needs the deny-all switch, that is runbook section 6 and it is yours alone; it signs everyone out and shows a plain 404 everywhere, so it is for an exposed route or a bad image, not for a slow page.

**Three things open on your laptop, besides the projector page:**

1. **`/admin`.** Who has joined and consented (`Notice v1, <date>`), who is active, and **Remove** if someone must leave the workspace. A removal ends their session at their next click. Don't remove in anger; it also removes their access to their own entries.
2. **The feedback inbox** (`feedback@getforma.dev`). The runbook keeps three piles: *bug* (steps and the student's words, no address), *wish*, and *security or data*. One report of "I can see someone else's entry" is the security pile: stop using that app for the day and write to the agents with the exact steps.
3. **Your alarm email.** The alarms (runbook section 6) go to your address: `UnhealthyTargets`, `NoRunningTask`, `LoadBalancer5xx`, low database storage and the budget. On a class day there is no deploy, so **any alarm is real**: switch to paper, check `https://beta.getforma.dev/health/ready` in a tab, and write to the agents. The scheduled stop is Dec 12 14:00 UTC after release E (the footer, JSON logs and AI usage) of Nov 15, so the service will not stop by itself this weekend.

**The daily AI cap.** 1,000 model calls a day for the whole workspace, shared by your builds, students' builds and Ask. When it is reached everyone sees `Today's AI limit for this workspace is reached. Building apps and Ask pause until midnight UTC; your apps and entries keep working.` Forms, entries, summaries and the leaderboard don't use the model, so the class apps keep working. Plan the "build your own app" exercise for the morning, and tell students to type one careful sentence rather than ten.

**If live updates misbehave** (the plan's fallback):

- Stop relying on them. Between questions or items, press **Refresh** on the projected page yourself.
- Submitting through forms does not depend on live updates. Tell students: `Updates are unavailable. Your unfinished form is kept. Try Refresh.` means press Refresh; a form that says `The record was busy. Submit again in a moment.` or `The result of your command could not be confirmed…` means submit the **same** form again (it can't save twice).
- Change one thing at a time on a page students have open, then wait about ten seconds. Posting an item is one change; closing it is another.
- *Release C:* if the countdown looks wrong, read its status line: `Nothing is open yet` means no item is posted or the student can't see one (share again); `Closed` while you expect it open means the item you touched last is a closed one: reopen the right item, or post the next. A student's `Not saved: the item this refers to is no longer open.` means the close came first; press **Reopen** if you meant to accept late answers.
- If a card says `You don’t have access to this card.`, press Refresh first. Since the fix for stale cards, a list that moved while the card was counting shows `Updating…` and re-reads, never this message; if it stays after Refresh, the share really changed (check the share dialog).

**If the model misbehaves** (a student's sentence keeps saying `That request could not be composed into an app. Nothing was placed.`): shorter words, one list, no roles or people in the sentence. The composer refuses anything that names permissions or approvers.

**After the weekend.** Real use continues Dec 1–12 with fixes only in announced windows (your own email the day before, and a note in the app's feedback link text). The purge by Dec 14 is runbook section 10.

---

**Where the wording was read (2026-11-14).** `app/admin/src/canvas/commands.ts` (`COPY`), `queries.ts`, `compose.ts`, `daily-limit.ts`, `create-app.ts`, `app.tsx`; `app/admin/src/work/document.ts`, `work/app.tsx`; `app/admin/src/record-components/controller.ts`; `app/admin/src/sharing/dialog.ts`, `sharing/api.ts`; `app/admin/src/org-admin/app.tsx`, `invite/app.tsx`, `pending/app.tsx`, `denied/app.tsx`; `app/src/auth/notice.rs`, `app/src/member_name.rs`, `app/src/org_admin.rs`, `app/src/composer/prompt.rs` and `model.rs`, `app/src/query/page.rs`, `app/src/storage/commands.rs` (reviewer-only forms are hidden from non-reviewers and refused in storage); on the Release C branch, `app/src/commands/mod.rs`, `app/src/query/record_components.rs`, `app/admin/src/canvas/actions.ts` and `app/src/composer/prompt.rs` with its APP-207 evidence (`evidence/APP-207/smoke-composer-2026-11-14.json`: the one real-model run, two passes of the quiz sentence; the attendance and homework sentences are the prompt's test fixtures and the composer campaign's stub-model checks). The ticket ids behind the body's plain words: the 25-page load evidence is APP-195 and APP-197; the stale-card fix is APP-198; the name-first chooser and the `Scores are shown to this app's reviewers.` card are APP-211 (Release C); the privacy rule is the plan's D7 and the no-app-mail rule its D12.
