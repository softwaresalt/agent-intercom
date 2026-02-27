//! Session orchestration modules.
//!
//! Covers agent process spawning, session lifecycle management,
//! checkpoint creation/restore, stall detection, stall event
//! dispatching, and child process monitoring.

pub mod checkpoint_manager;
pub mod child_monitor;
pub mod session_manager;
pub mod spawner;
pub mod stall_consumer;
pub mod stall_detector;
