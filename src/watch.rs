//! An explicit, cancellable status wait.
//!
//! uniffi 0.32's Swift generator never calls `rust_future_cancel`, so a Swift
//! `Task` that is cancelled while awaiting [`MpPipe::status_changed_since`]
//! leaves the Rust future parked until the next status change. This is that
//! cancellation, expressed as an object the caller holds: a watch's `next`
//! ends when the status moves on, when the pipe closes, or when the watch is
//! cancelled, whichever comes first.
//!
//! [`MpPipe::status_changed_since`]: crate::MpPipe::status_changed_since

use std::sync::Arc;

use modelpipe::ConnectHandle;
use tokio_util::sync::CancellationToken;

use crate::status::MpPipeStatus;

/// A cancellable view of one pipe's status sequence.
///
/// `statusChangedSince` stays for a caller that ends its wait by closing the
/// pipe; a watch is for ending a wait while the pipe stays up.
#[derive(uniffi::Object)]
pub struct MpWatch {
    handle: Arc<ConnectHandle>,
    /// Stored state rather than a notification: `cancelled()` completes at
    /// once when `cancel` already ran, so there is no window between a check
    /// and a wait registration for a cancel to fall through.
    cancel: CancellationToken,
}

impl MpWatch {
    /// A watch over the pipe the handle belongs to.
    pub(crate) fn new(handle: Arc<ConnectHandle>) -> Self {
        Self {
            handle,
            cancel: CancellationToken::new(),
        }
    }
}

#[uniffi::export(async_runtime = "tokio")]
impl MpWatch {
    /// Wait for a status different from `snapshot`. `None` once the pipe is
    /// closed and `snapshot` already says so, or once [`cancel`](Self::cancel)
    /// has been called, whether before this wait began or during it.
    pub async fn next(&self, snapshot: MpPipeStatus) -> Option<MpPipeStatus> {
        tokio::select! {
            // Cancellation is checked first, so a cancelled watch answers
            // `None` even when a status change is ready at the same moment.
            biased;
            () = self.cancel.cancelled() => None,
            next = self.handle.status_changed_since(snapshot.into()) => next.map(Into::into),
        }
    }

    /// End every wait on this watch, in flight or yet to start.
    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    /// Whether [`cancel`](Self::cancel) has been called.
    pub fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }
}

impl std::fmt::Debug for MpWatch {
    /// Hand-written, like the pipe's: the status it watches and whether it
    /// has been cancelled are the two facts worth logging, and the handle
    /// behind it renders nothing this crate chose.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MpWatch")
            .field("status", &self.handle.status())
            .field("cancelled", &self.cancel.is_cancelled())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[path = "watch_tests.rs"]
mod watch_tests;
