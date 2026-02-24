//! Integration tests for stall detector escalation chain.
//!
//! Validates the full escalation flow: Stalled → `AutoNudge` → Escalated,
//! as well as reset/self-recovery, pause/resume, and cancellation.

use std::time::Duration;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use agent_intercom::orchestrator::stall_detector::{StallDetector, StallEvent};

// ── Stalled event fires after inactivity threshold ───────────

#[tokio::test]
async fn stall_detected_after_inactivity() {
    let ct = CancellationToken::new();
    let (tx, mut rx) = mpsc::channel(32);

    let detector = StallDetector::new(
        "sess-1".into(),
        Duration::from_millis(200),
        Duration::from_millis(200),
        3,
        tx,
        ct.clone(),
    );
    let _handle = detector.spawn();

    let event = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("event within timeout")
        .expect("channel open");

    assert!(
        matches!(event, StallEvent::Stalled { ref session_id, .. } if session_id == "sess-1"),
        "expected Stalled event, got {event:?}"
    );

    ct.cancel();
}

// ── AutoNudge fires after escalation interval ────────────────

#[tokio::test]
async fn auto_nudge_after_escalation_interval() {
    let ct = CancellationToken::new();
    let (tx, mut rx) = mpsc::channel(32);

    let detector = StallDetector::new(
        "sess-2".into(),
        Duration::from_millis(100),
        Duration::from_millis(100),
        3,
        tx,
        ct.clone(),
    );
    let _handle = detector.spawn();

    // First event should be Stalled.
    let event1 = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("event1 timeout")
        .expect("open");
    assert!(matches!(event1, StallEvent::Stalled { .. }));

    // Second event should be AutoNudge with nudge_count=1.
    let event2 = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("event2 timeout")
        .expect("open");
    assert!(
        matches!(event2, StallEvent::AutoNudge { nudge_count: 1, .. }),
        "expected AutoNudge(1), got {event2:?}"
    );

    ct.cancel();
}

// ── Full escalation chain: Stalled → AutoNudge(1,2,3) → Escalated ──

#[tokio::test]
async fn full_escalation_chain() {
    let ct = CancellationToken::new();
    let (tx, mut rx) = mpsc::channel(32);

    let max_retries = 2;
    let detector = StallDetector::new(
        "sess-3".into(),
        Duration::from_millis(100),
        Duration::from_millis(100),
        max_retries,
        tx,
        ct.clone(),
    );
    let _handle = detector.spawn();

    // Collect events: Stalled, AutoNudge(1), AutoNudge(2), Escalated
    let mut events = Vec::new();
    for _ in 0..4 {
        let event = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("event timeout")
            .expect("open");
        events.push(event);
    }

    assert!(matches!(events[0], StallEvent::Stalled { .. }));
    assert!(matches!(
        events[1],
        StallEvent::AutoNudge { nudge_count: 1, .. }
    ));
    assert!(matches!(
        events[2],
        StallEvent::AutoNudge { nudge_count: 2, .. }
    ));
    assert!(matches!(
        events[3],
        StallEvent::Escalated { nudge_count: 3, .. }
    ));

    ct.cancel();
}

// ── Reset before threshold prevents stall ────────────────────

#[tokio::test]
async fn reset_before_threshold_prevents_stall() {
    let ct = CancellationToken::new();
    let (tx, mut rx) = mpsc::channel(32);

    let detector = StallDetector::new(
        "sess-4".into(),
        Duration::from_millis(500),
        Duration::from_millis(500),
        3,
        tx,
        ct.clone(),
    );
    let handle = detector.spawn();

    // Reset after 200ms (before the 500ms threshold).
    tokio::time::sleep(Duration::from_millis(200)).await;
    handle.reset();

    // Wait enough time for a stall to fire if the reset didn't work.
    let result = tokio::time::timeout(Duration::from_millis(400), rx.recv()).await;

    // Should timeout (no event received) because we reset.
    assert!(result.is_err(), "no stall event should fire after reset");

    ct.cancel();
}

