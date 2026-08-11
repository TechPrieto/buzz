# Custom patches on top of upstream Buzz

This fork (`TechPrieto/buzz`) mirrors `block/buzz` (upstream) plus everything
upstream's own engineers pushed to their remotes over time — the `fork/`
remote alone has 400+ branches (`duncan/*`, `eva/*`, `tho/*`, `kennylopez-*`,
`wpfleger/*`, etc.). Those are **not ours**. This file tracks only the
branches that carry changes we actually made or asked for, so a sync against
upstream (`git log fork/main..origin/main`) never gets confused with noise
that was never ours to begin with.

When upstream ships a native equivalent of something listed here, drop our
patch on the next sync and take theirs — fewer custom diffs to carry forward.

"Merged" below means the code is verifiably in `fork/main`'s git history —
confirmed by content, not just branch-ref ancestry (a fix can land via a
different commit hash than its original branch, e.g. cherry-pick or manual
apply, and still be "merged" in the sense that matters).

| Branch | What it does | Why | Status |
|---|---|---|---|
| `fix/mobile-dm-messaging` | Scopes mobile DM thread linearity to each root and suppresses nested thread links, so DMs stop showing messages from different threads mixed into one chronological list. | Reported by the team as broken mobile DM UX. | Merged (2026-08-06). |
| `feat/named-conversation-threads` | Desktop fix: threads created inside a thread now stay flat under the same root instead of nesting a new thread on every reply. | The nested-thread bug the team hit and fixed in our version. | Merged as `08d820cb8` (2026-08-06), desktop suite 3918/3918 passing. |
| `fix/cli-generic-file-upload` | Lets `buzz messages send --file` upload non-image/video files (docs, archives, text) through the generic path — text/html stays blocked. | Owner need: agents deliver files to the owner for download, not accept HTML uploads. | Merged (2026-08-06). Open, unmerged PR to upstream: `block/buzz#4753`. |
| `fix/cli-image-upload-422` | Sanitizes JPEG/PNG before upload to avoid relay 422 rejections. | Upload reliability fix. | Merged — landed earlier via a different commit hash than the original branch tip; confirmed present by content (`sanitize_image_bytes` in `crates/buzz-cli/src/client.rs`). |
| `fix/codex-dm-root-sessions` | Scopes Codex ACP sessions per thread root instead of per session lifetime (`--thread-scoped-sessions`). | Already the deployed behavior on the VPS's `buzz-acp` (frozen there since the 2026-08-04 buzz-ops decision) — was never in `fork/main` git history until now. | Merged (2026-08-06). |
| `fix/hermes-windows-empty-args` | Stops `hermes-acp`/`amp-acp` from crashing on Windows during model discovery. | Windows compatibility fix. | Merged (2026-08-06). |

**Reverted: `feat/relay-allow-html-uploads`.** Briefly merged and deployed
2026-08-06 (`buzz-relay:html-fix-2026-08-04`), then reverted the same day —
owner clarified the actual need was agent-to-owner file delivery, not
accepting HTML uploads (`9250524b8`, tag `techprieto-v1.2.1`). `text/html` is
back in both deny-lists (relay + CLI); upstream PR `block/buzz#4754` closed.
`techprieto-v1.2.0` still has HTML allowed — do not use it for new client
installs, use `techprieto-v1.2.1` or later.

**Test evidence for the 2026-08-06 batch** (`buzz-media`, `buzz-cli`, `buzz-acp` —
the only crates touched): 324 + 108 + 671 = 1103 tests passed, 0 failed.
`cargo test --workspace` itself is currently blocked on this VPS by a missing
system `pkg-config` needed only by an unrelated `buzz-relay` dev-dependency
(`mesh-llm` → `openssl-sys`) — not something these patches touch.

## 2026-08-08: upstream sync to `desktop-v0.5.7`

Merged `desktop-v0.5.7` (147 upstream commits) into `main` via `git merge
--ff-only` from `integration/desktop-v0.5.7-techprieto` (no rebase, per the
rule above). Commit: `6bbc31452`. Rollback point: tag
`techprieto-baseline-2026-08-07` (the pre-merge tip).

Validation before push:
- `detect-secrets` scoped to the ~1077 files this merge actually touches
  (not upstream's full history, which isn't ours to audit): 205 hits, all
  manually confirmed false positives (test fixtures, a dev-default DB
  connection string already marked `sadscan:disable` in the code itself,
  public Node.js binary checksums). Zero real secrets.
