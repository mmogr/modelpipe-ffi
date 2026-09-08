//! Errors say something a person can act on, and never say the ticket.

use super::*;

/// modelpipe's normative ticket vector 1, the same 67-character string the
/// app's own tests use. Present here so a "bad ticket" test cannot pass by
/// accidentally rejecting a good one.
const GOOD_TICKET: &str = "pipeadlvvgabqkyqvn6vjp7nhslea45a5yls6pnkmizfv4bbu2hxa5iruaaauhlp2na";

/// The one property that is a security property rather than a nicety: an
/// error is the thing most likely to reach a log, a crash report or a
/// screenshot, and a ticket is a bearer credential. None of them may carry it.
#[test]
fn no_error_renders_the_ticket() {
    let errors = [
        MpError::from(TicketParseError::Malformed),
        MpError::from(TicketParseError::UnsupportedVersion(9)),
        MpError::BadTicket {
            reason: "unreadable".to_owned(),
        },
        MpError::Bind {
            reason: "address in use".to_owned(),
        },
        MpError::Endpoint {
            reason: "no route".to_owned(),
        },
        MpError::InvalidRelay {
            url: "not a url".to_owned(),
        },
        MpError::PeerUnreachable,
        MpError::Unknown {
            detail: "NewVariant".to_owned(),
        },
    ];

    for error in &errors {
        let rendered = format!("{error} {error:?}");
        assert!(
            !rendered.contains(GOOD_TICKET),
            "an error rendered a ticket: {rendered}"
        );
        assert!(
            !rendered.contains("pipead"),
            "an error rendered a ticket prefix: {rendered}"
        );
    }
}

/// Every variant says a whole sentence. An empty or fragmentary message is
/// what the app would show a person mid-failure.
#[test]
fn every_error_is_a_sentence() {
    let errors = [
        MpError::from(TicketParseError::Malformed),
        MpError::from(TicketParseError::UnsupportedVersion(9)),
        MpError::Bind {
            reason: "address in use".to_owned(),
        },
        MpError::Endpoint {
            reason: "no route".to_owned(),
        },
        MpError::InvalidRelay {
            url: "nope".to_owned(),
        },
        MpError::PeerUnreachable,
        MpError::Unknown {
            detail: "NewVariant".to_owned(),
        },
    ];

    for error in &errors {
        let rendered = error.to_string();
        assert!(!rendered.is_empty(), "{error:?} rendered nothing");
        assert!(
            rendered.ends_with('.') || rendered.ends_with(')'),
            "{error:?} is a fragment rather than a sentence: {rendered}"
        );
        assert!(
            rendered.chars().next().is_some_and(char::is_uppercase),
            "{error:?} does not start a sentence: {rendered}"
        );
    }
}

/// Retryability is the one bit the app acts on, so it is asserted per variant
/// rather than left to whoever edits the `match` next.
#[test]
fn only_the_failures_that_could_pass_next_time_are_retryable() {
    assert!(
        !MpError::BadTicket {
            reason: String::new()
        }
        .is_retryable(),
        "a bad ticket is bad however many times it is pasted"
    );
    assert!(!MpError::UnsupportedTicketVersion { version: 9 }.is_retryable());
    assert!(
        !MpError::Bind {
            reason: String::new()
        }
        .is_retryable(),
        "another process holds the port; dialling again finds it held"
    );
    assert!(
        !MpError::InvalidRelay { url: String::new() }.is_retryable(),
        "a malformed URL does not become well-formed"
    );

    assert!(
        MpError::PeerUnreachable.is_retryable(),
        "a machine that was asleep may not be next time"
    );
    assert!(
        MpError::Endpoint {
            reason: String::new()
        }
        .is_retryable()
    );
    assert!(
        MpError::Unknown {
            detail: String::new()
        }
        .is_retryable(),
        "an unknown transport failure is more often transient than permanent"
    );
}

/// A malformed ticket is distinguishable from a machine that did not answer.
/// They are the two failures a person confuses, and the sentences have to
/// point at different things to fix.
#[test]
fn a_bad_ticket_and_an_absent_machine_do_not_read_alike() {
    let bad = MpError::from(TicketParseError::Malformed).to_string();
    let away = MpError::PeerUnreachable.to_string();

    assert!(
        bad.contains("pairing string"),
        "the bad-ticket sentence names what to re-copy: {bad}"
    );
    assert!(
        away.contains("other machine"),
        "the unreachable sentence names the far side: {away}"
    );
    assert_ne!(bad, away);
}

/// An unsupported version names the version, because "update the app" is only
/// actionable if the person can tell it apart from a typo.
#[test]
fn an_unsupported_version_says_which_version() {
    let error = MpError::from(TicketParseError::UnsupportedVersion(7));
    assert_eq!(error, MpError::UnsupportedTicketVersion { version: 7 });
    assert!(error.to_string().contains('7'), "{error}");
}

/// An operating-system reason is inconsistently punctuated, and the sentence
/// has to read correctly whichever way it arrives — no missing full stop, and
/// no doubled one.
#[test]
fn an_os_reason_gets_exactly_one_full_stop() {
    for reason in [
        "address in use",
        "address in use.",
        "address in use.  ",
        "  address in use  ",
    ] {
        let rendered = MpError::Bind {
            reason: reason.to_owned(),
        }
        .to_string();

        assert!(
            rendered.ends_with("address in use."),
            "{reason:?} rendered as {rendered:?}"
        );
        assert!(
            !rendered.ends_with(".."),
            "{reason:?} doubled the full stop: {rendered:?}"
        );
    }
}
