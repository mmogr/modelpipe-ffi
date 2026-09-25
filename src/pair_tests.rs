//! Pairing across the boundary, without a far machine: what is refused before
//! a dial, what a machine that is not there looks like, and what never reaches
//! a log.

use super::*;
use modelpipe::PairError;

use crate::identity_file::identity_file_tests::{GOOD_TICKET_KEY, Scratch};
use crate::pair_error::MpUnreached;
use crate::pipe::mp_connect;
use crate::runtime::runtime_tests::poll_on_this_thread;
use crate::status::MpPipeStatus;

/// modelpipe's normative ticket vector 1: well-formed, and names an endpoint
/// nothing is listening on.
pub(super) const GOOD_TICKET: &str =
    "pipeadlvvgabqkyqvn6vjp7nhslea45a5yls6pnkmizfv4bbu2hxa5iruaaauhlp2na";

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

/// The same pairing, polled from a thread with no tokio runtime, which is the
/// Swift caller's situation: the dial and the wait run on the library's own
/// runtime, and the answer is the same timeout.
#[test]
fn a_pairing_polled_from_a_thread_with_no_runtime_is_unreached() {
    let pairing = format!("{GOOD_TICKET}-123456");
    let error = poll_on_this_thread(mp_pair(pairing, None, offline_options(), 150))
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
        // Both arms: 404 has a sentence of its own.
        MpPairError::UnexpectedStatus { status: 404 },
        MpPairError::UnexpectedStatus { status: 503 },
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
    // The reason this variant exists. Folded into `Unknown` it answered
    // `true`, so an app offered a retry for a pairing that can never
    // succeed; modelpipe says `false` and this must not disagree.
    assert!(!MpPairError::UnexpectedStatus { status: 404 }.is_retryable());
    assert!(!MpPairError::UnexpectedStatus { status: 503 }.is_retryable());
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
    // Both ways, or the arm could be the constant `false` and pass.
    assert!(
        MpPairError::Dial {
            reason: String::new(),
            retryable: true
        }
        .is_retryable()
    );
}

/// What modelpipe hands over, and what crosses.
///
/// **The only test that drives `From<PairError>` for a status.** That matters
/// more than it sounds: of the three places this crate matches on either side
/// of this conversion, `is_retryable` and `Display` are exhaustive over
/// `MpPairError` and refuse to compile without a new arm — and this one
/// matches the *source*, `PairError`, which is `#[non_exhaustive]`, so its
/// wildcard is mandatory and swallows anything unhandled. That is how a status
/// added upstream arrived here as `Unknown`, rendering a Rust `Debug` string to
/// a person and answering `is_retryable() == true` where modelpipe answers
/// `false`. Other tests reach this conversion through `mp_pair`'s `?`, but only
/// for `NoCode` and `Unreached` — never for an answer carrying a status.
///
/// So this asserts the crossing rather than the shape. Without it the arm
/// above the wildcard can be deleted and every gate stays green.
#[test]
fn an_unexpected_status_crosses_as_a_number_rather_than_as_unknown() {
    for status in [404u16, 503] {
        assert_eq!(
            MpPairError::from(PairError::UnexpectedStatus { status }),
            MpPairError::UnexpectedStatus { status },
            "the status crosses as itself, not folded into `Unknown`"
        );
    }

    // The sentence a person is shown says which machine is at fault, and the
    // 404 — the one anyone has traced to a version — says so in its own
    // words. Neither renders a `Debug` string.
    let old = MpPairError::from(PairError::UnexpectedStatus { status: 404 }).message();
    assert!(old.contains("404") && old.contains("too old"), "{old}");
    let other = MpPairError::from(PairError::UnexpectedStatus { status: 503 }).message();
    assert!(
        other.contains("503") && !other.contains("too old"),
        "no status but the 404 has been traced to a version: {other}"
    );
    for message in [old, other] {
        assert!(!message.contains("UnexpectedStatus"), "{message}");
    }
}

