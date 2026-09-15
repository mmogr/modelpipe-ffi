//! Why a pairing did not pair, and why a wait did not reach.

use std::fmt;

use modelpipe::{PairError, PairingStringError, Unreached};

use crate::error::MpError;
use crate::status::{MpCloseReason, tracing_unknown};

/// Why [`crate::MpPipe::wait_reachable`] gave up.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Error)]
pub enum MpUnreached {
    /// The wait ran out. The pipe keeps looking.
    TimedOut {
        /// How long it waited.
        within_ms: u64,
    },
    /// The pipe closed before it reached the far machine.
    Closed {
        /// Why it closed, when anything recorded it.
        reason: Option<MpCloseReason>,
    },
}

#[uniffi::export]
impl MpUnreached {
    /// The sentence to show a person.
    #[must_use]
    pub fn message(&self) -> String {
        self.to_string()
    }

    /// Whether waiting again could plausibly reach it.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        match self {
            Self::TimedOut { .. } => true,
            Self::Closed { .. } => false,
        }
    }
}

impl fmt::Display for MpUnreached {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TimedOut { .. } => write!(
                f,
                "The other machine did not answer in time. Check it is awake and still sharing."
            ),
            Self::Closed { .. } => {
                write!(f, "The connection ended before the other machine answered.")
            }
        }
    }
}

impl std::error::Error for MpUnreached {}

impl From<Unreached> for MpUnreached {
    fn from(unreached: Unreached) -> Self {
        match unreached {
            Unreached::TimedOut(within) => Self::TimedOut {
                within_ms: u64::try_from(within.as_millis()).unwrap_or(u64::MAX),
            },
            Unreached::Closed(reason) => Self::Closed {
                reason: reason.map(Into::into),
            },
            // `Unreached` is `#[non_exhaustive]`. A reason added upstream lands
            // here rather than in a Swift `default:` arm; "the pipe ended" is
            // the conservative reading.
            other => {
                tracing_unknown(&format!("{other:?}"));
                Self::Closed { reason: None }
            }
        }
    }
}

/// Why a pairing did not produce a key.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Error)]
pub enum MpPairError {
    /// The pairing string is a ticket alone, with no code to redeem.
    NoCode,
    /// The pairing string is not one.
    BadPairingString {
        /// What was wrong with it, in a sentence. Never contains the string.
        reason: String,
    },
    /// The pipe to pair over could not be set up.
    Dial {
        /// The sentence [`MpError`] would have shown.
        reason: String,
        /// Whether dialling again could succeed.
        retryable: bool,
    },
    /// The far machine was not reached in the time given.
    Unreached {
        /// Which way the wait ended.
        why: MpUnreached,
    },
    /// The far machine refused the code.
    Refused,
    /// The code could not be presented, or its answer could not be read.
    Exchange {
        /// The operating system's reason.
        reason: String,
    },
    /// The far machine answered with something that is not a pairing answer.
    Unexpected {
        /// What was wrong with the answer.
        detail: String,
    },
    /// Something modelpipe grew that this build does not know about.
    Unknown {
        /// The upstream `Debug` rendering, for a bug report.
        detail: String,
    },
}

#[uniffi::export]
impl MpPairError {
    /// The sentence to show a person.
    #[must_use]
    pub fn message(&self) -> String {
        self.to_string()
    }

    /// Whether pairing again could succeed without anything else changing.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        match self {
            Self::Dial { retryable, .. } => *retryable,
            Self::Unreached { why } => why.is_retryable(),
            Self::Exchange { .. } | Self::Unknown { .. } => true,
            Self::NoCode
            | Self::BadPairingString { .. }
            | Self::Refused
            | Self::Unexpected { .. } => false,
        }
    }
}

impl fmt::Display for MpPairError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoCode => write!(
                f,
                "That pairing string has no code in it. Ask the other machine for a new one."
            ),
            Self::BadPairingString { reason } | Self::Dial { reason, .. } => {
                write!(f, "{reason}")
            }
            Self::Unreached { why } => write!(f, "{why}"),
            Self::Refused => write!(
                f,
                "The other machine did not accept that code. It may be wrong, expired or already \
                 used, so ask for a new one."
            ),
            Self::Exchange { reason } => write!(
                f,
                "The pairing could not be completed: {}.",
                reason.trim().trim_end_matches('.').trim_end()
            ),
            Self::Unexpected { detail } => write!(
                f,
                "The other machine's answer was not a pairing answer ({detail})."
            ),
            Self::Unknown { detail } => write!(
                f,
                "The pairing failed for a reason this app does not recognise ({detail})."
            ),
        }
    }
}

impl std::error::Error for MpPairError {}

impl From<PairingStringError> for MpPairError {
    fn from(error: PairingStringError) -> Self {
        Self::BadPairingString {
            reason: format!("That pairing string could not be read ({error})."),
        }
    }
}

impl From<PairError> for MpPairError {
    fn from(error: PairError) -> Self {
        match error {
            PairError::NoCode => Self::NoCode,
            PairError::Connect(e) => {
                let retryable = e.is_retryable();
                Self::Dial {
                    reason: MpError::from(e).message(),
                    retryable,
                }
            }
            PairError::Unreached(u) => Self::Unreached { why: u.into() },
            PairError::Refused => Self::Refused,
            PairError::Exchange(e) => Self::Exchange {
                reason: e.to_string(),
            },
            PairError::Unexpected(why) => Self::Unexpected {
                detail: why.to_owned(),
            },
            other => Self::Unknown {
                detail: format!("{other:?}"),
            },
        }
    }
}