- Isolated Docker Compose smoke test (own network/volumes/DB, random
  secrets, zero contact with `buzz-prod`): relay built from `6bbc31452`,
  full round trip verified — channel create, message send/read, media
  upload with sanitization. All passed. Stack fully torn down after.
- `cargo test --workspace --exclude buzz-relay` (broader than the previous
  3-crate scope; `buzz-relay`'s own test suite is still blocked by the same
  pre-existing missing `pkg-config`, its normal `--release` build was
  verified separately and succeeds). Two failure patterns found, both
  independently confirmed **not caused by this merge**:
  - `crates/buzz-agent/tests/fake_llm.rs` (turn-cancellation tests): flaky
    across repeated runs — different test names fail each run, one run passed
    clean. Reproduced the same failure on `techprieto-baseline-2026-08-07`
    (our tip *before* this merge existed) — pre-existing in our fork already.
  - `crates/git-sign-nostr/src/lib.rs::test_parse_envelope_rejects_invalid_oa_pubkey`:
    fails deterministically (3/3 runs), but reproduced identically on pure
    `desktop-v0.5.7` with none of our patches applied — an upstream bug, not
    ours to carry blame for, not introduced by this sync.

Pushed to `origin/main` (`TechPrieto/buzz`, public) after all of the above.
Not deployed to the VPS relay — that is a separate decision; see the
"Política de instalaciones a cliente" and rollback pattern in
`techprieto-workspace/operations/BUZZ-MESSAGING.md`.

## 2026-08-09: upstream sync to `desktop-v0.5.8` / `relay-v0.2.1`

Merged `origin/main` (11 upstream commits since `desktop-v0.5.7`) directly
into `fork/main` in an isolated worktree (`/tmp/sync-0.5.8`), no rebase, per
the rule above. Commit: `65b93f1d3`. Rollback point: tag
`techprieto-baseline-2026-08-09` (the pre-merge tip, `85ecb43bb`).

Conflicts were all version-bump noise (`.release/desktop-candidate.json`,
`CHANGELOG.md`, `desktop/package.json`, `desktop/src-tauri/Cargo.{toml,lock}`,
`desktop/src-tauri/tauri.conf.json`) — resolved by taking upstream's 0.5.8
version strings and merging both CHANGELOG entries (0.5.8 above 0.5.7, not
replacing it).

