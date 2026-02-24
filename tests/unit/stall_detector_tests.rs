//! Unit tests for stall detection (T110).
//!
//! Validates timer firing, reset, pause/resume, consecutive nudge
//! counting, and self-recovery detection.

use std::time::Duration;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use agent_intercom::orchestrator::stall_detector::{StallDetector, StallEvent};

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
