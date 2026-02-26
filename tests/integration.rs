#![allow(clippy::expect_used, clippy::unwrap_used, missing_docs)]

mod integration {
    mod test_helpers;

    mod approval_flow_tests;
    mod call_tool_dispatch_tests;
    mod channel_override_tests;
    mod checkpoint_manager_tests;
    mod crash_recovery_tests;
    mod diff_apply_tests;
    mod handler_accept_diff_tests;
    mod handler_auto_approve_tests;
    mod handler_blocking_tests;
    mod handler_edge_case_tests;
    mod handler_heartbeat_tests;
    mod handler_mode_tests;
    mod handler_recover_tests;
    mod handler_remote_log_tests;
    mod health_endpoint_tests;
    mod nudge_flow_tests;
    mod on_initialized_tests;
    mod prompt_flow_tests;
    mod retention_tests;
    mod session_lifecycle_tests;
    mod session_manager_tests;
    mod shutdown_recovery_tests;
    mod stall_escalation_tests;

    mod ipc_server_tests;
    mod mcp_dispatch_tests;
    mod policy_watcher_tests;
    mod startup_tests;
    mod stdio_transport_tests;
    mod steering_flow_tests;
    mod streamable_http_tests;
}
