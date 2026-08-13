use anyhow::{Context, Result};
use nostr::{Alphabet, Event, EventId, Keys, Kind, SingleLetterTag, Timestamp};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use crate::queue::FlushBatch;

pub(crate) struct NativeReplyOutbox {
    path: PathBuf,
    access: tokio::sync::Mutex<()>,
}

pub(crate) struct NativeReplyPublisher {
    rest: crate::relay::RestClient,
    outbox: NativeReplyOutbox,
    delivery: tokio::sync::Mutex<()>,
}

#[derive(Debug)]
pub(crate) struct NativeReplyReport {
    pub event_id: Option<String>,
    pub generated: bool,
    pub publish_attempted: bool,
    pub accepted: bool,
    pub persisted: bool,
    pub already_published_in_turn: bool,
    pub error: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct OutboxDrainReport {
    pub accepted: usize,
    pub remaining: usize,
}

impl NativeReplyOutbox {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self {
            path,
            access: tokio::sync::Mutex::new(()),
        }
    }

    pub(crate) async fn persist(&self, event: &Event) -> Result<()> {
        let _guard = self.access.lock().await;
        ensure_parent_dir(&self.path)?;
        let _flock = lock_outbox_exclusive(&self.path)?;
        let mut options = std::fs::OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&self.path)
            .with_context(|| format!("open native reply outbox {}", self.path.display()))?;
        enforce_private_file_permissions(&self.path)?;
        serde_json::to_writer(&mut file, event).context("serialize native reply outbox event")?;
        file.write_all(b"\n")
            .context("append native reply outbox newline")?;
        file.sync_all().context("sync native reply outbox")
    }

    pub(crate) async fn drain_with<F, Fut>(&self, mut deliver: F) -> Result<OutboxDrainReport>
    where
        F: FnMut(Event) -> Fut,
        Fut: std::future::Future<Output = Result<DeliveryStatus>>,
    {
        let _guard = self.access.lock().await;
        ensure_parent_dir(&self.path)?;
        let _flock = lock_outbox_exclusive(&self.path)?;
        let pending = read_pending(&self.path)?;
        if pending.is_empty() {
            return Ok(OutboxDrainReport {
                accepted: 0,
                remaining: 0,
            });
        }

        let mut accepted = 0;
        let mut retained = Vec::new();
        for event in pending {
            match deliver(event.clone()).await {
                Ok(_) => accepted += 1,
                Err(error) => {
                    tracing::warn!(
                        target: "native_reply::outbox",
                        event_id = %event.id,
                        "native reply outbox delivery failed: {error}"
                    );
                    retained.push(event);
                }
            }
        }

        if accepted > 0 {
            rewrite_pending(&self.path, &retained)?;
        }
        Ok(OutboxDrainReport {
            accepted,
            remaining: retained.len(),
        })
    }

    #[cfg(test)]
    async fn read_pending_for_test(&self) -> Result<Vec<Event>> {
        let _guard = self.access.lock().await;
        read_pending(&self.path)
    }
}

impl NativeReplyPublisher {
    pub(crate) fn new(rest: crate::relay::RestClient, outbox_path: PathBuf) -> Self {
        Self {
            rest,
            outbox: NativeReplyOutbox::new(outbox_path),
            delivery: tokio::sync::Mutex::new(()),
        }
    }

    pub(crate) async fn drain(&self) -> Result<OutboxDrainReport> {
        let _guard = self.delivery.lock().await;
        self.drain_locked().await
    }

    async fn drain_locked(&self) -> Result<OutboxDrainReport> {
        self.outbox
            .drain_with(|event| {
                let rest = self.rest.clone();
                async move { deliver_exact_signed_event(&rest, &event).await }
            })
            .await
    }

