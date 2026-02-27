//! Per-session stall detection timer with auto-nudge escalation.
//!
//! Each active session gets a [`StallDetector`] that fires after a
//! configurable inactivity threshold. The timer can be [`reset`](StallDetectorHandle::reset)
//! on any MCP activity, [`paused`](StallDetectorHandle::pause) during long-running
//! operations, and [`resumed`](StallDetectorHandle::resume) afterwards.
//!
//! Events are delivered via a `tokio::sync::mpsc` channel so the
//! orchestrator can react (post Slack alerts, issue nudges, escalate).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, Notify};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, info_span, warn, Instrument};

/// Events emitted by the stall detector for orchestrator handling.
#[derive(Debug, Clone)]
pub enum StallEvent {
    /// Agent has been idle past the inactivity threshold.
    Stalled {
        /// Session whose agent went silent.
        session_id: String,
        /// Seconds idle when the event was generated.
        idle_seconds: u64,
    },
    /// Auto-nudge triggered after escalation threshold with no operator response.
    AutoNudge {
        /// Session whose agent is still stalled.
        session_id: String,
        /// Cumulative nudge count (1-based).
        nudge_count: u32,
    },
    /// Max retries exceeded — escalated alert.
    Escalated {
        /// Session whose agent exceeded max nudge retries.
        session_id: String,
        /// Final nudge count at escalation.
        nudge_count: u32,
    },
    /// Agent resumed activity while a stall alert was active.
    SelfRecovered {
        /// Session whose agent self-recovered.
        session_id: String,
    },
}

/// Builder for a per-session stall detector.
///
/// Call [`spawn`](Self::spawn) to start the background timer task.
pub struct StallDetector {
    session_id: String,
    inactivity_threshold: Duration,
    escalation_interval: Duration,
    max_retries: u32,
    event_tx: mpsc::Sender<StallEvent>,
    cancel: CancellationToken,
}

impl StallDetector {
    /// Construct a new detector (does not start the timer yet).
    #[must_use]
    pub fn new(
        session_id: String,
        inactivity_threshold: Duration,
        escalation_interval: Duration,
        max_retries: u32,
        event_tx: mpsc::Sender<StallEvent>,
        cancel: CancellationToken,
    ) -> Self {
        Self {
            session_id,
            inactivity_threshold,
            escalation_interval,
            max_retries,
            event_tx,
            cancel,
        }
    }

    /// Spawn the background timer task and return a handle for controlling it.
    #[must_use]
    pub fn spawn(self) -> StallDetectorHandle {
        let reset_notify = Arc::new(Notify::new());
        let paused = Arc::new(AtomicBool::new(false));
        let stalled = Arc::new(AtomicBool::new(false));

        // Clone the cancellation token so the handle can cancel the task on drop.
        let cancel_for_handle = self.cancel.clone();

        let task_handle = tokio::spawn(
            Self::run(
                self.session_id.clone(),
                self.inactivity_threshold,
                self.escalation_interval,
                self.max_retries,
                self.event_tx,
                self.cancel,
                Arc::clone(&reset_notify),
                Arc::clone(&paused),
                Arc::clone(&stalled),
            )
            .instrument(info_span!("stall_detector")),
        );

        StallDetectorHandle {
            reset_notify,
            paused,
            stalled,
            session_id: self.session_id,
            join_handle: Some(task_handle),
            cancel: cancel_for_handle,
        }
    }

