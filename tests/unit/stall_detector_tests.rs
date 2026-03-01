//! Unit tests for stall detection (T110, T056).
//!
//! Validates timer firing, reset, pause/resume, consecutive nudge
//! counting, self-recovery detection, and stall notification content.

use std::time::Duration;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use agent_intercom::orchestrator::stall_detector::{StallDetector, StallEvent};
use agent_intercom::slack::blocks;

/// Helper to create a detector with short thresholds for testing.
fn test_detector(
    session_id: &str,
    inactivity_secs: u64,
    escalation_secs: u64,
    max_retries: u32,
) -> (StallDetector, mpsc::Receiver<StallEvent>, CancellationToken) {
    let ct = CancellationToken::new();
    let (tx, rx) = mpsc::channel(32);
    let detector = StallDetector::new(
        session_id.to_owned(),
        Duration::from_secs(inactivity_secs),
        Duration::from_secs(escalation_secs),
        max_retries,
        tx,
        ct.clone(),
    );
    (detector, rx, ct)
}

#[tokio::test]
async fn timer_fires_after_threshold() {
    let (detector, mut rx, cancel_token) = test_detector("s1", 1, 60, 3);
    let handle = detector.spawn();

    // Wait for the inactivity threshold to elapse.
    let event = tokio::time::timeout(Duration::from_secs(3), rx.recv())
        .await
        .expect("should receive event before timeout")
        .expect("channel should not be closed");

    assert!(
        matches!(event, StallEvent::Stalled { ref session_id, .. } if session_id == "s1"),
        "expected Stalled event, got {event:?}"
    );

    cancel_token.cancel();
    drop(handle);
}

#[tokio::test]
async fn reset_prevents_firing() {
    let (detector, mut rx, ct) = test_detector("s2", 1, 60, 3);
    let handle = detector.spawn();

    // Reset before the timer fires.
    tokio::time::sleep(Duration::from_millis(500)).await;
    handle.reset();

    // Wait just past the original threshold — should NOT fire because of reset.
    tokio::time::sleep(Duration::from_millis(700)).await;

    // Should be empty — timer was reset.
    let result = rx.try_recv();
    assert!(result.is_err(), "timer should not have fired after reset");

    // Now wait for the FULL threshold from the reset point.
    let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("should fire after full threshold from reset")
        .expect("channel should not be closed");

    assert!(matches!(event, StallEvent::Stalled { .. }));

    ct.cancel();
    drop(handle);
}

#[tokio::test]
async fn pause_and_resume_toggle() {
    let (detector, mut rx, ct) = test_detector("s3", 1, 60, 3);
    let handle = detector.spawn();

    // Pause immediately.
    handle.pause();

    // Wait beyond the threshold — should NOT fire while paused.
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let result = rx.try_recv();
    assert!(result.is_err(), "timer should not fire while paused");

    // Resume and wait for threshold.
    handle.resume();
    let event = tokio::time::timeout(Duration::from_secs(3), rx.recv())
        .await
        .expect("should fire after resume")
        .expect("channel should not be closed");

    assert!(matches!(event, StallEvent::Stalled { .. }));

    ct.cancel();
    drop(handle);
}

#[tokio::test]
async fn consecutive_nudge_counting() {
    // Very short thresholds so escalation happens quickly.
    let (detector, mut rx, ct) = test_detector("s4", 1, 1, 2);
    let handle = detector.spawn();

    // First stall event.
    let event1 = tokio::time::timeout(Duration::from_secs(3), rx.recv())
        .await
        .expect("event 1")
        .expect("channel open");
    assert!(matches!(event1, StallEvent::Stalled { .. }));

    // Escalation event should follow after escalation_threshold.
    let event2 = tokio::time::timeout(Duration::from_secs(3), rx.recv())
        .await
        .expect("event 2")
        .expect("channel open");
    assert!(
        matches!(event2, StallEvent::AutoNudge { nudge_count, .. } if nudge_count == 1),
        "expected AutoNudge with count 1, got {event2:?}"
    );

    // Second auto-nudge.
    let event3 = tokio::time::timeout(Duration::from_secs(3), rx.recv())
        .await
        .expect("event 3")
        .expect("channel open");
    assert!(
        matches!(event3, StallEvent::AutoNudge { nudge_count, .. } if nudge_count == 2),
        "expected AutoNudge with count 2, got {event3:?}"
    );

    // After max_retries, escalation.
    let event4 = tokio::time::timeout(Duration::from_secs(3), rx.recv())
        .await
        .expect("event 4")
        .expect("channel open");
    assert!(
        matches!(event4, StallEvent::Escalated { .. }),
        "expected Escalated event, got {event4:?}"
    );

    ct.cancel();
    drop(handle);
}