    pub(crate) async fn publish(
        &self,
        batch: &FlushBatch,
        content: &str,
        turn_started_at: Timestamp,
    ) -> NativeReplyReport {
        let _guard = self.delivery.lock().await;
        if let Err(error) = self.drain_locked().await {
            tracing::warn!(
                target: "native_reply::publish",
                "native reply pre-publish outbox drain failed: {error}"
            );
        }

        match manual_reply_exists_in_turn(&self.rest, batch, content, turn_started_at).await {
            Ok(true) => {
                return NativeReplyReport {
                    event_id: None,
                    generated: false,
                    publish_attempted: false,
                    accepted: true,
                    persisted: false,
                    already_published_in_turn: true,
                    error: None,
                };
            }
            Ok(false) => {}
            Err(error) => tracing::warn!(
                target: "native_reply::publish",
                "manual publication duplicate check failed; continuing with native delivery: {error}"
            ),
        }

        let event = match build_native_reply_event(batch, content, &self.rest.keys) {
            Ok(event) => event,
            Err(error) => {
                return NativeReplyReport {
                    event_id: None,
                    generated: false,
                    publish_attempted: false,
                    accepted: false,
                    persisted: false,
                    already_published_in_turn: false,
                    error: Some(error.to_string()),
                }
            }
        };
        let delivery = deliver_with_retry(&self.rest, &event).await;
        self.finish_delivery(event, delivery).await
    }

    async fn finish_delivery(
        &self,
        event: Event,
        delivery: Result<DeliveryStatus>,
    ) -> NativeReplyReport {
        let event_id = event.id.to_hex();
        match delivery {
            Ok(_) => NativeReplyReport {
                event_id: Some(event_id),
                generated: true,
                publish_attempted: true,
                accepted: true,
                persisted: false,
                already_published_in_turn: false,
                error: None,
            },
            Err(delivery_error) => match self.outbox.persist(&event).await {
                Ok(()) => NativeReplyReport {
                    event_id: Some(event_id),
                    generated: true,
                    publish_attempted: true,
                    accepted: false,
                    persisted: true,
                    already_published_in_turn: false,
                    error: Some(delivery_error.to_string()),
                },
                Err(persist_error) => NativeReplyReport {
                    event_id: Some(event_id),
                    generated: true,
                    publish_attempted: true,
                    accepted: false,
                    persisted: false,
                    already_published_in_turn: false,
                    error: Some(format!(
                        "delivery failed: {delivery_error}; outbox persistence failed: {persist_error}"
                    )),
                },
            },
        }
    }
}

/// Path of the stable sidecar lock file for a given outbox.
///
/// The lock is taken on this separate, never-replaced file rather than on
/// `path` itself: `rewrite_pending` retargets the `path` directory entry to a
/// new inode via `rename`, which would silently detach an flock held on the
/// old inode from any lock a fresh opener takes on the (now different)
/// current inode. A dedicated file that is only ever opened and locked, never
/// renamed, keeps the same inode for the outbox's entire lifetime, so the
/// lock stays meaningful across every acquirer.
fn lock_path_for(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".lock");
    PathBuf::from(name)
}

/// Block until an exclusive advisory lock on `path`'s outbox is held.
///
/// The default outbox path is scoped only by agent pubkey, not by channel or
/// process — several `buzz-acp` processes for the same identity (one per
/// channel) can share a single outbox file. The `tokio::sync::Mutex` on
/// [`NativeReplyOutbox`] only serializes access *within* one process; this
/// `flock` is what serializes the read-modify-rename cycle in `persist` and
/// `drain_with` *across* those processes so a concurrent writer can never
/// observe or clobber a half-written outbox.
#[cfg(unix)]
fn lock_outbox_exclusive(path: &Path) -> Result<nix::fcntl::Flock<std::fs::File>> {
    let lock_path = lock_path_for(path);
    let mut options = std::fs::OpenOptions::new();
    options.create(true).write(true).truncate(false);
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options
        .open(&lock_path)
        .with_context(|| format!("open native reply outbox lock {}", lock_path.display()))?;
    enforce_private_file_permissions(&lock_path)?;
    nix::fcntl::Flock::lock(file, nix::fcntl::FlockArg::LockExclusive).map_err(|(_, errno)| {
        anyhow::anyhow!(
            "flock native reply outbox lock {}: {errno}",
            lock_path.display()
        )
    })
}

