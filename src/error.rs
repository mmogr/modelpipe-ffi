//! The one error type that crosses the boundary.
//!
//! Flattened from modelpipe's `ConnectError` plus this crate's own ticket
//! parsing, because the Swift side does not act on the distinction between an
//! endpoint failure and a bind failure — it shows a sentence and offers a
//! retry. What it *does* act on is whether retrying is worth anything, so
//! that is the one bit kept, as [`MpError::is_retryable`].

use std::fmt;

use modelpipe::{ConnectError, TicketParseError};

/// Why a dial did not produce a pipe.
///
/// Every variant carries a sentence rather than a code, because the app shows
/// it. None of them carries the ticket: a ticket is a bearer credential, and
/// an error rendered into a log or a crash report is exactly where one should
/// not appear.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Error)]
pub enum MpError {
    /// The ticket is not a ticket: wrong prefix, bad base32, failed checksum,
    /// or a length outside the format's bounds.
    ///
    /// Distinct from every other variant because it is the only one the caller
    /// can fix by looking at what they pasted.
    BadTicket {
        /// What was wrong with it, in a sentence. Never contains the ticket.
        reason: String,
    },
    /// The ticket names a version of the format this build does not implement.
    UnsupportedTicketVersion {
        /// The version byte the ticket carried.
        version: u8,
    },
    /// The local loopback port could not be bound. Nothing to do with the far
    /// machine; another process most likely holds it.
    Bind {
        /// The operating system's reason.
        reason: String,
    },
    /// The endpoint underneath could not start. A local failure again, but of
    /// the transport rather than of one socket.
    Endpoint {
        /// The operating system's reason.
        reason: String,
    },
    /// A relay URL, in the ticket or in the options, is not a URL.
    InvalidRelay {
        /// The offending value. A relay URL is public infrastructure, not a
        /// credential, so this one is safe to render.
        url: String,
    },
    /// The far machine could not be reached at all.
    PeerUnreachable,
    /// Something modelpipe grew that this build does not know about.
    ///
    /// `ConnectError` is `#[non_exhaustive]`; this is where a new variant
    /// lands until someone maps it. Retryable, on the reasoning that an
    /// unknown transport failure is more often transient than permanent.
    Unknown {
        /// The upstream `Debug` rendering, for a bug report.
        detail: String,
    },
}

// Exported, not merely public. Both methods below existed and were tested on
// the Rust side long before this attribute did, and neither crossed the
// boundary: Swift got a bare enum, `isRetryable()` did not compile in the
// consumer, and `message()` did not exist to be missed. A `pub fn` on a type
// that crosses the FFI is not part of the FFI.
#[uniffi::export]
impl MpError {
    /// The sentence to show a person.
    ///
    /// Use this, and not Swift's `localizedDescription`. `UniFFI` generates
    /// `errorDescription` for every error enum as `String(reflecting: self)`,
    /// which is the *debug* rendering of the case and its payload:
    ///
    /// ```text
    /// modelpipe_ffi.MpError.Bind(reason: "Address already in use (os error 48)")
    /// ```
    ///
    /// That compiles, reads as a plausible message, and shows somebody the
    /// inside of the binding. This is the [`Display`] impl below — the one
    /// written to be read — and the Swift smoke test asserts the difference
    /// so it cannot quietly become the other thing.
    ///
    /// [`Display`]: std::fmt::Display
    #[must_use]
    pub fn message(&self) -> String {
        self.to_string()
    }

    /// Whether dialling again could plausibly succeed without anything else
    /// changing.
    ///
    /// The app uses this to decide between offering a retry and telling the
    /// person to go fix something. A bad ticket is not retryable however many
    /// times it is pasted; a peer that was asleep may not be next time.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        match self {
            Self::BadTicket { .. }
            | Self::UnsupportedTicketVersion { .. }
            | Self::Bind { .. }
            | Self::InvalidRelay { .. } => false,
            Self::Endpoint { .. } | Self::PeerUnreachable | Self::Unknown { .. } => true,
        }
    }
}

impl fmt::Display for MpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadTicket { reason } => write!(f, "{reason}"),
            Self::UnsupportedTicketVersion { version } => write!(
                f,
                "This pairing string is version {version}, which this app is too old to read. \
                 Update the app."
            ),
            Self::Bind { reason } => write!(
                f,
                "This device could not open a local port for the pipe: {}.",
                trimmed(reason)
            ),
            Self::Endpoint { reason } => write!(
                f,
                "This device could not start the connection: {}.",
                trimmed(reason)
            ),
            Self::InvalidRelay { url } => {
                write!(f, "The relay address {url} is not a valid URL.")
            }
            Self::PeerUnreachable => write!(
                f,
                "The other machine did not answer. Check it is awake and still sharing."
            ),
            Self::Unknown { detail } => write!(
                f,
                "The connection failed for a reason this app does not recognise ({detail})."
            ),
        }
    }
}

impl std::error::Error for MpError {}

/// An operating-system reason, ready to be the tail of a sentence.
///
/// These arrive from `io::Error` and are inconsistent about their own
/// punctuation: some end in a full stop, most do not, and a few carry trailing
/// whitespace. Interpolating one raw produced a message that stopped
/// mid-sentence — which the app shows to a person, so it is a defect rather
/// than untidiness. Trimming here lets the format string own the full stop and
/// stops "in use.." when the reason brought its own.
fn trimmed(reason: &str) -> &str {
    reason.trim().trim_end_matches('.').trim_end()
}

impl From<TicketParseError> for MpError {
    fn from(error: TicketParseError) -> Self {
        match error {
            TicketParseError::Malformed => Self::BadTicket {
                reason: "That does not look like a pairing string. Copy the whole thing, \
                         including the `pipe` at the front."
                    .to_owned(),
            },
            TicketParseError::UnsupportedVersion(version) => {
                Self::UnsupportedTicketVersion { version }
            }
            other => Self::BadTicket {
                reason: format!("That pairing string could not be read ({other:?})."),
            },
        }
    }
}

impl From<ConnectError> for MpError {
    fn from(error: ConnectError) -> Self {
        // `Display` on the io-backed variants deliberately omits the source,
        // so read the source directly to get an operating-system reason worth
        // showing. Falling back to the variant's own `Display` keeps this
        // total when a future variant carries no source.
        let reason = |error: &ConnectError| {
            std::error::Error::source(error).map_or_else(|| error.to_string(), ToString::to_string)
        };
        match &error {
            ConnectError::PeerUnreachable => Self::PeerUnreachable,
            ConnectError::Bind(_) => Self::Bind {
                reason: reason(&error),
            },
            ConnectError::Endpoint(_) => Self::Endpoint {
                reason: reason(&error),
            },
            ConnectError::InvalidRelay { url } => Self::InvalidRelay { url: url.clone() },
            other => Self::Unknown {
                detail: format!("{other:?}"),
            },
        }
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod error_tests;
