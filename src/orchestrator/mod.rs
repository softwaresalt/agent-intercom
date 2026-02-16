//! Session orchestration modules.
//!
//! Covers agent process spawning, session lifecycle management,
//! checkpoint creation/restore, and stall detection.

pub mod checkpoint_manager;
pub mod session_manager;
pub mod spawner;
pub mod stall_detector;
