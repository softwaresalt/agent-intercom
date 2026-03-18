//! Tier 2 live Slack integration test harness.
//!
//! Feature-gated behind `live-slack-tests`; compiled and run only when that
//! feature is explicitly enabled:
//!
//! ```text
//! cargo test --test live --features live-slack-tests
//! ```
//!
//! Requires two environment variables to be set before running:
//!
//! | Variable                | Purpose                                        |
//! |-------------------------|------------------------------------------------|
//! | `SLACK_TEST_BOT_TOKEN`  | Bot token authorised to post in the test channel |
//! | `SLACK_TEST_CHANNEL_ID` | Slack channel ID for posting and verifying messages |
//!
//! When the variables are absent the tests skip gracefully — they do **not**
//! fail, allowing local development without live credentials.
#![allow(clippy::expect_used, clippy::unwrap_used, missing_docs, dead_code)]

mod live {
    mod live_command_tests;
    pub(crate) mod live_helpers;
    mod live_interaction_tests;
    mod live_message_tests;
    mod live_modal_tests;
    mod live_threading_tests;
}
