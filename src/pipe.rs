//! The dial, and the object it hands back.

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use modelpipe::{ConnectHandle, Ticket};

use crate::error::MpError;
use crate::options::MpConnectOptions;
use crate::runtime::{block_on, runtime};
use crate::status::{MpCloseReason, MpNetworkMetrics, MpPipeStatus};

/// How long `shutdown` waits for in-flight requests before dropping them.
///
/// Short on purpose. The caller is a phone being backgrounded, and iOS gives a
/// process a few seconds at most before suspending it; a grace period longer
/// than that is a promise this side cannot keep. Two seconds is enough for a
/// response already on the wire and not enough to be noticed.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(2);

/// A live pipe: a loopback listener on this device that *is* the far
/// machine's model server.
///
/// Held by Swift as a reference-counted object. Dropping the last reference
/// tears the pipe down, so an app that loses its last reference without
/// calling [`Self::shutdown`] still leaves nothing bound — but call it anyway,
/// because only that path tells the far side rather than leaving it to time
/// out.
#[derive(uniffi::Object)]
pub struct MpPipe {
    handle: ConnectHandle,
}

/// Dial the machine a pairing ticket names.
///
/// Returns **as soon as the local port is bound**, not once the far machine
/// answers. That is upstream's contract and it is the one the app is built
/// around: the returned pipe starts at [`MpPipeStatus::Idle`] and walks to
/// `Relayed` or `Direct` afterwards, which is what the status pill shows. A
/// machine that is switched off produces a perfectly good pipe that stays
/// `Idle` — it is not an error, and treating it as one would mean a dial that
/// blocks for the thirty seconds iroh spends giving up.
///
/// Takes no token, because there is nothing to take: see [`MpConnectOptions`].
///
/// # Errors
///
/// [`MpError::BadTicket`] or [`MpError::UnsupportedTicketVersion`] if the
/// pairing string is not one, before anything touches the network; then
/// whatever the dial itself refuses with.
#[uniffi::export(async_runtime = "tokio")]
pub async fn mp_connect(ticket: String, options: MpConnectOptions) -> Result<Arc<MpPipe>, MpError> {
    // Parsed first, and on the caller's thread: a malformed ticket should cost
    // nothing and should not be indistinguishable from a machine that is away.
    let ticket = Ticket::from_str(&ticket)?;
    let handle = modelpipe::connect(&ticket, options.apply()).await?;
    Ok(Arc::new(MpPipe { handle }))
}

#[uniffi::export(async_runtime = "tokio")]
impl MpPipe {
    /// `http://127.0.0.1:<port>/v1` — point an OpenAI-compatible client here,
    /// with the far machine's key as the API key.
    ///
    /// Stable for the life of the pipe.
    pub fn base_url(&self) -> String {
        self.handle.base_url()
    }

    /// The loopback port this pipe bound.
    ///
    /// [`Self::base_url`] is what a client wants; this is for a log line or a
    /// diagnostic screen that wants the number on its own.
    pub fn port(&self) -> u16 {
        self.handle.local_addr().port()
    }

    /// What the pipe is doing right now.
    pub fn status(&self) -> MpPipeStatus {
        self.handle.status().into()
    }

    /// Wait for a status different from `snapshot`, then return it; `None`
    /// once the pipe is closed and `snapshot` already says so.
    ///
    /// The caller supplies the snapshot, which is the whole point: a
    /// transition landing between reading [`Self::status`] and waiting again
    /// is reported rather than coalesced away, and the sequence *ends* instead
    /// of answering `Closed` forever at whatever rate it is asked.
    ///
    /// The Swift shape this is for:
    ///
    /// ```swift
    /// var held = pipe.status()
    /// continuation.yield(held)                       // current value first
    /// while let next = await pipe.statusChangedSince(snapshot: held) {
    ///     held = next
    ///     continuation.yield(held)
    /// }
    /// continuation.finish()                          // only after a close
    /// ```
    pub async fn status_changed_since(&self, snapshot: MpPipeStatus) -> Option<MpPipeStatus> {
        self.handle
            .status_changed_since(snapshot.into())
            .await
            .map(Into::into)
    }

    /// Why the pipe closed, or `None` while it is open.
    pub fn close_reason(&self) -> Option<MpCloseReason> {
        self.handle.close_reason().map(Into::into)
    }

    /// Tell the endpoint the network underneath it may have changed.
    ///
    /// Call this from the app's own foreground/resume handling, every time,
    /// without trying to work out whether it was needed. It is harmless when
    /// nothing changed and harmless when the endpoint already noticed.
    ///
    /// It exists because iOS is one of the hosts iroh cannot watch for itself:
    /// sleep/wake detection there is deliberately disabled in favour of a poll
    /// measured in the hour, so a phone that resumes on a new cellular bearer
    /// has a pipe with nothing to repair it until that poll comes round —
    /// unless the app says so, here, from the resume it already handles.
    pub async fn notify_network_change(&self) {
        self.handle.notify_network_change().await;
    }

    /// Transport counters for this pipe's endpoint.
    pub fn network_metrics(&self) -> MpNetworkMetrics {
        self.handle.network_metrics().into()
    }

    /// Close the pipe, telling the far side rather than leaving it to time
    /// out.
    ///
    /// Idempotent: calling it twice is fine, and the second call returns at
    /// once. Afterwards the base URL refuses connections rather than hanging,
    /// and the status sequence has ended.
    pub async fn shutdown(&self) {
        self.handle.shutdown_timeout(SHUTDOWN_GRACE).await;
    }
}

impl std::fmt::Debug for MpPipe {
    /// Hand-written rather than derived, for the same reason modelpipe writes
    /// its own: a derive renders whatever the inner handle renders, and that
    /// is a decision this crate should make deliberately rather than inherit.
    /// Port and status are the two facts worth having in a log; neither the
    /// ticket nor anything else credential-shaped is reachable from here, and
    /// this keeps it that way by construction.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MpPipe")
            .field("port", &self.handle.local_addr().port())
            .field("status", &self.handle.status())
            .finish()
    }
}

impl Drop for MpPipe {
    fn drop(&mut self) {
        // `ConnectHandle`'s own `Drop` is synchronous and closes the listener,
        // so the port is never left bound. What it cannot do from a `Drop` is
        // the async half that tells the far side, and a Swift object losing
        // its last reference is a perfectly ordinary way for a pipe to end —
        // an app dismissing a screen mid-dial does it. Entering the runtime
        // here buys that courtesy where there is a runtime to enter.
        //
        // Guarded on not already being inside it: `block_on` from a runtime
        // thread panics, and a `Drop` that panics while unwinding aborts the
        // process. Inside the runtime, the synchronous `Drop` below is all
        // that happens, which is correct rather than merely safe.
        if tokio::runtime::Handle::try_current().is_err() {
            let _guard = runtime().enter();
            block_on(self.handle.shutdown_timeout(Duration::ZERO));
        }
    }
}

#[cfg(test)]
#[path = "pipe_tests.rs"]
mod pipe_tests;
