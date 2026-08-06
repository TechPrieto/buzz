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

| Branch | What it does | Why | Status |
|---|---|---|---|
| `fix/mobile-dm-messaging` | Scopes mobile DM thread linearity to each root and suppresses nested thread links, so DMs stop showing messages from different threads mixed into one chronological list. | Reported by the team as broken mobile DM UX. | Merging into `fork/main` now (2026-08-06). |
| `feat/relay-allow-html-uploads` | Removes `text/html` from the generic file-upload deny-list on the relay + CLI. | Explicit owner decision after being warned of the XSS/phishing risk (files are still served `Content-Disposition: attachment` + `nosniff` + `CSP: default-src 'none'`, so they download rather than render). | Deployed to production relay (2026-08-04). |
| `fix/cli-generic-file-upload` | Lets `buzz messages send --file` upload non-image/video files through the generic path. | Needed for the HTML-upload change above to be usable from the CLI. | Merged. |
| `fix/cli-image-upload-422` | Sanitizes JPEG/PNG before upload to avoid relay 422 rejections. | Upload reliability fix. | Merged. |
| `fix/codex-dm-root-sessions` | Scopes Codex ACP sessions per thread root instead of per session lifetime. | Snapshot taken before v0.5.4 integration; still pending full merge. | Pending. |
| `fix/hermes-windows-empty-args` | Stops `hermes-acp`/`amp-acp` from crashing on Windows during model discovery. | Windows compatibility fix. | Pending. |
| `feat/named-conversation-threads` / `feat/named-conversation-threads-on-v0.5.4` | Fix for threads being created nested instead of flat. | The nested-thread bug the team hit and fixed in our version. | Snapshot taken before v0.5.4 rebase; still pending full integration. |

## Sync procedure

Never `rebase` `fork/main` against `origin/main` — it rewrites history that
client installs may already be anchored to. Instead, periodically:

```sh
git log fork/main..origin/main --oneline   # see what upstream shipped
```

Cherry-pick or merge in only what's worth taking (security fixes: yes;
unrelated features: no). Update this table when a patch lands, gets
superseded by an upstream equivalent, or gets dropped.
