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

/// Dial with the key at `path`, and when the dial refuses that key, throw it
/// away and dial once more.
///
/// Once, and only when [`make_way`] says the second dial could go
/// differently: not when the refusal was about the directory rather than the
/// key, where dialling again fails the same way. `refuses_the_key` is the
/// predicate, written at each call site against modelpipe's own error before
/// it is flattened, because it is the one part of this worth reading there.
///
/// The file is looked at before the first dial, because nothing serialises
/// two dials that resolve one path. Two refused over one file both arrive
/// here, and by the time the second does, the first may have thrown the file
/// away and linked a new key into place, which its live pipe runs on. The
/// second leaves that key where it is and dials on it too, which makes two
/// dials on one key: one device, as normal. Throwing it away instead would
/// leave the first dial's live pipe on a key the file no longer holds, so the
/// next launch would present a different one.
pub(crate) async fn dial_healing<T, E, Fut>(
    path: Option<&Path>,
    refuses_the_key: impl Fn(&E) -> bool,
    mut dial: impl FnMut() -> Fut,
) -> Result<T, E>
where
    Fut: Future<Output = Result<T, E>>,
{
    let before = path.map(|path| (path, Stamp::of(path)));
    match dial().await {
        Ok(value) => Ok(value),
        Err(error) => {
            let again = refuses_the_key(&error)
                && before.is_some_and(|(path, stamp)| make_way(path, stamp));
            if !again {
                return Err(error);
            }
            dial().await
        }
    }
}

/// Enough of a file to tell it from one put in its place: which file it is
/// where the platform says (device and inode), its length, and when it was
/// last written. Read from the path itself, as `remove_file` acts on it.
///
/// Not proof: a replacement with the same inode, length and timestamp passes
/// for the original, and the file can still change between this look and the
/// removal. It narrows that window to two system calls, which a lock across
/// the whole dial would close and is more machinery than the hazard has
/// earned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Stamp {
    len: u64,
    modified: Option<std::time::SystemTime>,
    #[cfg(unix)]
    file: (u64, u64),
}

impl Stamp {
    /// What is at `path` now, or `None` when nothing is.
    pub(crate) fn of(path: &Path) -> Option<Self> {
        let meta = std::fs::symlink_metadata(path).ok()?;
        Some(Self {
            len: meta.len(),
            modified: meta.modified().ok(),
            #[cfg(unix)]
            file: {
                use std::os::unix::fs::MetadataExt as _;
                (meta.dev(), meta.ino())
            },
        })
    }
}

/// After a refusal of the key at `path`, clear the way for one more dial, and
/// say whether that dial could go any differently.
///
/// When the path no longer holds the file `before` describes, another dial
/// has been at it since this one looked: thrown the file away, linked its own
/// key in, or both. Whatever is there now is that dial's, so it is left
/// alone, and `true` sends this one to dial again on what the path holds
/// then. When it is still the same file, the file is thrown away, and `true`
/// says it was.
///
/// `false` is the answer that stops a caller dialling again: there was no
/// file and still is none, or the file could not be removed. Either way the
/// refusal was about the directory or the path rather than about the file's
/// contents, and a second attempt fails the same way for as long as anyone
/// lets it; removing a path that is a directory fails too, which is the
/// answer wanted there.
///
/// Nothing is read first. What a key looks like is modelpipe's to judge, and
/// a second opinion about its format here is the duplication this seam exists
/// to refuse — this side only ever hears that the file could not be used.
pub(crate) fn make_way(path: &Path, before: Option<Stamp>) -> bool {
    if Stamp::of(path) != before {
        return true;
    }
    // `is_some` first: with no file before the dial, a key linked in between
    // the look above and the removal would otherwise be removed.
    before.is_some() && std::fs::remove_file(path).is_ok()
}

#[cfg(test)]
#[path = "identity_file_tests.rs"]
pub(crate) mod identity_file_tests;
