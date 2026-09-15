//! Pairing, across the boundary: a pairing string in, a key and a live pipe
//! out.

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use modelpipe::PairingString;

use crate::options::MpConnectOptions;
use crate::pair_error::MpPairError;
use crate::pipe::MpPipe;

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
#[uniffi::export(async_runtime = "tokio")]
pub async fn mp_pair(
    pairing: String,
    label: Option<String>,
    options: MpConnectOptions,
    reach_within_ms: u64,
) -> Result<MpPaired, MpPairError> {
    let pairing = PairingString::from_str(&pairing)?;
    let paired = modelpipe::pair(
        &pairing,
        label.as_deref(),
        options.apply(),
        Duration::from_millis(reach_within_ms),
    )
    .await?;
    Ok(MpPaired {
        pipe: Arc::new(MpPipe::new(paired.handle)),
        api_key: paired.api_key,
        device: paired.device,
        serving: paired.serving.to_string(),
    })
}

#[cfg(test)]
#[path = "pair_tests.rs"]
mod pair_tests;
