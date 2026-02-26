//! Integration tests for server startup reliability (US2).
//!
//! Covers scenarios S023-S026: normal startup, port conflict detection,
//! clean exit on bind failure.

use agent_intercom::mcp::sse::bind_http;

use super::test_helpers::test_app_state;

/// Create a config with `http_port` set to a specific value.
fn config_with_port(port: u16) -> agent_intercom::config::GlobalConfig {
    let tmp = std::env::temp_dir();
    let root = tmp.to_string_lossy().replace('\\', "/");
    let toml = format!(
        r#"
default_workspace_root = '{root}'
http_port = {port}
ipc_name = "test-startup"
max_concurrent_sessions = 5
host_cli = "echo"

[slack]
channel_id = "C_TEST"

[timeouts]
approval_seconds = 2
prompt_seconds = 2
wait_seconds = 2

[stall]
enabled = false
inactivity_threshold_seconds = 300
escalation_threshold_seconds = 120
max_retries = 3
default_nudge_message = "continue"
"#
    );
    agent_intercom::config::GlobalConfig::from_toml_str(&toml).expect("valid test config")
}

/// S023: Normal startup — bind succeeds on an available port.
#[tokio::test]
async fn bind_http_succeeds_on_free_port() {
    // port 0 lets the OS pick a free ephemeral port
    let config = config_with_port(0);
    let state = test_app_state(config).await;

    let result = bind_http(&state).await;
    assert!(
        result.is_ok(),
        "bind_http should succeed on free port 0, got: {result:?}"
    );
    let listener = result.unwrap();
    let addr = listener.local_addr().expect("listener has local addr");
    assert_ne!(addr.port(), 0, "OS should have assigned a real port");
}

/// S024: Port conflict — bind fails when the port is already in use.
#[tokio::test]
async fn bind_http_fails_on_occupied_port() {
    // Occupy a random ephemeral port first.
    let occupied = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("initial bind succeeds");
    let port = occupied.local_addr().expect("has local addr").port();

    let config = config_with_port(port);
    let state = test_app_state(config).await;

    let result = bind_http(&state).await;
    assert!(
        result.is_err(),
        "bind_http should fail on occupied port {port}"
    );

    // Error message should contain the port number for diagnostics.
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("failed to bind") || msg.contains(&port.to_string()),
        "error message should describe the bind failure, got: {msg}"
    );
}

/// S025: `bind_http` returns `AppError::Config` (not a panic) on failure.
#[tokio::test]
async fn bind_http_returns_config_error_variant() {
    let occupied = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("initial bind succeeds");
    let port = occupied.local_addr().expect("has local addr").port();

    let config = config_with_port(port);
    let state = test_app_state(config).await;

    let err = bind_http(&state).await.expect_err("should fail");
    // Verify the error is the Config variant (not Db, Slack, etc.)
    assert!(
        matches!(err, agent_intercom::AppError::Config(_)),
        "expected AppError::Config, got: {err:?}"
    );
}

/// S026: Second bind attempt after successful one fails gracefully.
#[tokio::test]
async fn second_bind_on_same_port_fails() {
    let config = config_with_port(0);
    let state = test_app_state(config.clone()).await;

    let listener = bind_http(&state).await.expect("first bind succeeds");
    let port = listener.local_addr().expect("has addr").port();

    // Now try to bind the same port again (simulating a second instance).
    let config2 = config_with_port(port);
    let state2 = test_app_state(config2).await;
    let result = bind_http(&state2).await;
    assert!(result.is_err(), "second bind on port {port} should fail");
}