    /// Core timer loop.
    #[allow(clippy::too_many_arguments)] // Internal plumbing; not part of public API width.
    async fn run(
        session_id: String,
        inactivity_threshold: Duration,
        escalation_interval: Duration,
        max_retries: u32,
        event_tx: mpsc::Sender<StallEvent>,
        cancel: CancellationToken,
        reset_notify: Arc<Notify>,
        paused: Arc<AtomicBool>,
        stalled: Arc<AtomicBool>,
    ) {
        let mut nudge_count: u32 = 0;

        loop {
            // ── Wait for inactivity threshold or reset ───────
            let fired = tokio::select! {
                () = cancel.cancelled() => {
                    debug!(session_id, "stall detector cancelled");
                    return;
                }
                () = Self::wait_unless_paused(
                    inactivity_threshold,
                    &paused,
                    &reset_notify,
                    &cancel,
                ) => true,
                () = reset_notify.notified() => false,
            };

            if !fired {
                // Reset received before threshold — check self-recovery.
                if stalled.swap(false, Ordering::SeqCst) {
                    info!(session_id, "agent self-recovered");
                    nudge_count = 0;
                    let _ = event_tx
                        .send(StallEvent::SelfRecovered {
                            session_id: session_id.clone(),
                        })
                        .await;
                }
                continue;
            }

            // ── Stall detected ───────────────────────────────
            stalled.store(true, Ordering::SeqCst);
            let idle_secs = inactivity_threshold.as_secs();
            info!(session_id, idle_secs, "stall detected");

            let _ = event_tx
                .send(StallEvent::Stalled {
                    session_id: session_id.clone(),
                    idle_seconds: idle_secs,
                })
                .await;

            // ── Escalation loop ──────────────────────────────
            loop {
                let escalation_fired = tokio::select! {
                    () = cancel.cancelled() => return,
                    () = tokio::time::sleep(escalation_interval) => true,
                    () = reset_notify.notified() => false,
                };

                if !escalation_fired {
                    // Agent self-recovered during escalation.
                    if stalled.swap(false, Ordering::SeqCst) {
                        info!(session_id, "agent self-recovered during escalation");
                        nudge_count = 0;
                        let _ = event_tx
                            .send(StallEvent::SelfRecovered {
                                session_id: session_id.clone(),
                            })
                            .await;
                    }
                    break;
                }

                nudge_count += 1;

                if nudge_count > max_retries {
                    warn!(session_id, nudge_count, "stall escalated past max retries");
                    let _ = event_tx
                        .send(StallEvent::Escalated {
                            session_id: session_id.clone(),
                            nudge_count,
                        })
                        .await;
                    // Stay stalled but stop escalating — wait for manual intervention or reset.
                    tokio::select! {
                        () = cancel.cancelled() => return,
                        () = reset_notify.notified() => {
                            if stalled.swap(false, Ordering::SeqCst) {
                                nudge_count = 0;
                                let _ = event_tx
                                    .send(StallEvent::SelfRecovered {
                                        session_id: session_id.clone(),
                                    })
                                    .await;
                            }
                            break;
                        }
                    }
                }

                info!(session_id, nudge_count, "auto-nudge");
                let _ = event_tx
                    .send(StallEvent::AutoNudge {
                        session_id: session_id.clone(),
                        nudge_count,
                    })
                    .await;
            }
        }
    }

    /// Sleep for a duration while respecting the pause flag.
    ///
    /// If paused, waits until unpaused before starting the sleep.
    /// If a reset fires during sleep, the future completes early via
    /// `Notify::notified` in the outer `select!`.
    ///
    /// When paused, this function polls at 50 ms intervals until unpaused.
    /// A `Notify`-based approach would be more efficient, but the pause
    /// state is rare (only during long-running server operations) and the
    /// overhead is negligible for this use case.
    async fn wait_unless_paused(
        duration: Duration,
        paused: &AtomicBool,
        reset_notify: &Notify,
        cancel: &CancellationToken,
    ) {
        // If paused, spin-wait with a short poll interval.
        while paused.load(Ordering::SeqCst) {
            tokio::select! {
                () = cancel.cancelled() => return,
                () = reset_notify.notified() => return,
                () = tokio::time::sleep(Duration::from_millis(50)) => {}
            }
        }
        tokio::time::sleep(duration).await;
    }
}

/// Handle returned from [`StallDetector::spawn`] for controlling the timer.
pub struct StallDetectorHandle {
    reset_notify: Arc<Notify>,
    paused: Arc<AtomicBool>,
    stalled: Arc<AtomicBool>,
    session_id: String,
    /// Task handle for the background detector loop.
    join_handle: Option<JoinHandle<()>>,
    /// Per-session cancellation token — cancelled when the handle is dropped.
    cancel: CancellationToken,
}

impl Drop for StallDetectorHandle {
    /// Cancel the background detector task when the handle is dropped.
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

impl StallDetectorHandle {
    /// Reset the inactivity timer (call on every tool activity or heartbeat).
    pub fn reset(&self) {
        self.reset_notify.notify_one();
    }

    /// Pause stall detection (e.g., during long-running server operations).
    pub fn pause(&self) {
        self.paused.store(true, Ordering::SeqCst);
    }

    /// Resume stall detection after a pause.
    pub fn resume(&self) {
        self.paused.store(false, Ordering::SeqCst);
        self.reset_notify.notify_one();
    }

    /// Whether the detector currently considers the session stalled.
    #[must_use]
    pub fn is_stalled(&self) -> bool {
        self.stalled.load(Ordering::SeqCst)
    }

    /// The session ID this handle controls.
    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }
}

impl StallDetectorHandle {
    /// Await the detector's completion.
    ///
    /// Signals the background task to stop via the cancellation token, then
    /// waits for it to exit.  If no `JoinHandle` is stored, this is a no-op.
    pub async fn await_completion(mut self) {
        self.cancel.cancel();
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.await;
        }
    }
}