#[cfg(not(unix))]
fn lock_outbox_exclusive(_path: &Path) -> Result<()> {
    Ok(())
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        let existed = parent.exists();
        std::fs::create_dir_all(parent).with_context(|| {
            format!("create native reply outbox directory {}", parent.display())
        })?;
        let is_canonical_buzz_dir = parent.file_name().is_some_and(|name| name == "buzz");
        if !existed || is_canonical_buzz_dir {
            enforce_private_directory_permissions(parent)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn enforce_private_file_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .with_context(|| format!("set private permissions on {}", path.display()))
}

#[cfg(not(unix))]
fn enforce_private_file_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn enforce_private_directory_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .with_context(|| format!("set private permissions on {}", path.display()))
}

#[cfg(not(unix))]
fn enforce_private_directory_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

fn read_pending(path: &Path) -> Result<Vec<Event>> {
    let contents = match std::fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("read native reply outbox {}", path.display()))
        }
    };
    contents
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            serde_json::from_str::<Event>(line).with_context(|| {
                format!(
                    "parse native reply outbox {} line {}",
                    path.display(),
                    index + 1
                )
            })
        })
        .collect()
}

fn rewrite_pending(path: &Path, pending: &[Event]) -> Result<()> {
    ensure_parent_dir(path)?;
    let mut temp_name = path.as_os_str().to_os_string();
    temp_name.push(".rewrite");
    let temp_path = PathBuf::from(temp_name);
    let mut options = std::fs::OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut temp = options
        .open(&temp_path)
        .with_context(|| format!("open native reply outbox rewrite {}", temp_path.display()))?;
    enforce_private_file_permissions(&temp_path)?;
    for event in pending {
        serde_json::to_writer(&mut temp, event).context("serialize retained native reply")?;
        temp.write_all(b"\n")
            .context("write retained native reply newline")?;
    }
    temp.sync_all()
        .context("sync native reply outbox rewrite")?;
    std::fs::rename(&temp_path, path).with_context(|| {
        format!(
            "replace native reply outbox {} from {}",
            path.display(),
            temp_path.display()
        )
    })?;
    enforce_private_file_permissions(path)?;
    #[cfg(unix)]
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::File::open(parent)
            .and_then(|dir| dir.sync_all())
            .with_context(|| format!("sync native reply outbox directory {}", parent.display()))?;
    }
    Ok(())
}

pub(crate) fn build_native_reply_event(
    batch: &FlushBatch,
    content: &str,
    keys: &Keys,
) -> Result<Event> {
    let triggering_event = batch
        .events
        .last()
        .context("native reply batch has no triggering event")?;
    let root_hex = crate::queue::effective_root_id(&triggering_event.event);
    let root_event_id = EventId::from_hex(&root_hex).context("invalid effective thread root")?;
    let thread_ref = buzz_sdk::ThreadRef {
        root_event_id,
        parent_event_id: root_event_id,
    };
    buzz_sdk::build_message(
        batch.channel_id,
        content,
        Some(&thread_ref),
        &[],
        false,
        &[],
    )
    .context("build native reply")?
    .sign_with_keys(keys)
    .context("sign native reply")
}

