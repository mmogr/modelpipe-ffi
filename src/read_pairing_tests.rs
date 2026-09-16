//! Reading a pairing string without pairing: what a form learns, what it does
//! not, and what never reaches a log.

use super::pair_tests::GOOD_TICKET;
use super::*;

/// A ticket alone is a dial, not a pairing: no code, and the ticket as is.
#[test]
fn a_ticket_alone_reads_as_a_ticket_with_no_code() {
    let read = mp_read_pairing(GOOD_TICKET).expect("a ticket is a pairing string");
    assert_eq!(read.ticket, GOOD_TICKET);
    assert!(!read.has_code);
}

/// A ticket with a code is a first pairing, and the ticket comes back without
/// the code: the code is for `mp_pair`, which takes the whole string.
#[test]
fn a_ticket_with_a_code_reads_as_a_first_pairing_and_keeps_the_code_out() {
    let read = mp_read_pairing(&format!("{GOOD_TICKET}-123456")).expect("a pairing string");
    assert_eq!(read.ticket, GOOD_TICKET);
    assert!(read.has_code);
    assert!(!read.ticket.contains("123456"));
}

/// A QR code carries the string upper-cased and a paste may carry space
/// around it; the ticket comes back in the one canonical form, so a digest of
/// it is the same digest whichever way it arrived.
#[test]
fn the_ticket_comes_back_canonical_whatever_case_or_space_it_arrived_in() {
    let shouted = format!("  {}-123456\n", GOOD_TICKET.to_ascii_uppercase());
    let read = mp_read_pairing(&shouted).expect("a scan parses");
    assert_eq!(read.ticket, GOOD_TICKET);
    assert!(read.has_code);
}

/// The refusals are modelpipe's, one per part, and each is the sentence
/// `mp_pair` would have given for the same string.
#[test]
fn a_string_that_is_not_one_is_refused_with_modelpipes_reason() {
    for (pasted, wrong_part) in [
        ("", "a pairing string is a ticket"),
        ("   ", "a pairing string is a ticket"),
        ("nope-123456", "the part before the code is not a ticket"),
        (
            &*format!("{GOOD_TICKET}-12345"),
            "the part after the last '-' should be the six-digit code",
        ),
        ("nope", "the part before the code is not a ticket"),
    ] {
        let error = mp_read_pairing(pasted).expect_err(pasted);
        assert!(
            matches!(error, MpPairError::BadPairingString { .. }),
            "{pasted:?}: {error:?}"
        );
        assert!(!error.is_retryable());
        let message = error.message();
        assert!(message.contains(wrong_part), "{pasted:?}: {message}");
        assert!(
            message.starts_with("That pairing string could not be read ("),
            "{message}"
        );
        assert!(message.ends_with(")."), "{message}");
    }
}

/// Neither the refusal nor the value's `Debug` carries the ticket or the code.
#[test]
fn neither_a_refusal_nor_a_debug_rendering_shows_the_ticket_or_the_code() {
    let error = mp_read_pairing(&format!("{GOOD_TICKET}-12345")).expect_err("a five-digit code");
    for rendering in [error.message(), format!("{error:?}")] {
        assert!(!rendering.contains(GOOD_TICKET), "{rendering}");
        assert!(!rendering.contains("12345"), "{rendering}");
    }
    let read = mp_read_pairing(&format!("{GOOD_TICKET}-123456")).expect("a pairing string");
    let rendered = format!("{read:?}");
    assert!(!rendered.contains(GOOD_TICKET), "{rendered}");
    assert!(!rendered.contains("123456"), "{rendered}");
    assert!(rendered.contains("has_code: true"), "{rendered}");
}
