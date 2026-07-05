//! Unit tests for ACP heartbeat steering delivery (T8.4).
//!
//! Verifies `deliver_pending_steering` fetches all unconsumed steering messages
//! for a session, delivers them via the driver, and marks them consumed — the
//! behaviour the ACP heartbeat handler reuses so operator steering queued while
//! the agent was busy is delivered at the next heartbeat (mirrors the MCP
//! `heartbeat` tool without the HTTP endpoint).

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use agent_intercom::acp::reader::deliver_pending_steering;
use agent_intercom::driver::AgentDriver;
use agent_intercom::models::steering::{SteeringMessage, SteeringSource};
use agent_intercom::persistence::{db, steering_repo::SteeringRepo};
use agent_intercom::Result;

async fn make_db() -> Arc<db::Database> {
    Arc::new(db::connect_memory().await.expect("in-memory db"))
}

fn sample_msg(session_id: &str, text: &str) -> SteeringMessage {
    SteeringMessage::new(
        session_id.to_owned(),
        None,
        text.to_owned(),
        SteeringSource::Slack,
    )
}

/// Minimal driver that records delivered prompts and always succeeds.
struct RecordingDriver {
    delivered: Arc<Mutex<Vec<String>>>,
}

impl RecordingDriver {
    fn new() -> Self {
        Self {
            delivered: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl AgentDriver for RecordingDriver {
    fn resolve_clearance(
        &self,
        _request_id: &str,
        _approved: bool,
        _reason: Option<String>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    fn send_prompt(
        &self,
        _session_id: &str,
        prompt: &str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let delivered = Arc::clone(&self.delivered);
        let prompt = prompt.to_owned();
        Box::pin(async move {
            delivered.lock().expect("lock").push(prompt);
            Ok(())
        })
    }

    fn interrupt(
        &self,
        _session_id: &str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    fn resolve_prompt(
        &self,
        _prompt_id: &str,
        _decision: &str,
        _instruction: Option<String>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    fn resolve_wait(
        &self,
        _session_id: &str,
        _instruction: Option<String>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }
}

#[tokio::test]
async fn deliver_pending_steering_delivers_and_consumes_all() {
    let db = make_db().await;
    let repo = SteeringRepo::new(Arc::clone(&db));
    repo.insert(&sample_msg("sess-hb-1", "first"))
        .await
        .expect("insert first");
    repo.insert(&sample_msg("sess-hb-1", "second"))
        .await
        .expect("insert second");

    let driver = RecordingDriver::new();
    let delivered = deliver_pending_steering("sess-hb-1", &driver, &repo)
        .await
        .expect("deliver should succeed");

    assert_eq!(delivered, 2, "both queued messages must be delivered");
    assert_eq!(
        driver.delivered.lock().expect("lock").as_slice(),
        &["first".to_owned(), "second".to_owned()],
        "messages delivered in FIFO order"
    );
    let remaining = repo
        .fetch_unconsumed("sess-hb-1")
        .await
        .expect("fetch remaining");
    assert!(
        remaining.is_empty(),
        "all delivered messages must be marked consumed"
    );
}

#[tokio::test]
async fn deliver_pending_steering_empty_queue_is_noop() {
    let db = make_db().await;
    let repo = SteeringRepo::new(Arc::clone(&db));
    let driver = RecordingDriver::new();

    let delivered = deliver_pending_steering("sess-hb-empty", &driver, &repo)
        .await
        .expect("deliver should succeed on empty queue");

    assert_eq!(delivered, 0, "no messages to deliver");
    assert!(driver.delivered.lock().expect("lock").is_empty());
}

#[tokio::test]
async fn deliver_pending_steering_only_targets_the_given_session() {
    let db = make_db().await;
    let repo = SteeringRepo::new(Arc::clone(&db));
    repo.insert(&sample_msg("sess-a", "for-a"))
        .await
        .expect("insert a");
    repo.insert(&sample_msg("sess-b", "for-b"))
        .await
        .expect("insert b");

    let driver = RecordingDriver::new();
    let delivered = deliver_pending_steering("sess-a", &driver, &repo)
        .await
        .expect("deliver");

    assert_eq!(delivered, 1, "only sess-a's message is delivered");
    assert_eq!(
        driver.delivered.lock().expect("lock").as_slice(),
        &["for-a".to_owned()]
    );
    let b_remaining = repo.fetch_unconsumed("sess-b").await.expect("fetch b");
    assert_eq!(b_remaining.len(), 1, "sess-b's message stays queued");
}