// ── Self-recovery emits SelfRecovered event ──────────────────

#[tokio::test]
async fn self_recovery_after_stall() {
    let ct = CancellationToken::new();
    let (tx, mut rx) = mpsc::channel(32);

    let detector = StallDetector::new(
        "sess-5".into(),
        Duration::from_millis(100),
        Duration::from_millis(500),
        3,
        tx,
        ct.clone(),
    );
    let handle = detector.spawn();

    // Wait for stall.
    let event = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("stall timeout")
        .expect("open");
    assert!(matches!(event, StallEvent::Stalled { .. }));

    // Reset to trigger self-recovery.
    handle.reset();

    let event = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("recovery timeout")
        .expect("open");
    assert!(
        matches!(event, StallEvent::SelfRecovered { ref session_id } if session_id == "sess-5"),
        "expected SelfRecovered, got {event:?}"
    );

    ct.cancel();
}

// ── Pause prevents stall events ──────────────────────────────

#[tokio::test]
async fn pause_prevents_stall_events() {
    let ct = CancellationToken::new();
    let (tx, mut rx) = mpsc::channel(32);

    let detector = StallDetector::new(
        "sess-6".into(),
        Duration::from_millis(200),
        Duration::from_millis(200),
        3,
        tx,
        ct.clone(),
    );
    let handle = detector.spawn();

    // Pause immediately.
    handle.pause();

    // Wait longer than the threshold.
    let result = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await;
    assert!(result.is_err(), "no events should fire while paused");

    ct.cancel();
}

// ── Resume after pause restarts detection ────────────────────

#[tokio::test]
async fn resume_after_pause_restarts_detection() {
    let ct = CancellationToken::new();
    let (tx, mut rx) = mpsc::channel(32);

    let detector = StallDetector::new(
        "sess-7".into(),
        Duration::from_millis(200),
        Duration::from_millis(200),
        3,
        tx,
        ct.clone(),
    );
    let handle = detector.spawn();

    // Pause for a bit, then resume.
    handle.pause();
    tokio::time::sleep(Duration::from_millis(300)).await;
    handle.resume();

    // After resume, stall should eventually fire.
    let event = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("event after resume")
        .expect("open");
    assert!(
        matches!(event, StallEvent::Stalled { .. }),
        "should fire stall after resume"
    );

    ct.cancel();
}

// ── Cancellation stops the detector ──────────────────────────

#[tokio::test]
async fn cancellation_stops_detector() {
    let ct = CancellationToken::new();
    let (tx, mut rx) = mpsc::channel(32);

    let detector = StallDetector::new(
        "sess-8".into(),
        Duration::from_millis(500),
        Duration::from_millis(500),
        3,
        tx,
        ct.clone(),
    );
    let handle = detector.spawn();

    // Cancel before the stall threshold.
    ct.cancel();

    // Wait briefly; no events should fire.
    let result = tokio::time::timeout(Duration::from_millis(700), rx.recv()).await;

    // Channel might close (returning None) or timeout — either means no stall.
    if let Ok(Some(event)) = result {
        panic!("unexpected event after cancel: {event:?}");
    }

    drop(handle);
}

// ── is_stalled reflects detector state ───────────────────────

#[tokio::test]
async fn is_stalled_reflects_state() {
    let ct = CancellationToken::new();
    let (tx, mut rx) = mpsc::channel(32);

    let detector = StallDetector::new(
        "sess-9".into(),
        Duration::from_millis(100),
        Duration::from_millis(500),
        3,
        tx,
        ct.clone(),
    );
    let handle = detector.spawn();

    // Not stalled initially.
    assert!(!handle.is_stalled());

    // Wait for stall.
    let _event = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("stall event")
        .expect("open");

    // Now stalled.
    assert!(handle.is_stalled());

    // Reset → should clear stalled flag.
    handle.reset();
    // Brief delay for the reset to process.
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(!handle.is_stalled());

    ct.cancel();
}
