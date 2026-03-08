//! Integration test: full fallback flow for prompt refine (S029→S030→S031).
//!
//! T044 [P] [US4]: Exercises the complete thread-reply fallback path:
//! 1. `open_modal` fails → `register_thread_reply_fallback` called (S029)
//! 2. Operator replies in thread → `route_thread_reply` captures the reply (S030)
//! 3. Acknowledgment step — entry is removed from the map (S031)

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{oneshot, Mutex};

use agent_intercom::slack::handlers::thread_reply::{
    fallback_map_key, register_thread_reply_fallback, route_thread_reply,
};

/// Full fallback flow: register → reply → verify delivery and cleanup.
#[tokio::test]
async fn test_s029_s030_s031_full_fallback_flow() {
    let pending = Arc::new(Mutex::new(HashMap::<
        String,
        (String, String, oneshot::Sender<String>),
    >::new()));
    let channel_id = "C_INTEGRATION";
    let thread_ts = "1700000000.000100".to_owned();
    let authorized_user = "U_OPERATOR".to_owned();
    let session_id = "session-integration-001".to_owned();

    // Step 1: modal fails → register fallback (S029).
    let (tx, rx) = oneshot::channel::<String>();
    register_thread_reply_fallback(
        channel_id,
        thread_ts.clone(),
        session_id.clone(),
        authorized_user.clone(),
        tx,
        Arc::clone(&pending),
    )
    .await;

    // Verify registration via composite key.
    let key = fallback_map_key(channel_id, &thread_ts);
    assert!(
        pending.lock().await.contains_key(&key),
        "S029: fallback entry should be registered after modal failure"
    );

    // Step 2: operator replies in thread (S030) — run concurrently to simulate real usage.
    let reply_task = {
        let pending = Arc::clone(&pending);
        let thread_ts = thread_ts.clone();
        let authorized_user = authorized_user.clone();
        tokio::spawn(async move {
            route_thread_reply(
                channel_id,
                &thread_ts,
                &authorized_user,
                "refine: use smaller steps",
                pending,
            )
            .await
        })
    };

    let result = reply_task.await.expect("reply task should not panic");
    assert!(result.is_ok(), "S030: route_thread_reply should succeed");
    assert!(
        result.unwrap(),
        "S030: route_thread_reply should return Ok(true) when reply is captured"
    );

    // Step 3: oneshot delivers the reply text (S030).
    let reply_text = rx.await.expect("S030: oneshot should deliver the reply");
    assert_eq!(
        reply_text, "refine: use smaller steps",
        "S030: received reply text should match what the operator typed"
    );

    // S031: entry is removed — the caller would post an ack message after this.
    assert!(
        !pending.lock().await.contains_key(&key),
        "S031: entry should be removed from map after reply is captured"
    );
}
