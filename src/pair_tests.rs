//! Pairing across the boundary, without a far machine: what is refused before
//! a dial, what a machine that is not there looks like, and what never reaches
//! a log.

use super::*;
use crate::pair_error::MpUnreached;
use crate::pipe::mp_connect;
use crate::status::MpPipeStatus;

/// modelpipe's normative ticket vector 1: well-formed, and names an endpoint
/// nothing is listening on.
const GOOD_TICKET: &str = "pipeadlvvgabqkyqvn6vjp7nhslea45a5yls6pnkmizfv4bbu2hxa5iruaaauhlp2na";

fn offline_options() -> MpConnectOptions {
    MpConnectOptions {
        discovery: false,
        port_mapping: false,
        ..MpConnectOptions::default()
    }
}

/// A ticket alone has no code to redeem, and is refused before anything is
/// dialled.
#[tokio::test]
async fn a_pairing_string_with_no_code_is_refused_without_dialling() {
    let error = mp_pair(GOOD_TICKET.to_owned(), None, offline_options(), 100)
        .await
        .expect_err("a ticket alone has no code");
    assert!(matches!(error, MpPairError::NoCode), "{error:?}");
    assert!(!error.is_retryable());
}

#[tokio::test]
async fn a_pairing_string_that_is_not_one_is_refused() {
    let error = mp_pair("nope-123456".to_owned(), None, offline_options(), 100)
        .await
        .expect_err("not a pairing string");
    assert!(
        matches!(error, MpPairError::BadPairingString { .. }),
        "{error:?}"
    );
    assert!(!error.is_retryable());
}

/// Nothing listens at the vector ticket, so the code is never presented: the
/// wait to reach the far machine runs out first, and says so.
#[tokio::test]
async fn pairing_with_a_machine_that_is_not_there_times_out_and_says_so() {
    let pairing = format!("{GOOD_TICKET}-123456");
    let error = mp_pair(pairing, Some("Phone".to_owned()), offline_options(), 150)
        .await
        .expect_err("nothing is listening");
    assert!(
        matches!(
            error,
            MpPairError::Unreached {
                why: MpUnreached::TimedOut { within_ms: 150 }
            }
        ),
        "{error:?}"
    );
    assert!(error.is_retryable());
    let message = error.message();
    assert!(
        message.ends_with('.') || message.ends_with(')'),
        "{message}"
    );
}

#[tokio::test]
async fn waiting_on_a_machine_that_is_not_there_times_out() {
    let pipe = mp_connect(GOOD_TICKET.to_owned(), offline_options())
        .await
        .expect("binds");
    let error = pipe.wait_reachable(150).await.expect_err("nothing answers");
    assert!(
        matches!(error, MpUnreached::TimedOut { within_ms: 150 }),
        "{error:?}"
    );
    assert!(error.is_retryable());
    assert_eq!(
        pipe.status(),
        MpPipeStatus::Idle,
        "a timeout tears nothing down"
    );
}

#[tokio::test]
async fn a_peer_id_is_sixty_four_hex_characters() {
    let pipe = mp_connect(GOOD_TICKET.to_owned(), offline_options())
        .await
        .expect("binds");
    let id = pipe.peer_id();
    assert_eq!(id.len(), 64, "{id}");
    assert!(id.bytes().all(|b| b.is_ascii_hexdigit()), "{id}");
}

/// The values most likely to reach a log are the errors. None of them carries
/// the string that was pasted, the code inside it, or a key.
#[tokio::test]
async fn no_pair_error_renders_the_code_or_the_key() {
    let bad = mp_pair("nope-424242".to_owned(), None, offline_options(), 100)
        .await
        .expect_err("not a pairing string");
    let away = mp_pair(
        format!("{GOOD_TICKET}-424242"),
        None,
        offline_options(),
        100,
    )
    .await
    .expect_err("nothing is listening");
    for error in [bad, away, MpPairError::NoCode, MpPairError::Refused] {
        for rendering in [error.message(), format!("{error:?}")] {
            assert!(!rendering.contains("424242"), "{rendering}");
            assert!(!rendering.contains("nope"), "{rendering}");
            assert!(!rendering.contains(GOOD_TICKET), "{rendering}");
        }
    }
}

/// `MpPaired` is the one value that carries a key. Its `Debug` withholds it.
#[tokio::test]
async fn the_debug_of_a_paired_device_never_shows_its_key() {
    let pipe = mp_connect(GOOD_TICKET.to_owned(), offline_options())
        .await
        .expect("binds");
    let paired = MpPaired {
        pipe,
        api_key: "secret-key-0123".to_owned(),
        device: "dev-test".to_owned(),
        serving: "0".repeat(64),
    };
    let rendered = format!("{paired:?}");
    assert!(!rendered.contains("secret-key-0123"), "{rendered}");
    assert!(rendered.contains("dev-test"), "{rendered}");
    assert!(rendered.contains(&"0".repeat(64)), "{rendered}");
}

/// The same contract `MpError` keeps: every message is a whole sentence.
#[test]
fn every_pair_error_is_a_sentence() {
    let errors = [
        MpPairError::NoCode,
        MpPairError::BadPairingString {
            reason: "That pairing string could not be read (too short).".to_owned(),
        },
        MpPairError::Dial {
            reason: "The connection could not start (os error 1).".to_owned(),
            retryable: true,
        },
        MpPairError::Unreached {
            why: MpUnreached::TimedOut { within_ms: 1 },
        },
        MpPairError::Unreached {
            why: MpUnreached::Closed { reason: None },
        },
        MpPairError::Refused,
        MpPairError::Exchange {
            reason: "connection reset".to_owned(),
        },
        MpPairError::Unexpected {
            detail: "no device id".to_owned(),
        },
        MpPairError::Unknown {
            detail: "Something".to_owned(),
        },
    ];
    for error in errors {
        let message = error.message();
        assert!(
            message.ends_with('.') || message.ends_with(')'),
            "{error:?}: {message}"
        );
        assert!(!message.contains("MpPairError"), "{message}");
    }
}

/// Only the failures that could pass next time are retryable: a code the far
/// machine refused, a string that is not one, and a ticket with no code are
/// not, however many times they are tried.
#[test]
fn only_the_failures_that_could_pass_next_time_are_retryable() {
    assert!(!MpPairError::NoCode.is_retryable());
    assert!(
        !MpPairError::BadPairingString {
            reason: String::new()
        }
        .is_retryable()
    );
    assert!(!MpPairError::Refused.is_retryable());
    assert!(
        !MpPairError::Unexpected {
            detail: String::new()
        }
        .is_retryable()
    );
    assert!(
        !MpPairError::Unreached {
            why: MpUnreached::Closed { reason: None }
        }
        .is_retryable()
    );
    assert!(
        MpPairError::Unreached {
            why: MpUnreached::TimedOut { within_ms: 1 }
        }
        .is_retryable()
    );
    assert!(
        MpPairError::Exchange {
            reason: String::new()
        }
        .is_retryable()
    );
    assert!(
        MpPairError::Unknown {
            detail: String::new()
        }
        .is_retryable()
    );
    assert!(
        !MpPairError::Dial {
            reason: String::new(),
            retryable: false
        }
        .is_retryable()
    );
}
