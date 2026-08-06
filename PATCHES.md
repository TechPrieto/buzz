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
| `feat/relay-allow-html-uploads` | Removes `text/html` from the generic file-upload deny-list on the relay + CLI. | Explicit owner decision after being warned of the XSS/phishing risk (files are still served `Content-Disposition: attachment` + `nosniff` + `CSP: default-src 'none'`, so they download rather than render). | Merged (2026-08-06). Was live on the production relay (`buzz-relay:html-fix-2026-08-04`) since 2026-08-04 via a manually-built image, but was NOT in `fork/main` git history until this merge — that gap is closed now. Open, unmerged PR to upstream: `block/buzz#4754`. |
| `fix/cli-generic-file-upload` | Lets `buzz messages send --file` upload non-image/video files through the generic path. | Needed for the HTML-upload change above to be usable from the CLI. | Merged (2026-08-06), same commit pair as the row above. Open, unmerged PR to upstream: `block/buzz#4753`. |
| `fix/cli-image-upload-422` | Sanitizes JPEG/PNG before upload to avoid relay 422 rejections. | Upload reliability fix. | Merged — landed earlier via a different commit hash than the original branch tip; confirmed present by content (`sanitize_image_bytes` in `crates/buzz-cli/src/client.rs`). |
| `fix/codex-dm-root-sessions` | Scopes Codex ACP sessions per thread root instead of per session lifetime (`--thread-scoped-sessions`). | Already the deployed behavior on the VPS's `buzz-acp` (frozen there since the 2026-08-04 buzz-ops decision) — was never in `fork/main` git history until now. | Merged (2026-08-06). |
| `fix/hermes-windows-empty-args` | Stops `hermes-acp`/`amp-acp` from crashing on Windows during model discovery. | Windows compatibility fix. | Merged (2026-08-06). |

**Test evidence for the 2026-08-06 batch** (`buzz-media`, `buzz-cli`, `buzz-acp` —
the only crates touched): 324 + 108 + 671 = 1103 tests passed, 0 failed.
`cargo test --workspace` itself is currently blocked on this VPS by a missing
system `pkg-config` needed only by an unrelated `buzz-relay` dev-dependency
(`mesh-llm` → `openssl-sys`) — not something these patches touch.

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
