//! Unit tests for `AgentEvent` enum construction and field access (T007).

use agent_intercom::driver::AgentEvent;
use agent_intercom::models::progress::{ProgressItem, ProgressStatus};

#[test]
fn agent_event_clearance_requested_constructs_and_accesses_fields() {
    let event = AgentEvent::ClearanceRequested {
        request_id: "req-001".into(),
        session_id: "sess-abc".into(),
        title: "Create new module".into(),
        description: "Adding src/driver/mod.rs".into(),
        diff: Some("--- /dev/null\n+++ b/src/driver/mod.rs".into()),
        file_path: "src/driver/mod.rs".into(),
        risk_level: "low".into(),
    };

    if let AgentEvent::ClearanceRequested {
        request_id,
        session_id,
        title,
        description,
        diff,
        file_path,
        risk_level,
    } = &event
    {
        assert_eq!(request_id, "req-001");
        assert_eq!(session_id, "sess-abc");
        assert_eq!(title, "Create new module");
        assert_eq!(description, "Adding src/driver/mod.rs");
        assert!(diff.is_some());
        assert_eq!(file_path, "src/driver/mod.rs");
        assert_eq!(risk_level, "low");
    } else {
        panic!("wrong variant");
    }
}

#[test]
fn agent_event_clearance_requested_with_no_diff() {
    let event = AgentEvent::ClearanceRequested {
        request_id: "req-002".into(),
        session_id: "sess-abc".into(),
        title: "Delete file".into(),
        description: "Removing obsolete module".into(),
        diff: None,
        file_path: "src/old.rs".into(),
        risk_level: "high".into(),
    };

    if let AgentEvent::ClearanceRequested { diff, .. } = &event {
        assert!(diff.is_none());
    } else {
        panic!("wrong variant");
    }
}

#[test]
fn agent_event_status_updated_constructs_and_accesses_fields() {
    let event = AgentEvent::StatusUpdated {
        session_id: "sess-001".into(),
        message: "Running cargo test...".into(),
    };

    if let AgentEvent::StatusUpdated {
        session_id,
        message,
    } = &event
    {
        assert_eq!(session_id, "sess-001");
        assert_eq!(message, "Running cargo test...");
    } else {
        panic!("wrong variant");
    }
}

#[test]
fn agent_event_prompt_forwarded_constructs_and_accesses_fields() {
    let event = AgentEvent::PromptForwarded {
        session_id: "sess-001".into(),
        prompt_id: "prompt-001".into(),
        prompt_text: "Should I refactor the error handling?".into(),
        prompt_type: "continuation".into(),
    };

    if let AgentEvent::PromptForwarded {
        session_id,
        prompt_id,
        prompt_text,
        prompt_type,
    } = &event
    {
        assert_eq!(session_id, "sess-001");
        assert_eq!(prompt_id, "prompt-001");
        assert_eq!(prompt_text, "Should I refactor the error handling?");
        assert_eq!(prompt_type, "continuation");
    } else {
        panic!("wrong variant");
    }
}

#[test]
fn agent_event_heartbeat_received_with_progress() {
    let items = vec![
        ProgressItem {
            label: "Writing tests".into(),
            status: ProgressStatus::Done,
        },
        ProgressItem {
            label: "Implementation".into(),
            status: ProgressStatus::InProgress,
        },
    ];

    let event = AgentEvent::HeartbeatReceived {
        session_id: "sess-001".into(),
        progress: Some(items),
    };

    if let AgentEvent::HeartbeatReceived {
        session_id,
        progress,
    } = &event
    {
        assert_eq!(session_id, "sess-001");
        let p = progress.as_ref().expect("progress should be Some");
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].label, "Writing tests");
    } else {
        panic!("wrong variant");
    }
}

#[test]
fn agent_event_heartbeat_received_without_progress() {
    let event = AgentEvent::HeartbeatReceived {
        session_id: "sess-001".into(),
        progress: None,
    };

    if let AgentEvent::HeartbeatReceived { progress, .. } = &event {
        assert!(progress.is_none());
    } else {
        panic!("wrong variant");
    }
}

#[test]
fn agent_event_session_terminated_with_exit_code() {
    let event = AgentEvent::SessionTerminated {
        session_id: "sess-001".into(),
        exit_code: Some(1),
        reason: "process exited with code 1".into(),
    };

    if let AgentEvent::SessionTerminated {
        session_id,
        exit_code,
        reason,
    } = &event
    {
        assert_eq!(session_id, "sess-001");
        assert_eq!(*exit_code, Some(1));
        assert!(reason.contains("code 1"));
    } else {
        panic!("wrong variant");
    }
}

#[test]
fn agent_event_session_terminated_stream_close() {
    let event = AgentEvent::SessionTerminated {
        session_id: "sess-001".into(),
        exit_code: None,
        reason: "stream closed".into(),
    };

    if let AgentEvent::SessionTerminated {
        exit_code, reason, ..
    } = &event
    {
        assert!(exit_code.is_none(), "stream close has no exit code");
        assert_eq!(reason, "stream closed");
    } else {
        panic!("wrong variant");
    }
}

#[test]
fn agent_event_clones_correctly() {
    let original = AgentEvent::StatusUpdated {
        session_id: "sess-001".into(),
        message: "test message".into(),
    };
    let cloned = original.clone();

    if let (
        AgentEvent::StatusUpdated {
            session_id: s1,
            message: m1,
        },
        AgentEvent::StatusUpdated {
            session_id: s2,
            message: m2,
        },
    ) = (&original, &cloned)
    {
        assert_eq!(s1, s2);
        assert_eq!(m1, m2);
    } else {
        panic!("clone produced wrong variant");
    }
}
