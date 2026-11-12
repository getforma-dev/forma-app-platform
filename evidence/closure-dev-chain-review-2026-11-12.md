# Independent review: dev-chain closures (APP-127, APP-130, APP-133, APP-136, APP-137)

- **Reviewer:** an independent Claude subagent (Opus 5.5). It did not write the code, the tests or the closure mappings. This review follows `Tickets/_tools/REVIEW.md`.
- **Date:** 2026-11-12.
- **Revision:** `main` at `3979034`, plus branch `chore/dev-chain-close` commits `9f93ec2` (the new test in `ops/beta/test_release.py`) and `180743f` (a `justfile` comment). The review also covers the uncommitted `evidence/APP-1{27,30,33,36,37}/closure-2026-11-12.md` and `evidence/APP-130/build-3979034.json`.
- **Not done:** no commit, no push and no AWS call. No `just images`, cargo or real clean was run.

## What I read

- The five tickets and their closure mappings.
- `ops/beta/{lib.sh,deploy.sh,run-step.sh,release.py,image_record.py,build-images.sh,test_release.py,test_private_state.py}`, `scripts/dev/clean.sh`, the `LegacyFenceTests` in `ops/promotion/test_maintenance.py` and `install()` in `ops/promotion/bootstrap_state.py`.
- `docs/DEVELOPMENT.md` §Delivery, `docs/pitch/RELEASES.md`, `docs/pitch/CLEAN-TRIAL-DEPLOY.md`, and `evidence/APP-140/{hosted-activation-2026-11-01,shared-auth-fix-1d88470}/`.
- `evidence/closure-2026-11-12/ci-run.md` and `evidence/APP-128/closure-2026-11-12.md`.
- The coordinator's run logs: `images-run.log`, `app130-cargo.log` and `clean-dry.txt`.

## What I ran (results)

- `python3 -m unittest discover -s ops/beta -p 'test_*.py'`: 35 tests OK, 48.4 s.
- **Mutation 1:** I deleted `exit 3` in `mutate()` (`lib.sh:112`). The new test failed 11 times: all ten step subtests and the dry-run tail. I restored the file with `git checkout`; `cmp` matched the saved copy and `git status` shows only the untracked evidence.
- **Mutation 2:** I deleted the line that prints the code (`lib.sh:109`). All `DeployFlowTests` still pass. I restored it the same way.
- **Mutation 3:** I deleted the TD-revision and `activate`-approval requirements (`release.py:126-127`). All of `test_release.py` still passes. I restored it the same way.
- **Fence probe:** I made a temporary 0700 private directory holding `promotion-managed.json`, with logging `aws` and `curl` shims first on `PATH`.
  - `deploy.sh push <id>`: "legacy operations are fenced", exit 1.
  - `verify --dry-run`: exit 0.
  - `new … --dry-run`: exit 0, "would create …/windows/<id> and release.json".
  - Neither shim was called.
- **Tags:** `git tag -l 'beta/*'`, `git for-each-ref` and `git ls-remote --tags origin`. `beta/20261101-711d87d` is annotated, peels to `711d87d` and was tagged at 20:13:41 UTC, the row's time. Its message has the `deploy.sh:397-403` format.
- **APP-136 commands:**
  - `just --summary`: 13 recipes.
  - `just check --list`: 23 steps.
  - `just campaigns --ci`: 22 of 34.
  - `just deploy help`: 18 steps. It created nothing (the main checkout still has no `.cache/ops`).
  - A link scan of the seven documents: 91 relative links, 0 missing.
  - `git diff --check origin/main`: clean.
  - `board.py check`: OK, 178 tickets.
- **APP-127 box 3:** `git grep` for absolute and scratchpad paths in `ops/beta` and `app/src` finds only the two origin comments and a hostile test string.
- **APP-130:**
  - `just clean docker --dry-run`: 10 sets, the newest three kept, 21 `would run: docker image rm` lines and the 72 h prune.
  - `gh run list --commit 3979034`: 12 workflows succeeded, including Tenant foundation; "Promote to beta" is pending.
  - `git diff --name-only 5695869 3979034`: lists the changed files.
  - `../forma-verify` is at `3979034` with a clean tracked tree.

## Findings

### Major

**M1. APP-127 box 5: "runs entirely through recipes" is not supported for window `1608eea9`.**

The repository's own record shows steps outside the recipes:
- The live state came from another clone through `GENUI_OPS_PRIV` (`docs/pitch/CLEAN-TRIAL-DEPLOY.md:37`).
- `base-input.json` was edited by hand (`:74`).
- The services were stopped explicitly (`:76`).
- Operator tasks went through `ops/beta/run-step.sh` directly (`:87`). The retained run labels `baseline13`, `migrate-repeat`, `provision-clean`, `verify-clean`, `issue-owner` and `list-owner` (`evidence/APP-140/hosted-activation-2026-11-01/sources.json`) are not labels `just deploy` produces (`deploy.sh:311-318`).
- Secrets were written by hand (`:97`).
- The activation receipt was normalized by hand, and the failed recipe `verify` was replaced by a corrected verifier from `64ce1f0` (`evidence/APP-140/hosted-activation-2026-11-01/README.md:16-18`).