/// The arms either side of the new one, which the wildcard would swallow
/// just as quietly.
///
/// `Unexpected` survives upstream — modelpipe answers "an empty key or
/// device" among others — so this is not the dead half of the split. And
/// `Refused` is here because losing *it* is the same bug in a worse place:
/// it would become `Unknown`, whose `is_retryable()` is true, so an app
/// would offer to retry a code the far machine has already rejected.
#[test]
fn the_arms_either_side_of_a_status_still_cross_as_themselves() {
    assert_eq!(
        MpPairError::from(PairError::Unexpected("an empty key or device")),
        MpPairError::Unexpected {
            detail: "an empty key or device".to_owned()
        }
    );
    assert_eq!(
        MpPairError::from(PairError::Refused),
        MpPairError::Refused,
        "a refused code must not become a retryable `Unknown`"
    );
}

/// A pairing keeps its key where a later dial to that machine will look for
/// it.
///
/// Nothing is listening, so the exchange never happens and the code is never
/// presented — but the key is minted while the pipe is being bound, which is
/// before any of that, so the file is there to assert on. It has to carry the
/// same name `mp_connect` gives it: the app introduces this device as it
/// redeems the code, and a pairing that wrote a different file would
/// introduce a device that never dials again. Nothing else in this crate
/// checks that the two halves agree.
#[tokio::test]
async fn pairing_keeps_its_key_in_the_directory_it_was_given() {
    let scratch = Scratch::new("pairing-keeps-its-key");

    let error = mp_pair(
        format!("{GOOD_TICKET}-123456"),
        Some("Phone".to_owned()),
        MpConnectOptions {
            identity_dir: Some(scratch.as_str().to_owned()),
            ..offline_options()
        },
        150,
    )
    .await
    .expect_err("nothing is listening at the vector ticket");

    assert!(matches!(error, MpPairError::Unreached { .. }), "{error:?}");
    assert_eq!(scratch.entries(), vec![GOOD_TICKET_KEY.to_owned()]);
}

/// A directory that is not there fails a pairing the way it fails a dial,
/// flattened into the one case `MpPairError` has for a dial that did not
/// start.
#[tokio::test]
async fn a_directory_that_is_not_there_makes_pairing_fail_as_a_dial() {
    let scratch = Scratch::new("pairing-directory-not-created");
    let absent = scratch.path().join("not-made-here");

    let error = mp_pair(
        format!("{GOOD_TICKET}-123456"),
        None,
        MpConnectOptions {
            identity_dir: Some(absent.to_str().expect("UTF-8").to_owned()),
            ..offline_options()
        },
        150,
    )
    .await
    .expect_err("there is nowhere to write the key");

    assert!(
        matches!(
            error,
            MpPairError::Dial {
                retryable: false,
                ..
            }
        ),
        "{error:?}"
    );
    assert!(
        error.message().contains(GOOD_TICKET_KEY),
        "the refusal does not name the file: {}",
        error.message()
    );
    assert!(!absent.exists(), "the directory was created after all");
}

/// A pairing heals its key too, which a pairing could not do before.
///
/// `MpPairError` folds every transport failure into one case carrying a
/// sentence, so nothing above this boundary can tell an unusable key from a
/// machine that is switched off without reading modelpipe's words back. The
/// retry is written underneath that, against modelpipe's own
/// `PairError::Connect(ConnectError::Identity)`, so the distinction is used
/// where it still exists and no new case crosses into Swift.
///
/// Reaching `Unreached` is the assertion: it means the dial got past the key
/// file and went looking for the far machine, which is as far as anything can
/// go with nothing listening. Before the retry this was `Dial`, and the
/// pairing stopped at the key.
#[tokio::test]
async fn a_key_pairing_cannot_use_is_replaced_and_pairing_goes_on() {
    let scratch = Scratch::new("pairing-heals-its-key");
    let path = scratch.path().join(GOOD_TICKET_KEY);
    std::fs::write(&path, b"not a key at all\n").expect("writable");

    let error = mp_pair(
        format!("{GOOD_TICKET}-123456"),
        None,
        MpConnectOptions {
            identity_dir: Some(scratch.as_str().to_owned()),
            ..offline_options()
        },
        150,
    )
    .await
    .expect_err("nothing is listening at the vector ticket");

    assert!(
        matches!(error, MpPairError::Unreached { .. }),
        "the pairing stopped at the key rather than at the far machine: {error:?}"
    );
    let now = std::fs::read(&path).expect("a key was minted in its place");
    assert_ne!(
        now, b"not a key at all\n",
        "the unusable key is still there"
    );
}