fn is_accepted_ack(response: &serde_json::Value) -> bool {
    response.get("accepted").and_then(|value| value.as_bool()) == Some(true)
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum DeliveryStatus {
    AlreadyPresent,
    Accepted,
}

pub(crate) async fn deliver_exact_signed_event(
    rest: &crate::relay::RestClient,
    event: &Event,
) -> Result<DeliveryStatus> {
    deliver_exact_signed_event_with(
        event,
        |filter| async move {
            rest.query(std::slice::from_ref(&filter))
                .await
                .map_err(Into::into)
        },
        |event| async move { rest.submit_event(&event).await.map_err(Into::into) },
    )
    .await
}

const DELIVERY_ATTEMPTS: usize = 3;

async fn deliver_with_retry(
    rest: &crate::relay::RestClient,
    event: &Event,
) -> Result<DeliveryStatus> {
    deliver_with_retry_using(
        || deliver_exact_signed_event(rest, event),
        std::time::Duration::from_millis(250),
    )
    .await
}

async fn deliver_with_retry_using<F, Fut>(
    mut deliver: F,
    base_delay: std::time::Duration,
) -> Result<DeliveryStatus>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<DeliveryStatus>>,
{
    let mut last_error = None;
    for attempt in 0..DELIVERY_ATTEMPTS {
        match deliver().await {
            Ok(status) => return Ok(status),
            Err(error) => last_error = Some(error),
        }
        if attempt + 1 < DELIVERY_ATTEMPTS && !base_delay.is_zero() {
            tokio::time::sleep(base_delay * (1 << attempt)).await;
        }
    }
    Err(last_error.expect("delivery attempts is nonzero"))
}

async fn deliver_exact_signed_event_with<Q, QFut, S, SFut>(
    event: &Event,
    query: Q,
    submit: S,
) -> Result<DeliveryStatus>
where
    Q: FnOnce(nostr::Filter) -> QFut,
    QFut: std::future::Future<Output = Result<serde_json::Value>>,
    S: FnOnce(Event) -> SFut,
    SFut: std::future::Future<Output = Result<serde_json::Value>>,
{
    let filter = nostr::Filter::new().kind(event.kind).id(event.id);
    let query_response = query(filter)
        .await
        .context("query exact native reply event id")?;
    let event_id = event.id.to_hex();
    let already_present = query_response.as_array().is_some_and(|events| {
        events.iter().any(|candidate| {
            candidate.get("id").and_then(|value| value.as_str()) == Some(event_id.as_str())
        })
    });
    if already_present {
        return Ok(DeliveryStatus::AlreadyPresent);
    }

    let response = submit(event.clone())
        .await
        .context("submit native reply event")?;
    let ack_event_id = response.get("event_id").and_then(|value| value.as_str());
    if is_accepted_ack(&response) && ack_event_id == Some(event_id.as_str()) {
        Ok(DeliveryStatus::Accepted)
    } else {
        anyhow::bail!("relay did not accept native reply: {response}")
    }
}

async fn manual_reply_exists_in_turn(
    rest: &crate::relay::RestClient,
    batch: &FlushBatch,
    final_response: &str,
    turn_started_at: Timestamp,
) -> Result<bool> {
    let triggering_event = batch
        .events
        .last()
        .context("native reply batch has no triggering event")?;
    let expected_root = crate::queue::effective_root_id(&triggering_event.event);
    let h_tag = SingleLetterTag::lowercase(Alphabet::H);
    let filter = nostr::Filter::new()
        .kind(Kind::Custom(9))
        .author(rest.keys.public_key())
        .custom_tags(h_tag, [batch.channel_id.to_string()])
        .since(turn_started_at);
    let response = rest
        .query(&[filter])
        .await
        .context("query manual publications from current turn")?;
    let events = response
        .as_array()
        .context("manual publication query did not return an array")?;
    for value in events {
        let event: Event = serde_json::from_value(value.clone())
            .context("parse current-turn manual publication")?;
        if manual_reply_matches_final(&event, &expected_root, final_response) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn manual_reply_matches_final(event: &Event, expected_root: &str, final_response: &str) -> bool {
    crate::queue::effective_root_id(event) == expected_root
        && event.content.trim() == final_response.trim()
}

pub(crate) fn should_publish_native_reply(
    source: &crate::pool::PromptSource,
    outcome: &crate::pool::PromptOutcome,
    text: &str,
) -> bool {
    matches!(source, crate::pool::PromptSource::Channel(_))
        && matches!(
            outcome,
            crate::pool::PromptOutcome::Ok(crate::acp::StopReason::EndTurn)
        )
        && !text.trim().is_empty()
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Instant;

    use nostr::{EventBuilder, Kind, Tag};
    use uuid::Uuid;

    use super::*;
    use crate::queue::{BatchEvent, FlushBatch};

    fn signed_message(content: &str, tags: Vec<Tag>) -> Event {
        EventBuilder::new(Kind::Custom(9), content)
            .tags(tags)
            .sign_with_keys(&Keys::generate())
            .expect("test event should sign")
    }

    #[test]
    fn native_reply_is_flat_under_effective_root_of_newest_trigger() {
        let channel_id = Uuid::new_v4();
        let root_id = signed_message("root", vec![]).id;
        let parent_id = signed_message("parent", vec![]).id;
        let older = signed_message("older", vec![]);
        let newest = signed_message(
            "newest",
            vec![
                Tag::parse(["e", &root_id.to_hex(), "", "root"]).expect("root tag"),
                Tag::parse(["e", &parent_id.to_hex(), "", "reply"]).expect("reply tag"),
            ],
        );
        let batch = FlushBatch {
            channel_id,
            events: vec![
                BatchEvent {
                    event: older,
                    prompt_tag: "test".into(),
                    received_at: Instant::now(),
                },
                BatchEvent {
                    event: newest,
                    prompt_tag: "test".into(),
                    received_at: Instant::now(),
                },
            ],
            cancelled_events: vec![],
            cancel_reason: None,
        };

        let event = build_native_reply_event(&batch, "final answer", &Keys::generate())
            .expect("native reply should build");
        let e_tags: Vec<Vec<String>> = event
            .tags
            .iter()
            .filter(|tag| tag.as_slice().first().is_some_and(|value| value == "e"))
            .map(|tag| tag.as_slice().to_vec())
            .collect();

        assert_eq!(event.kind, Kind::Custom(9));
        assert_eq!(event.content, "final answer");
        assert_eq!(
            e_tags,
            vec![vec![
                "e".to_string(),
                root_id.to_hex(),
                "".to_string(),
                "reply".to_string(),
            ]]
        );
    }

    #[test]
    fn native_reply_ack_requires_explicit_accepted_true() {
        assert!(is_accepted_ack(&serde_json::json!({ "accepted": true })));
        assert!(!is_accepted_ack(&serde_json::json!({ "accepted": false })));
        assert!(!is_accepted_ack(&serde_json::json!({ "message": "ok" })));
        assert!(!is_accepted_ack(&serde_json::Value::Null));
    }

    #[tokio::test]
    async fn exact_event_id_query_prevents_duplicate_submit() {
        let event = signed_message("already there", vec![]);
        let submits = Arc::new(AtomicUsize::new(0));
        let submit_counter = Arc::clone(&submits);
        let expected_id = event.id;
        let queried_event = event.clone();

        let outcome = deliver_exact_signed_event_with(
            &event,
            move |filter| async move {
                let serialized = serde_json::to_value(filter).expect("serialize exact filter");
                assert_eq!(serialized["ids"][0], expected_id.to_hex());
                assert_eq!(serialized["kinds"][0], 9);
                Ok(serde_json::json!([queried_event]))
            },
            move |_| async move {
                submit_counter.fetch_add(1, Ordering::SeqCst);
                Ok(serde_json::json!({ "accepted": true }))
            },
        )
        .await
        .expect("duplicate query should count as delivered");

        assert_eq!(outcome, DeliveryStatus::AlreadyPresent);
        assert_eq!(submits.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn failed_exact_id_query_never_attempts_submit() {
        let event = signed_message("uncertain presence", vec![]);
        let submits = Arc::new(AtomicUsize::new(0));
        let submit_counter = Arc::clone(&submits);
        let event_id = event.id.to_hex();
        let outcome = deliver_exact_signed_event_with(
            &event,
            |_| async { anyhow::bail!("query unavailable") },
            move |_| async move {
                submit_counter.fetch_add(1, Ordering::SeqCst);
                Ok(serde_json::json!({
                    "accepted": true,
                    "event_id": event_id
                }))
            },
        )
        .await;

        assert!(outcome.is_err());
        assert_eq!(submits.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn submit_ack_must_name_the_exact_signed_event() {
        let event = signed_message("ack me", vec![]);
        let accepted = deliver_exact_signed_event_with(
            &event,
            |_| async { Ok(serde_json::json!([])) },
            |event| async move {
                Ok(serde_json::json!({
                    "accepted": true,
                    "event_id": event.id.to_hex()
                }))
            },
        )
        .await
        .expect("matching ACK should be accepted");
        assert_eq!(accepted, DeliveryStatus::Accepted);

        let mismatch = deliver_exact_signed_event_with(
            &event,
            |_| async { Ok(serde_json::json!([])) },
            |_| async {
                Ok(serde_json::json!({
                    "accepted": true,
                    "event_id": "f".repeat(64)
                }))
            },
        )
        .await;
        assert!(mismatch.is_err());
    }

    #[tokio::test]
    async fn delivery_retries_three_times_and_stops_after_success() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&attempts);
        let result = deliver_with_retry_using(
            move || {
                let attempt = counter.fetch_add(1, Ordering::SeqCst);
                async move {
                    if attempt < 2 {
                        anyhow::bail!("transient failure")
                    }
                    Ok(DeliveryStatus::Accepted)
                }
            },
            std::time::Duration::ZERO,
        )
        .await;

        assert_eq!(
            result.expect("third attempt should pass"),
            DeliveryStatus::Accepted
        );
        assert_eq!(attempts.load(Ordering::SeqCst), DELIVERY_ATTEMPTS);
    }

    #[test]
    fn transition_guard_matches_only_same_root_and_final_content() {
        let root = signed_message("root", vec![]);
        let parent = signed_message("parent", vec![]);
        let nested = signed_message(
            "manual nested reply",
            vec![
                Tag::parse(["e", &root.id.to_hex(), "", "root"]).expect("root tag"),
                Tag::parse(["e", &parent.id.to_hex(), "", "reply"]).expect("reply tag"),
            ],
        );
        let unrelated = signed_message("top-level", vec![]);

        assert!(manual_reply_matches_final(
            &nested,
            &root.id.to_hex(),
            "manual nested reply"
        ));
        assert!(!manual_reply_matches_final(
            &nested,
            &root.id.to_hex(),
            "different final reply"
        ));
        assert!(!manual_reply_matches_final(
            &unrelated,
            &root.id.to_hex(),
            "top-level"
        ));
    }

    #[tokio::test]
    async fn outbox_persists_exact_event_and_drain_removes_only_accepted() {
        let path = std::env::temp_dir().join(format!(
            "buzz-acp-native-reply-outbox-{}.ndjson",
            Uuid::new_v4()
        ));
        let outbox = NativeReplyOutbox::new(path.clone());
        let accepted = signed_message("accepted", vec![]);
        let rejected = signed_message("rejected", vec![]);
        outbox
            .persist(&accepted)
            .await
            .expect("persist accepted event");
        outbox
            .persist(&rejected)
            .await
            .expect("persist rejected event");
        let accepted_id = accepted.id;
        let rejected_id = rejected.id;

        let report = outbox
            .drain_with(move |event| async move {
                if event.id == accepted_id {
                    Ok(DeliveryStatus::Accepted)
                } else {
                    anyhow::bail!("relay rejected {}", event.id)
                }
            })
            .await
            .expect("drain should rewrite retained events");

        assert_eq!(report.accepted, 1);
        assert_eq!(report.remaining, 1);
        let pending = outbox.read_pending_for_test().await.expect("read pending");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, rejected_id);
        let _ = std::fs::remove_file(path);
    }

    /// Two independently-opened file handles are how the kernel actually
    /// distinguishes concurrent processes for `flock` purposes — this proves
    /// the lock is a real cross-process mutual-exclusion primitive and not
    /// just re-testing the in-process `tokio::sync::Mutex`.
    #[cfg(unix)]
    #[test]
    fn outbox_lock_is_exclusive_across_independent_file_handles() {
        let path = std::env::temp_dir().join(format!(
            "buzz-acp-native-reply-lock-{}.ndjson",
            Uuid::new_v4()
        ));
        let lock_path = lock_path_for(&path);

        let first_holder = lock_outbox_exclusive(&path).expect("first lock should succeed");

        let second_handle = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&lock_path)
            .expect("open independent second handle");
        let second_attempt =
            nix::fcntl::Flock::lock(second_handle, nix::fcntl::FlockArg::LockExclusiveNonblock);
        let second_handle = match second_attempt {
            Ok(_) => panic!("a second independent handle must not acquire the lock while the first holder is alive"),
            Err((handle, errno)) => {
                assert_eq!(errno, nix::errno::Errno::EWOULDBLOCK);
                handle
            }
        };

        drop(first_holder);

        let third_attempt =
            nix::fcntl::Flock::lock(second_handle, nix::fcntl::FlockArg::LockExclusiveNonblock)
                .expect("lock must become available once the first holder drops it");
        drop(third_attempt);

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&lock_path);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn outbox_repairs_insecure_existing_file_and_canonical_directory_modes() {
        use std::os::unix::fs::PermissionsExt;

        let root = std::env::temp_dir().join(format!(
            "buzz-acp-native-reply-permissions-{}",
            Uuid::new_v4()
        ));
        let directory = root.join("buzz");
        std::fs::create_dir_all(&directory).expect("create test directory");
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o777))
            .expect("make directory insecure");
        let path = directory.join("outbox.ndjson");
        std::fs::write(&path, b"").expect("create test outbox");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o666))
            .expect("make file insecure");

        NativeReplyOutbox::new(path.clone())
            .persist(&signed_message("private", vec![]))
            .await
            .expect("persist should repair permissions");

        assert_eq!(
            std::fs::metadata(&path)
                .expect("file metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(&directory)
                .expect("directory metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn rejected_publication_persists_the_exact_signed_event() {
        let path = std::env::temp_dir().join(format!(
            "buzz-acp-native-reply-rejected-{}.ndjson",
            Uuid::new_v4()
        ));
        let publisher = NativeReplyPublisher::new(
            crate::relay::RestClient {
                http: reqwest::Client::new(),
                base_url: "http://unused.invalid".into(),
                keys: Keys::generate(),
                auth_tag_json: None,
            },
            path.clone(),
        );
        let event = signed_message("persist me exactly", vec![]);
        let expected = event.clone();

        let report = publisher
            .finish_delivery(event, Err(anyhow::anyhow!("accepted=false")))
            .await;

        assert!(report.generated);
        assert!(report.publish_attempted);
        assert!(!report.accepted);
        assert!(report.persisted);
        let pending = publisher
            .outbox
            .read_pending_for_test()
            .await
            .expect("read persisted event");
        assert_eq!(pending, vec![expected]);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn native_reply_gate_rejects_empty_heartbeat_cancelled_and_error_turns() {
        use crate::acp::{AcpError, StopReason};
        use crate::pool::{PromptOutcome, PromptSource};

        let channel = PromptSource::Channel(Uuid::new_v4());
        assert!(should_publish_native_reply(
            &channel,
            &PromptOutcome::Ok(StopReason::EndTurn),
            "answer"
        ));
        assert!(!should_publish_native_reply(
            &channel,
            &PromptOutcome::Ok(StopReason::EndTurn),
            " \n\t"
        ));
        assert!(!should_publish_native_reply(
            &PromptSource::Heartbeat,
            &PromptOutcome::Ok(StopReason::EndTurn),
            "heartbeat answer"
        ));
        assert!(!should_publish_native_reply(
            &channel,
            &PromptOutcome::Cancelled,
            "cancelled answer"
        ));
        assert!(!should_publish_native_reply(
            &channel,
            &PromptOutcome::Ok(StopReason::Cancelled),
            "cancelled stop reason"
        ));
        assert!(!should_publish_native_reply(
            &channel,
            &PromptOutcome::Ok(StopReason::MaxTokens),
            "partial final text"
        ));
        assert!(!should_publish_native_reply(
            &channel,
            &PromptOutcome::Error(AcpError::Protocol("failed".into())),
            "error answer"
        ));
    }
}
