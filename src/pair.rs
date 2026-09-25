//! Pairing, across the boundary: a pairing string in, a key and a live pipe
//! out. And, for a form, a pairing string read without any of that.

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use modelpipe::{ConnectError, PairError, PairingString};

use crate::identity_file;
use crate::options::MpConnectOptions;
use crate::pair_error::MpPairError;
use crate::pipe::MpPipe;
use crate::runtime::in_runtime;

/// What a first pairing produces: this device's key, the name it is held
/// under, the machine that issued it, and the pipe it was issued over.
#[derive(uniffi::Record)]
pub struct MpPaired {
    /// The pipe the code was redeemed over, still up.
    pub pipe: Arc<MpPipe>,
    /// This device's key from now on. Store it; nothing hands it out again.
    pub api_key: String,
    /// The name the far machine holds the key under.
    pub device: String,
    /// The far machine's endpoint id, sixty-four hex characters.
    pub serving: String,
}

impl std::fmt::Debug for MpPaired {
    /// Hand-written, and the one place in this crate where that is a security
    /// property rather than taste: a derived `Debug` would render `api_key`,
    /// and a device's key in a log line or a crash report is a key that
    /// leaked. The device and the machine it paired with are what a bug report
    /// needs; `finish_non_exhaustive` says plainly that something is withheld.
    /// modelpipe's own `Paired` writes the identical impl for the identical
    /// reason.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MpPaired")
            .field("device", &self.device)
            .field("serving", &self.serving)
            .finish_non_exhaustive()
    }
}

/// Pair with the machine a pairing string names, and keep the pipe.
///
/// # Errors
///
/// [`MpPairError`], whose [`MpPairError::message`] is the sentence to show.
#[uniffi::export]
pub async fn mp_pair(
    pairing: String,
    label: Option<String>,
    options: MpConnectOptions,
    reach_within_ms: u64,
) -> Result<MpPaired, MpPairError> {
    in_runtime(async move {
        let pairing = PairingString::from_str(&pairing)?;
        // The same file a later dial to this machine will carry, so the
        // endpoint recorded beside the key as it is minted is the one that
        // then chats.
        let identity = identity_file::resolve(options.identity_dir.as_deref(), pairing.ticket());
        let reach_within = Duration::from_millis(reach_within_ms);
        let paired = match modelpipe::pair(
            &pairing,
            label.as_deref(),
            options.apply(identity.as_deref()),
            reach_within,
        )
        .await
        {
            Ok(paired) => paired,
            // `mp_connect`'s arm, one variant out. `PairError::Connect` is
            // raised before the code is presented -- modelpipe dials, waits
            // to reach the far machine and only then exchanges -- so a second
            // attempt cannot spend a one-time code that a first attempt
            // already spent. The variant that could, `Exchange`, is
            // deliberately not matched here.
            Err(error) => {
                let discarded = matches!(error, PairError::Connect(ConnectError::Identity { .. }))
                    && identity.as_deref().is_some_and(identity_file::discard);
                if !discarded {
                    return Err(error.into());
                }
                modelpipe::pair(
                    &pairing,
                    label.as_deref(),
                    options.apply(identity.as_deref()),
                    reach_within,
                )
                .await?
            }
        };
        Ok(MpPaired {
            pipe: Arc::new(MpPipe::new(paired.handle)),
            api_key: paired.api_key,
            device: paired.device,
            serving: paired.serving.to_string(),
        })
    })
    .await
}

/// A pairing string taken apart, as much of it as an app needs: who it names,
/// and whether it carries a code. The code itself never crosses.
#[derive(Clone, PartialEq, Eq, uniffi::Record)]
pub struct MpPairingString {
    /// The ticket, in modelpipe's canonical lower-case form, whatever case it
    /// was pasted or scanned in.
    pub ticket: String,
    /// Whether the string carries a code, which makes it a first pairing for
    /// [`mp_pair`] rather than a dial for [`crate::mp_connect`].
    pub has_code: bool,
}

impl std::fmt::Debug for MpPairingString {
    /// Hand-written for the reason `MpPaired`'s is: a ticket is what this
    /// crate redacts wherever an error renders one, and a derived `Debug`
    /// would print it in full. Whether there is a code is what a bug report
    /// needs; `finish_non_exhaustive` says something is withheld.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MpPairingString")
            .field("has_code", &self.has_code)
            .finish_non_exhaustive()
    }
}

/// Read a pairing string without pairing.
///
/// A form can accept or refuse a paste as it is typed, decide whether to ask
/// for a token, and keep the ticket, without a dial. The parse is modelpipe's
/// own, so what this accepts [`mp_pair`] accepts, and a ticket with a bad
/// checksum is refused here rather than at the dial. Synchronous: one C call.
///
/// # Errors
///
/// [`MpPairError::BadPairingString`], whose [`MpPairError::message`] is the
/// sentence to show, and which never contains the string.
#[uniffi::export]
pub fn mp_read_pairing(pairing: &str) -> Result<MpPairingString, MpPairError> {
    let pairing = PairingString::from_str(pairing)?;
    Ok(MpPairingString {
        ticket: pairing.ticket().to_string(),
        has_code: pairing.code().is_some(),
    })
}

#[cfg(test)]
#[path = "pair_tests.rs"]
mod pair_tests;

#[cfg(test)]
#[path = "read_pairing_tests.rs"]
mod read_pairing_tests;