What the evidence does prove: `just deploy activate` wrote the row (`RELEASES.md:29`) and the tag from a `release.json` that passed `check --activated` (`deploy.sh:385-403`).

Window `a58e69cd` (`shared-auth-fix-1d88470/README.md`: no operator task, provider or secret change) is closer to a recipe-only run, but no record says that every one of its steps was a recipe.

Fix: reword the mapping to state what ran, and either get Victor's acceptance of the narrower reading, or record box 5 as unmet and superseded by APP-167. The path is now fenced, so it can never be re-run.

**M2. APP-133 box 1: two of the four `--activated` requirements have no negative test.**

`release.py:124-127` implements all four. `test_release.py:112` only checks that a fresh record has *some* problem, and Mutation 3 shows the TD-revision and `activate`-approval checks can be removed undetected. Fix: add one subtest per requirement, on an otherwise activated record.

**M3. APP-130 box 2: the clippy, `clippy-dev` and `just check` evidence is from `5695869`, not `3979034`.**

Release code changed in between: `app/src/actions/routes.rs`, `slack/flows.rs` and `storage/agents.rs`. GitHub's Tenant foundation job runs clippy without `dev-stand` (`tenant-foundation.yml:49`) and is green on `3979034`. `clippy-dev` and the local-only `just check` steps have no run on this revision. Fix: run `just check` (or at least `clippy clippy-dev`) at `3979034` and cite it.

### Minor

- **m1. APP-127 closure: the fence is overstated.** It says "every legacy `just deploy` operation refuses (`lib.sh:49`)". But `new` and `verify` call only `priv_ready` (`deploy.sh:111-112`, `417-430`), and `help` exits first. The fence probe confirmed it. Without `--dry-run`, `new` would create a window under a fenced directory. No AWS is reachable by that path.
- **m2. APP-127 box 1, "the AWS command printed", is not asserted.** The new test checks for `[MUTATION]` only (`test_release.py:360`), and Mutation 2 passes. Fix: assert each step's command text, for example `aws ecr` or `run-task`.
- **m3. APP-127 box 2, "before any AWS call": `library` makes a read call first.** Its `prepare` runs `aws ecs describe-task-definition` before `record_approval` (`deploy.sh:328-333` vs `lib.sh:118`). The mutation itself still follows the record. Fix: reword the box mapping, or record the approval before `prepare` runs.
- **m4. APP-133 box 5: "newest among several windows" is not exercised.** The closure discloses this. The code at `deploy.sh:420-423` is correct by inspection.
- **m5. APP-127 closure: the account of the private state is inaccurate.**
  - It says the state "moved to the promotion S3 bucket". But `bootstrap_state.py` `install()` (`:95-123`) imports the ledger and the live window into S3 and deletes no window directory.
  - The `1608eea9` state was in `forma-pitch-mvp-devflow/.cache/ops/beta`, not in the main checkout, which contradicts box 4's "since APP-137" sentence for that window.
  - Neither directory exists now; nothing here records their removal.
- **m6. APP-130 box 4: the clean tree is inferred, not recorded.** `images-run.log` omits the command line, and `build.json` does not record `--allow-dirty`. The clean tree is inferred from `build-images.sh:35-38` and from `forma-verify` being clean now. The two manifest hashes differ while `equal` is true; that is by design (`image_record.py:52-70` compares bytes and source, not install paths).
- **m7. APP-136 box 5 is a forward-looking claim.** It reads "Done in this closure's commit", so re-run `board.py check` on that commit. Box 1's recipe list omits `campaign`, `campaigns` and `doctor`; all three exist, so the conclusion holds.

## Verdicts

- **APP-127: changes requested.** Boxes 1–4 are met. Box 1 now has a test that catches a removed `exit 3` (Mutation 1); m2 and m3 are wording or test gaps. Box 5 is not met as written (M1).
- **APP-130: changes requested (evidence only).** Boxes 1, 4 and 5 are met on `3979034`. Box 3 is met: `scripts/dev` is unchanged since `5695869`, the APP-128 run. Box 2 needs a run on `3979034` (M3).
- **APP-133: changes requested (one test).** Boxes 2, 3, 4 and 6 are met by tests or runs. Box 5 is met for one window (m4). Box 7 is met by the row and the tag. Box 1 is correct in code but lacks a negative control (M2).
- **APP-136: no blocking findings.** Boxes 1–4 are met on `3979034` + `180743f`, and DEVELOPMENT.md states the APP-167 fence honestly (`docs/DEVELOPMENT.md:140`). Box 5 is pending the commit's `board.py check`.
- **APP-137: no blocking findings.** Boxes 1–5 are met by passing tests, including paths with spaces (`test_private_state.py:18-22`). This review is the attributable independent review for box 6: I read `lib.sh:14-63` and found no new issue.

## Recheck, 2026-11-12