#[tokio::test]
async fn self_recovery_clears_alert() {
    let (detector, mut rx, ct) = test_detector("s5", 1, 60, 3);
    let handle = detector.spawn();

    // Wait for stall event.
    let event = tokio::time::timeout(Duration::from_secs(3), rx.recv())
        .await
        .expect("stall event")
        .expect("channel open");
    assert!(matches!(event, StallEvent::Stalled { .. }));

    // Agent resumes — reset the timer.
    handle.reset();

    // Self-recovery event should be emitted.
    let recovery = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("recovery event")
        .expect("channel open");

    assert!(
        matches!(recovery, StallEvent::SelfRecovered { .. }),
        "expected SelfRecovered event, got {recovery:?}"
    );

    ct.cancel();
    drop(handle);
}

#[tokio::test]
async fn cancellation_stops_detector() {
    let (detector, mut rx, ct) = test_detector("s6", 1, 60, 3);
    let handle = detector.spawn();

    // Cancel immediately.
    ct.cancel();
    drop(handle);
    // Give the task a moment to shut down.
    tokio::time::sleep(Duration::from_millis(100)).await;

    // No events should be received.
    let result = rx.try_recv();
    assert!(result.is_err(), "no events after cancellation");
}

// ── Stall notification content (T056 — S058, S060, S061) ─────────────────────

/// S058 — The stall alert blocks produced for a stalled session must include
/// the session ID so the operator can identify which agent stalled.
#[test]
fn stall_alert_blocks_contain_session_id() {
    let session_id = "session-abc-123";
    let idle_seconds = 300_u64;
    let block_text = blocks::stall_alert_message(session_id, idle_seconds);
    assert!(
        block_text.contains(session_id),
        "stall notification must include session ID '{session_id}'; got: {block_text}"
    );
    assert!(
        block_text.contains("300") || block_text.contains("5 min") || block_text.contains("idle"),
        "stall notification must reference idle time; got: {block_text}"
    );
}

/// S060 — The stall notification message must include actionable recovery steps
/// so the operator knows how to respond.
#[test]
fn stall_alert_blocks_contain_recovery_steps() {
    let block_text = blocks::stall_alert_message("sess-xyz", 120);
    // Must contain at minimum one actionable recovery suggestion.
    let has_recovery = block_text.contains("spawn")
        || block_text.contains("resume")
        || block_text.contains("ctl")
        || block_text.contains("Recovery")
        || block_text.contains("recovery")
        || block_text.contains("step");
    assert!(
        has_recovery,
        "stall notification must include recovery steps; got: {block_text}"
    );
}

/// S061 — When no Slack channel is configured, the stall detector still fires
/// its timer correctly and emits `StallEvent::Stalled` via the mpsc channel.
/// The absence of Slack must not prevent the event from being delivered.
#[tokio::test]
async fn stall_event_emitted_without_slack_configured() {
    // Create detector with no SlackService (None, mirrors S061 where no channel exists).
    let (detector, mut rx, ct) = test_detector("s-no-slack", 1, 60, 3);
    let handle = detector.spawn();

    let event = tokio::time::timeout(Duration::from_secs(3), rx.recv())
        .await
        .expect("event must be received before timeout")
        .expect("channel must remain open");

    assert!(
        matches!(event, StallEvent::Stalled { ref session_id, .. } if session_id == "s-no-slack"),
        "expected Stalled event for 's-no-slack'; got: {event:?}"
    );

    ct.cancel();
    drop(handle);
}

// ── Phase 11: ACP stall detection (T091–T093, S063–S066) ─────────────────────

