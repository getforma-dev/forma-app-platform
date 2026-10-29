# Independent optional secret handoff review

No blocking finding in the reviewed runtime or conditional infrastructure source. This is local preparation; final combined-source CI, preflight binding and managed acceptance remain open. Source hashes and exact check output are in [independent-review.json](independent-review.json).

The supervisor removes the raw environment value before validation, refresh or child creation. Only `serve` accepts it; a simultaneous explicit file variable refuses. It preserves the exact bounded UTF-8 bytes and leaves JSON/schema/expiry/identity policy with the existing Rust validator. Default absence keeps the callback disabled.

File creation uses an existing current-UID mode0700 directory, exclusive no-follow creation, mode0600 and a fresh random name. The held non-inheritable file descriptor prevents inode reuse before cleanup. Launch rechecks directory/file identity, owner, permissions and single-link state; cleanup refuses observed replacement or extra hardlinks. Existing explicit configuration files are not owned or deleted. Normal and failed child exit, startup failures, cancellation and bounded TERM/KILL handling clean the owned file.

The conditional infrastructure change defaults to no injection and no optional read policy. When enabled it supplies only the full JSON secret reference and grants only the application execution role `GetSecretValue` on the exact approved ARN. It changes neither task-role authority nor migration secret delivery, adds no secret resource or decrypt grant, and never places JSON in a stack parameter/output. The CDK W2509 warning is a name heuristic on a nonsecret ARN parameter.

Independent checks passed: **22 runtime tests**, all **3 native supervisor modes** (absent→404, valid injection→303, malformed JSON refused before listening), **15 template tests**, and diff whitespace validation. All owned synthetic files were removed, synthetic secrets were absent from captured output, source hashes still match author receipts, and the native binary hash matches the author's record.

The private directory assumes trusted same-UID processes. The stat/unlink sequence is not an atomic defense against a hostile process with the same UID. Environment removal is not memory zeroization; SIGKILL or host crash cannot execute cleanup. Task/ephemeral-volume teardown remains necessary for abrupt termination.

Before activation, root must bind the exact optional ARN and reviewed image in preflight, observe every service task stopped, and complete the full stack update including the conditional policy. The previous callback-only image predates this runtime addition. These local checks establish neither actual ECS secret delivery nor hosted identity, membership, session admission, mail delivery or revocation. The reviewer made no cloud calls, read no credentials, and changed no implementation or staged files.