The same independent Claude subagent reviewer rechecked the branch head `d1abb8c` (`4f216df` and `d1abb8c` on top of `180743f`) and the five updated, uncommitted closures. No commit, push or AWS call was made.

**What I ran:**
- `python3 -m unittest discover -s ops/beta -p 'test_*.py'`: 38 tests OK.
- `bash -n` on `lib.sh` and `deploy.sh`: OK.
- `git diff --check 180743f d1abb8c`: clean.
- `board.py check`: OK, 178 tickets.
- `git diff --name-only 3979034 d1abb8c` lists only `justfile` and `ops/beta/*`.

**My own mutations.** Each file was restored with `git checkout` and checked with `cmp`, and `git status` was clean afterwards.
- **A.** I moved `library`'s `describe-task-definition` back into `prepare`. The `library` ordering subtest fails ("'0 aws ecs describe-task-definition' ran before its approval").
- **B.** I deleted the code-print line in `mutate()`. All ten refusal subtests fail.
- **C.** I removed `unfenced` from `new` (`deploy.sh:112`) and from `verify` (`:432`). The `new` and `verify` fence subtests fail.
- **D.** I deleted `release.py`'s TD-revision and `activate`-approval checks. The `revision` and `approval` subtests fail.

**Status of the first findings:**
- **M2** is fixed by `test_each_activated_requirement_is_refused_on_its_own` (`test_release.py:114`).
- **m1** is fixed by `unfenced()` (`lib.sh:49`) and `test_the_promotion_marker_fences_window_steps` (`:448`).
- **m2** is fixed: the refusal test checks the command text between `[MUTATION]` and "refused" (`:405`).
- **m3** is fixed: `library` now makes its read inside the gate (`deploy.sh:333-335`), covered by `test_an_approved_step_records_the_words_before_any_aws_call` (`:419`).
- **M3** is resolved. The `devchain-check.log` summary shows all 21 steps ok, including `clippy` and `clippy-dev`, on Rust and admin sources identical to `3979034`.
- **m5, m6 and m7** are corrected in the closures.

**The APP-127 box 5 record is accurate.** I checked each claim against the sources it cites:
- `docs/pitch/CLEAN-TRIAL-DEPLOY.md:37,74,76,87,97`;
- `evidence/APP-140/hosted-activation-2026-11-01/README.md:16-18` and `sources.json`;
- the tag and `RELEASES.md:29`;
- the board rule at `Tickets/README.md:39`;
- APP-167's last box, which is ticked.

Leaving the box unticked and the ticket in Backlog for Victor's decision is correct.

### New or remaining findings (all minor)

- **n1. APP-127 boxes 1–2, clean-trial windows only.** `just deploy migrate` runs `shared_broker_activation.py --state stopped` through `shown` (`deploy.sh:304`, `lib.sh:93`) before `run-step.sh`'s gate. That script calls `aws sts get-caller-identity` and `aws ecs describe-services` (`infra/scripts/shared_broker_activation.py:17,25`, using `preflight.py:602-605`). So a refused clean-trial `migrate` still makes read calls, and an approved one reads before `record_approval` (`lib.sh:124`). The tests use no clean-trial window, and the closure's "no other step's `prepare` calls AWS" is true only of `prepare`. Window `1608eea9` was a clean-trial window. Fix: wrap the guard in `prepare`, or record the exception in box 1/2's mapping. The path is fenced now.
- **n2. APP-130 box 2: the source identity is attributed, not recorded.** "So every step read `d1abb8c`'s tree" overstates it: only `py-ops` provably read `d1abb8c` (38 tests). The earlier steps read `4f216df`, which is equivalent for Rust and admin. The log itself names no commit. The conclusion holds. Box 1's row still cites `5695869` for `--lib`, but the Limits line corrects it.
- **n3. APP-127 intro: "`verify` with no window still runs" is conditional.** That is true only when `$PRIV/windows` has no activated window (`deploy.sh:419-431`). Otherwise it picks one and is fenced.
- **n4. APP-133 box 5 still has no in-repo test for "newest among several".** I ran a reviewer scratch test using the `DeployFlowTests` harness. It is outside the repo and not retained. It makes three windows: activated 11-01, activated 11-03, and one not activated. `verify` with no window picked the 11-03 window and recorded `.verified` only there. The inverted control failed as expected. The behaviour therefore holds. Adding that case to `test_release.py` would retain the evidence.

### Recheck verdicts

- **APP-127: no blocking findings on the change.** Boxes 1–4 are met, with n1 as a recorded limit. Box 5 is correctly "not met as written", so the ticket is not closable until Victor decides.
- **APP-130: no blocking findings.** All five boxes are met on `3979034`-equivalent sources (n2 is wording).
- **APP-133: no blocking findings.** Boxes 1–4 and 6–7 are met. Box 5 is met by reading and by the reviewer's scratch run (n4).
- **APP-136: no blocking findings.** Boxes 1–4 are met. Box 5 needs `board.py check` on the closure commit, after the board is regenerated.
- **APP-137: no blocking findings.** Boxes 1–5 are met by passing tests. Box 6 is this review and its recheck.