/// S063 — Stream activity (any successful ACP message parse) must reset the
/// stall timer.  This test verifies the `StallDetectorHandle::reset()` mechanism
/// directly — the same mechanism triggered by `AgentEvent::StreamActivity`.
#[tokio::test]
async fn acp_stream_activity_resets_stall_timer() {
    // Very short threshold so the test completes quickly.
    let (detector, mut rx, ct) = test_detector("acp-s1", 1, 60, 3);
    let handle = detector.spawn();

    // Simulate stream activity before the threshold expires.
    tokio::time::sleep(Duration::from_millis(500)).await;
    handle.reset(); // ← what run_reader does when StreamActivity fires

    // Wait past the original threshold — stall should NOT have fired.
    tokio::time::sleep(Duration::from_millis(700)).await;
    let result = rx.try_recv();
    assert!(
        result.is_err(),
        "stream activity must prevent stall from firing before full threshold elapses"
    );

    // After the full threshold from the reset point, stall fires normally.
    let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("stall must fire after full threshold from reset")
        .expect("channel must be open");
    assert!(
        matches!(event, StallEvent::Stalled { .. }),
        "expected Stalled event after inactivity; got {event:?}"
    );

    ct.cancel();
    drop(handle);
}

/// S064 — When a stall fires for an ACP session, the nudge mechanism must
/// deliver the nudge directly on the agent stream.
///
/// This test verifies the ACP driver `send_prompt` path: posting a nudge via
/// the driver writes a `prompt/send` message to the registered writer channel,
/// which is what the stall consumer does for ACP sessions in lieu of a Slack
/// notification.
#[tokio::test]
async fn acp_nudge_delivered_via_stream() {
    use agent_intercom::driver::acp_driver::AcpDriver;
    use agent_intercom::driver::AgentDriver;
    use tokio::sync::mpsc;

    let session_id = "acp-nudge-sess";
    let acp_driver = AcpDriver::new();
    let (writer_tx, mut writer_rx) = mpsc::channel::<serde_json::Value>(8);
    acp_driver.register_session(session_id, writer_tx).await;

    // Simulate the stall consumer calling driver.send_prompt for ACP nudge.
    acp_driver
        .send_prompt(session_id, "You seem stalled. Please continue.")
        .await
        .expect("send_prompt must succeed");

    // Verify the nudge arrived on the stream writer channel.
    let msg = writer_rx
        .try_recv()
        .expect("writer channel must have received the nudge");
    assert_eq!(
        msg["method"].as_str(),
        Some("prompt/send"),
        "ACP nudge must be delivered as a prompt/send message on the stream"
    );
    assert!(
        msg["params"]["text"]
            .as_str()
            .unwrap_or_default()
            .contains("stalled"),
        "nudge text must reference the stall; got: {:?}",
        msg["params"]["text"]
    );
}

/// S066 — After all nudge retries are exhausted, the stall detector emits an
/// `Escalated` event so the stall consumer can notify the operator.
///
/// This test verifies the detector's escalation path at the unit level.
/// The stall consumer converts `Escalated` into a Slack notification (tested
/// separately in integration tests where `SlackService` is available).
#[tokio::test]
async fn nudge_retry_exhaustion_notifies_operator() {
    // 1-second inactivity threshold, 1-second escalation intervals, 2 retries.
    let (detector, mut rx, ct) = test_detector("acp-escalate", 1, 1, 2);
    let handle = detector.spawn();

    // Collect: Stalled, AutoNudge x2, Escalated
    let event1 = tokio::time::timeout(Duration::from_secs(3), rx.recv())
        .await
        .expect("stall event")
        .expect("channel open");
    assert!(
        matches!(event1, StallEvent::Stalled { .. }),
        "expected Stalled first; got {event1:?}"
    );

    let event2 = tokio::time::timeout(Duration::from_secs(3), rx.recv())
        .await
        .expect("nudge 1")
        .expect("channel open");
    assert!(
        matches!(event2, StallEvent::AutoNudge { nudge_count: 1, .. }),
        "expected AutoNudge count=1; got {event2:?}"
    );

    let event3 = tokio::time::timeout(Duration::from_secs(3), rx.recv())
        .await
        .expect("nudge 2")
        .expect("channel open");
    assert!(
        matches!(event3, StallEvent::AutoNudge { nudge_count: 2, .. }),
        "expected AutoNudge count=2; got {event3:?}"
    );

    let event4 = tokio::time::timeout(Duration::from_secs(3), rx.recv())
        .await
        .expect("escalated")
        .expect("channel open");
    assert!(
        matches!(event4, StallEvent::Escalated { .. }),
        "expected Escalated after max retries; got {event4:?}"
    );

    ct.cancel();
    drop(handle);
}
