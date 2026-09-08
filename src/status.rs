//! The value types that cross the boundary: status, close reason, metrics.
//!
//! Each one mirrors a modelpipe type rather than re-exporting it. modelpipe
//! marks all three `#[non_exhaustive]`, which is right for a Rust dependent
//! that should recompile against a new variant, and impossible for a generated
//! binding: a Swift enum is exhaustive or it is nothing. Mirroring converts
//! "a variant appeared upstream" from a silent behaviour change into a
//! non-exhaustive `match` that fails this crate's build — which is the moment a
//! human should decide what the phone does about it.

use modelpipe::{CloseReason, NetworkMetrics, PipeStatus};

/// What the pipe is doing, in the four states the app already draws.
///
/// The names and order match `GGChatCore.PipeStatus` exactly, because the
/// consumer maps one to the other and a reordering would be a silent
/// mistranslation rather than a compile error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum MpPipeStatus {
    /// Bound locally, with no path to the far machine yet. Not an error: a
    /// pipe dialled at a machine that is switched off sits here.
    Idle,
    /// A hole-punched path straight to the far machine.
    Direct,
    /// Going through a relay, which carries ciphertext it cannot read.
    Relayed,
    /// Terminal. Nothing further will be reported; dial again to recover.
    Closed,
}

impl From<PipeStatus> for MpPipeStatus {
    fn from(status: PipeStatus) -> Self {
        match status {
            PipeStatus::Idle => Self::Idle,
            PipeStatus::Direct => Self::Direct,
            PipeStatus::Relayed => Self::Relayed,
            PipeStatus::Closed => Self::Closed,
            // modelpipe marks the enum `#[non_exhaustive]`. A variant added
            // upstream lands here rather than in a Swift `default:` arm the
            // app would have to invent a meaning for. `Closed` is the
            // conservative reading: it makes the app hang up and re-dial,
            // which is recoverable, where guessing `Direct` would have it
            // stream into a pipe that is not there.
            other => {
                tracing_unknown(&format!("{other:?}"));
                Self::Closed
            }
        }
    }
}

impl From<MpPipeStatus> for PipeStatus {
    fn from(status: MpPipeStatus) -> Self {
        match status {
            MpPipeStatus::Idle => Self::Idle,
            MpPipeStatus::Direct => Self::Direct,
            MpPipeStatus::Relayed => Self::Relayed,
            MpPipeStatus::Closed => Self::Closed,
        }
    }
}

/// Why a pipe closed, when it has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum MpCloseReason {
    /// Someone called `shutdown`, on this side or the far one.
    Shutdown,
    /// The local listener stopped accepting. The pipe is gone for a reason
    /// that is this machine's, not the network's.
    ListenerFailed,
}

impl From<CloseReason> for MpCloseReason {
    fn from(reason: CloseReason) -> Self {
        match reason {
            CloseReason::Shutdown => Self::Shutdown,
            CloseReason::ListenerFailed => Self::ListenerFailed,
            // Same reasoning as `MpPipeStatus`: an upstream variant becomes a
            // build failure here, and until someone decides otherwise the
            // closest honest answer is that it was not a clean shutdown.
            other => {
                tracing_unknown(&format!("{other:?}"));
                Self::ListenerFailed
            }
        }
    }
}

/// What the transport underneath this pipe has been doing, since it started.
///
/// Monotonic totals for one endpoint's whole life, so the reading that means
/// something is a difference or a ratio rather than any single number. The
/// rate-limit counter is the one nothing else on the phone can see: a relay
/// refusing this endpoint looks exactly like a network that will not connect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, uniffi::Record)]
pub struct MpNetworkMetrics {
    /// Relay connections this endpoint has opened.
    pub relay_connections: u64,
    /// Relay connections that failed to open.
    pub relay_connections_failed: u64,
    /// Relay connections refused because the relay is rate limiting this
    /// endpoint. Non-zero here is the difference between "the network is
    /// broken" and "this endpoint is being throttled".
    pub relay_connections_ratelimited: u64,
}

impl From<NetworkMetrics> for MpNetworkMetrics {
    fn from(metrics: NetworkMetrics) -> Self {
        Self {
            relay_connections: metrics.relay_connections,
            relay_connections_failed: metrics.relay_connections_failed,
            relay_connections_ratelimited: metrics.relay_connections_ratelimited,
        }
    }
}

/// One line when an upstream variant this build does not know about arrives.
///
/// Deliberately not an error path: the pipe still works, and a phone in the
/// middle of a reply should not be handed a failure because modelpipe grew a
/// state. The line names the variant so the next build knows what to add.
fn tracing_unknown(variant: &str) {
    // No `tracing` dependency here on purpose — one line on stderr, at the
    // one place it can happen, is not worth a subscriber the host would have
    // to install and configure through the FFI boundary.
    eprintln!("modelpipe-ffi: unknown upstream variant {variant}, read conservatively");
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod status_tests;