**Notable upstream change: `Revert "fix(acp): reject unattended permission
requests"` (`#5323`, reverts `#4609`).** Changes `buzz-acp`'s *default*
`permission_mode` from `dontAsk` (fail-closed: unattended
`session/request_permission` calls are rejected) to `bypassPermissions`
(auto-approved). Discussed with the owner in #buzz-ops (2026-08-08/09) —
decision: **take the code as-is (do not patch it) but override the default
back to `dontAsk` on the VPS via `BUZZ_ACP_PERMISSION_MODE=dontAsk` in the
deploy `.env`.** Reasoning: keeps this sync free of a custom code diff to
carry forward (upstream's default is the one in the table above them,
`bypassPermissions`, so future syncs won't conflict on this file), while
preserving fail-closed behavior in prod until we've actually observed
whether auto-approve solves a real problem for us. Toggling later is a
one-line `.env` change + service restart, not a rebuild.

Validation before push:
- `cargo test --workspace --exclude buzz-relay --no-fail-fast`: same two
  failure patterns as the 0.5.7 sync, both re-confirmed **not caused by this
  merge**:
  - `crates/buzz-agent/tests/fake_llm.rs` (turn-cancellation tests): flaky —
    different test names fail across repeated runs (5 runs on the merge tip,
    3 more on `techprieto-baseline-2026-08-09` pre-merge), one clean run in
    each set. Reproduces identically before and after this merge.
  - `crates/git-sign-nostr/src/lib.rs::test_parse_envelope_rejects_invalid_oa_pubkey`:
    fails deterministically — same pre-existing upstream bug documented in
    the 0.5.7 sync entry above, untouched by this merge.
- `cargo build --release -p buzz-relay`: compiles clean (8m25s), confirming
  the crate builds even though its own test suite is still blocked locally
  by the pre-existing missing `pkg-config`.

Pushed to `origin/main` (`TechPrieto/buzz`, public) after all of the above.
Not deployed to the VPS relay yet, and the `BUZZ_ACP_PERMISSION_MODE`
override has not been applied to the VPS `.env` yet either — both are
separate steps pending the owner's go-ahead.

## 2026-08-10: upstream sync to `desktop-v0.5.9`

Merged `origin/main` (25 upstream commits since `desktop-v0.5.8`) directly
into `fork/main` in an isolated worktree (`/tmp/sync-0.5.9`), no rebase, per
the rule above. Commit: `146f6261a`. Rollback point: tag
`techprieto-baseline-2026-08-10` (the pre-merge tip, `f5b15af9c`).

**Real conflicts this time, not just version-bump noise** — the largest
upstream change in this batch was `563e4346d` ("Reduce repeated ACP session
context", #5423), which touches the same `SessionState`/session-creation
code path as our own `fix/codex-dm-root-sessions` patch:

- `crates/buzz-acp/src/pool.rs`: both sides added a struct in the same spot
  (our `ConversationKey` for thread-scoped DM sessions, their
  `ChannelDeliveryState` for standing-context delivery tracking) — kept
  both, combined the `invalidate_channel` cleanup to clear both
  `root_sessions` and `deliveries`, and combined session-creation to both
  route into `root_sessions`/`sessions` per our thread-scoping logic *and*
  seed `deliveries` + call `notify_session_spawned` per theirs (verified
  `deliveries` is keyed by plain `channel_id`, independent of our
  thread-scoping, so both belong together unconditionally).
- `crates/buzz-acp/src/queue.rs`: three of our test functions and two of
  upstream's landed at the same anchor point in the test module, producing
  an interleaved conflict. Reconstructed both original test bodies in full
  from each side's pre-merge blob (`git show HEAD:...` / `git show
  origin/main:...`) rather than trying to hand-edit the tangled diff, then
  verified `FormatPromptArgs` already carries both sides' fields
  (`stable_dm_root_reply` ours, `conversation_context_had_delivered_events`
  theirs) with no further conflict.
- `desktop/src/features/messages/lib/persistentAgentAudience.ts`: took
  upstream's version outright — our side was leftover WIP from the
  `feat/named-conversation-threads` snapshot commits, not a maintained
  patch, and upstream's rewrite is the more current, intentional behavior.

`cargo check -p buzz-acp` confirmed the manual resolution compiles before
committing.

Also reviewed the new `feat(desktop): NIP-AM agent-usage backend` commit
(`5e4c05f90`, ~2000 lines in `usage.rs`) for security surface before
merging: no new outbound HTTP calls — it's local aggregation that publishes
NIP events through our own relay for usage/cost tracking, no new network
exposure. `config.rs` (the file the `permission_mode` revert touched last
sync) has zero changes in this batch — reviewed line by line, confirmed.

Validation before push:
- `cargo test --workspace --exclude buzz-relay --no-fail-fast`: only the
  same pre-existing `git-sign-nostr` failure documented in the 0.5.7 sync
  entry — the `buzz-agent` turn-cancellation flakiness didn't even show up
  in the main run this time, but 3 repeated `-p buzz-agent --test fake_llm`
  runs on both the merge tip and the `techprieto-baseline-2026-08-10`
  pre-merge tip reproduced it identically on both (different test names
  failing each run) — confirmed still preexisting, not from this merge.
- `cargo build --release -p buzz-relay`: compiles clean (8m24s).

Pushed to `origin/main` (`TechPrieto/buzz`, public) after all of the above.
Not deployed to the VPS relay yet — separate step pending the owner's
go-ahead.

## Sync procedure

Never `rebase` `fork/main` against `origin/main` — it rewrites history that
client installs may already be anchored to. Instead, periodically:

```sh
git log fork/main..origin/main --oneline   # see what upstream shipped
```

Cherry-pick or merge in only what's worth taking (security fixes: yes;
unrelated features: no). Update this table when a patch lands, gets
superseded by an upstream equivalent, or gets dropped.

**Verify "merged" claims by content, not just `git merge-base --is-ancestor`
against the original branch name.** A patch can land via a different commit
hash (cherry-pick, manual apply) and still be genuinely present — ancestry
checks against the wrong ref give false negatives. Conversely, a status of
"Deployed to production" does NOT imply "in `fork/main`'s git history" —
those are different facts and this table tracks the git-history one.
