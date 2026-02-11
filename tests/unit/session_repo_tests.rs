use std::sync::Arc;

use monocoque_agent_rem::config::GlobalConfig;
use monocoque_agent_rem::models::session::{Session, SessionMode, SessionStatus};
use monocoque_agent_rem::persistence::{db, session_repo::SessionRepo};

fn config_for_tests() -> GlobalConfig {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = format!(
        r#"
default_workspace_root = '{root}'
http_port = 3000
ipc_name = "monocoque-agent-rem"
max_concurrent_sessions = 2
host_cli = "claude"
host_cli_args = ["--stdio"]
authorized_user_ids = ["U123", "U456"]

[slack]
channel_id = "C123"

[timeouts]
approval_seconds = 3600
prompt_seconds = 1800
wait_seconds = 0

[stall]
enabled = true
inactivity_threshold_seconds = 300
escalation_threshold_seconds = 120
max_retries = 3
default_nudge_message = "continue"

[commands]
status = "git status"
"#,
        root = temp.path().to_str().expect("utf8 path"),
    );

    GlobalConfig::from_toml_str(&toml).expect("valid config")
}

#[tokio::test]
async fn create_and_update_session() {
    let config = config_for_tests();
    let db = db::connect(&config, true).await.expect("db connect");
    let repo = SessionRepo::new(Arc::new(db));

    let session = Session::new(
        "U123".into(),
        "/test/workspace".into(),
        Some("hello".into()),
        SessionMode::Remote,
    );
    let created = repo.create(&session).await.expect("create session");
    assert_eq!(created.owner_user_id, "U123");

    let activated = repo
        .update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate session");
    assert_eq!(activated.status, SessionStatus::Active);

    let count = repo.count_active().await.expect("count active");
    assert_eq!(count, 1);

    let fetched = repo.get_by_id(&created.id).await.expect("fetch session");
    assert_eq!(fetched.status, SessionStatus::Active);
}
