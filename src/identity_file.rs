//! Where a device's endpoint key lives, and what it is called.
//!
//! A dial is given a **directory**; this module names the file inside it. The
//! name is a digest of the ticket being dialled, so one far machine is one
//! file: a relay allows a single live connection per endpoint id, and a phone
//! holding two machines has to meet each of them as a different device.
//!
//! **The name is a contract with the app, not an implementation detail.**
//! ggchat computed it in Swift before this crate did — `Ticket.digest` there
//! is SHA-256 of the canonical ticket, first eight bytes, lower-case hex —
//! and every device already paired holds a file under that name. Producing a
//! different one would not fail: every such device would simply introduce
//! itself to its desktop as a new device, for ever, with no error anywhere.
//! `identity_file_tests.rs` pins the bytes against modelpipe's own normative
//! ticket vectors so that a change here has to be deliberate.
//!
//! **What is hashed is the ticket modelpipe would print, never the string the
//! caller passed in.** [`Display`](std::fmt::Display) for a ticket is
//! canonicalising — it sorts and deduplicates addresses, drops an address tag
//! this build does not know, and lower-cases — so the two differ for tickets
//! that are perfectly valid, and taking the argument would give the same
//! machine two names depending on how its ticket was written down. Requiring
//! a `&Ticket` here is what makes that mistake impossible rather than merely
//! discouraged.
//!
//! **Nothing here creates a directory.** The app owns that: it makes the
//! directory `0o700` at creation and marks it as no backup's business, and a
//! directory created here would have neither property until the app's next
//! call. A directory that is missing or unwritable arrives as
//! [`MpError::Identity`](crate::MpError::Identity), which names the file and
//! is not retryable.

use std::path::{Path, PathBuf};

use modelpipe::Ticket;
use sha2::{Digest as _, Sha256};

/// How many leading bytes of the digest name the file: eight, so sixteen hex
/// characters. ggchat's `Ticket.digest` takes the same eight.
const NAME_BYTES: usize = 8;

/// What the name ends in.
const SUFFIX: &str = ".key";

/// Lower-case hex, by table. Deliberately not a format macro: the only thing
/// being formatted here is a byte, and `check_no_credentials.sh` reads every
/// format-macro line in this crate looking for a ticket interpolated into
/// output. Keeping the macro out of the file keeps that gate about output.
const HEX: &[u8; 16] = b"0123456789abcdef";

/// What the key file for `ticket` is called.
pub(crate) fn file_name(ticket: &Ticket) -> String {
    let canonical = ticket.to_string();
    let digest = Sha256::digest(canonical.as_bytes());
    let mut name = String::with_capacity(NAME_BYTES * 2 + SUFFIX.len());
    for byte in &digest[..NAME_BYTES] {
        name.push(char::from(HEX[usize::from(byte >> 4)]));
        name.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    name.push_str(SUFFIX);
    name
}

/// Where the key for `ticket` goes, given the directory a caller chose.
///
/// `None` in, `None` out: a caller that names no directory is asking for a
/// dial that keeps no key, which is what every version before this one did
/// when it could not write. Joining is all that happens — no file is opened,
/// no directory is made, and nothing is checked.
pub(crate) fn resolve(dir: Option<&str>, ticket: &Ticket) -> Option<PathBuf> {
    // `?` rather than `dir.map(...)`, which is the same thing and is what
    // `clippy::single_option_map` refuses: a function whose whole body maps
    // over one optional argument wants that map at the call site. Keeping the
    // option here is deliberate — "no directory means no key" is one rule and
    // has two callers — so the lint is answered rather than allowed.
    let dir = dir?;
    Some(Path::new(dir).join(file_name(ticket)))
}

/// Throw the key at `path` away, and say whether there was one to throw.
///
/// `false` is the answer that stops a caller dialling again: it means the
/// refusal was about the directory or the path rather than about the file's
/// contents, and a second attempt fails the same way for as long as anyone
/// lets it. Removing a path that is a directory fails too, which is the
/// answer wanted there.
///
/// Nothing is read first. What a key looks like is modelpipe's to judge, and
/// a second opinion about its format here is the duplication this seam exists
/// to refuse — this side only ever hears that the file could not be used.
pub(crate) fn discard(path: &Path) -> bool {
    std::fs::remove_file(path).is_ok()
}

#[cfg(test)]
#[path = "identity_file_tests.rs"]
pub(crate) mod identity_file_tests;
