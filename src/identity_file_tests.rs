use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use super::*;

/// modelpipe's normative ticket vector 1: well-formed, and names an endpoint
/// nothing is listening on.
pub(crate) const GOOD_TICKET: &str =
    "pipeadlvvgabqkyqvn6vjp7nhslea45a5yls6pnkmizfv4bbu2hxa5iruaaauhlp2na";

/// What [`GOOD_TICKET`]'s key file is called, spelled out.
///
/// The one literal in this crate that another repository also computes:
/// ggchat's `Ticket.digest` produced this name in Swift before this crate
/// named anything, and every device paired on modelpipe-ffi 0.3.x holds a
/// file under it. Imported by `pipe_tests` and `pair_tests` rather than
/// repeated there, so the rule has one spelling to change and a reviewer can
/// see that the dial and the pairing are asserting the same thing.
pub(crate) const GOOD_TICKET_KEY: &str = "0382e9033d890983.key";

/// A directory of this test's own, removed when it goes out of scope.
///
/// Written here rather than taken from `tempfile`: this crate's manifest has
/// four dependencies and an argument about why, and a directory under
/// `temp_dir` with the process id in its name is the whole of what these
/// tests need.
pub(crate) struct Scratch(PathBuf);

impl Scratch {
    pub(crate) fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("modelpipe-ffi-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("a scratch directory under the temp dir");
        Self(path)
    }

    /// The directory itself.
    pub(crate) fn path(&self) -> &Path {
        &self.0
    }

    /// The directory as the `identityDir` string a caller would pass.
    pub(crate) fn as_str(&self) -> &str {
        self.0
            .to_str()
            .expect("the temp dir is UTF-8 on both CI hosts")
    }

    /// Everything in it, sorted — the exact set, so a leftover temporary is
    /// a failure rather than something an `exists` check steps over.
    pub(crate) fn entries(&self) -> Vec<String> {
        let mut found: Vec<String> = fs::read_dir(&self.0)
            .expect("the scratch directory is there")
            .map(|entry| {
                entry
                    .expect("a readable directory entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        found.sort();
        found
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn ticket(s: &str) -> Ticket {
    Ticket::from_str(s).expect("a ticket modelpipe's own vectors call well-formed")
}

/// The name is SHA-256 of the canonical ticket, first eight bytes, lower-case
/// hex — the rule ggchat wrote in Swift, spelled out here as bytes.
///
/// Each vector asserts its own canonicality first. Three of these strings are
/// already what modelpipe prints, and a test that assumed so without checking
/// would go on passing if the encoder ever stopped agreeing with the file
/// the vectors live in.
#[test]
fn the_name_is_the_canonical_tickets_digest() {
    for (vector, expected) in [
        (GOOD_TICKET, GOOD_TICKET_KEY),
        (
            "pipeadlvvgabqkyqvn6vjp7nhslea45a5yls6pnkmizfv4bbu2hxa5iruaicaajcaainxaaaaaaaaaaaaaaaaaaach4qaabstehw",
            "539f2a323bfd6b3c.key",
        ),
        (
            "pipeadlvvgabqkyqvn6vjp7nhslea45a5yls6pnkmizfv4bbu2hxa5iruaqaaangq5duobztulzpojswyylzfzsxqylnobwgkltdn5ws6aiaa3akqaihcfiqbrp5xr4q",
            "6f3ce5a887c72cd2.key",
        ),
    ] {
        let parsed = ticket(vector);
        assert_eq!(
            parsed.to_string(),
            vector,
            "this vector is not what modelpipe prints, so the digest below is of something else"
        );
        assert_eq!(file_name(&parsed), expected, "vector {vector}");
    }
}

/// A ticket that arrives spelled one way and prints another still names one
/// file — the one its printed form names.
///
/// modelpipe's `accept/an-unknown-address-tag-skipped` vector carries an
/// address tag this build does not know. The parse drops it, so the string
/// that comes back is shorter and carries a different checksum, and hashing
/// what arrived instead would give a different file for the same machine.
/// This is the case case-folding cannot explain, and the only one in the
/// normative set where the two answers are both well-formed.
#[test]
fn a_ticket_whose_spelling_changes_on_the_way_in_names_one_file() {
    let arrived = "pipeadlvvgabqkyqvn6vjp7nhslea45a5yls6pnkmizfv4bbu2hxa5iruaqbaadmbkaba4ivc7yaatpk3pxpaacgvrnq";
    let parsed = ticket(arrived);

    assert_ne!(
        parsed.to_string(),
        arrived,
        "this vector no longer changes on the way in, so it proves nothing here"
    );
    assert_eq!(file_name(&parsed), "7a747de33b931e6b.key");
    assert_ne!(
        file_name(&parsed),
        "b95dddb1efb1e7ca.key",
        "the name was taken from the string that arrived rather than the one modelpipe prints"
    );
}

/// The same machine read off a QR code is the same machine.
///
/// A ticket parses case-insensitively and prints lower-case, so an upper-case
/// spelling has to reach the same file. The second assertion is what the
/// first cannot say on its own: the name of the shouted string itself.
#[test]
fn a_shouted_ticket_names_the_same_file() {
    let shouted = ticket(&GOOD_TICKET.to_uppercase());

    assert_eq!(file_name(&shouted), GOOD_TICKET_KEY);
    assert_ne!(
        file_name(&shouted),
        "20aa414b7f8b627b.key",
        "the name was taken from the argument rather than the canonical ticket"
    );
}

/// No directory, no key — a dial that keeps nothing, which is what every
/// version before this one did when the app could not write.
#[test]
fn no_directory_means_no_identity() {
    assert!(resolve(None, &ticket(GOOD_TICKET)).is_none());
}

/// The directory is joined and nothing else happens to it.
#[test]
fn the_directory_is_joined_and_nothing_else() {
    let resolved =
        resolve(Some("/somewhere/keys"), &ticket(GOOD_TICKET)).expect("a directory was given");

    assert_eq!(resolved, Path::new("/somewhere/keys").join(GOOD_TICKET_KEY));
}

/// Naming a file makes nothing.
///
/// The app creates the directory, `0o700` at creation and out of the backup,
/// and one made here would have neither property. Resolving against a
/// directory that is not there must leave it not there.
#[test]
fn resolving_creates_nothing() {
    let scratch = Scratch::new("resolve-creates-nothing");
    let absent = scratch.path().join("not-made-here");

    let resolved = resolve(Some(absent.to_str().expect("UTF-8")), &ticket(GOOD_TICKET))
        .expect("a directory was given");

    assert_eq!(resolved, absent.join(GOOD_TICKET_KEY));
    assert!(!absent.exists(), "resolving made the directory");
    assert!(!resolved.exists(), "resolving made the file");
}

/// Throwing a key away says whether there was one, which is the answer a
/// caller needs to decide whether dialling again could go any differently.
#[test]
fn discard_removes_the_key_and_says_whether_there_was_one() {
    let scratch = Scratch::new("discard-says-so");
    let path = scratch.path().join(GOOD_TICKET_KEY);
    fs::write(&path, b"anything at all").expect("the scratch directory is writable");

    assert!(discard(&path), "a file that was there reported as absent");
    assert!(!path.exists());
    assert!(!discard(&path), "there was nothing left to throw away");
}

/// What a key looks like is modelpipe's to judge. This side is told only that
/// the file could not be used, so it removes whatever is at the path without
/// forming a second opinion about the format — and without reading a key it
/// has no reason to hold.
#[test]
fn discard_does_not_judge_what_is_in_the_file() {
    let scratch = Scratch::new("discard-does-not-judge");
    let path = scratch.path().join(GOOD_TICKET_KEY);
    fs::write(
        &path,
        b"aznmoyaqvvqgfrtdjxnwcejbjrp72o4mrywtnjxqqxwpxyrymxaa\n",
    )
    .expect("the scratch directory is writable");

    assert!(discard(&path));
    assert!(!path.exists());
}

/// A path that is not a file is not thrown away, so the caller is told there
/// was nothing to throw and does not dial again.
#[test]
fn a_path_that_is_a_directory_is_not_discarded() {
    let scratch = Scratch::new("discard-a-directory");
    let path = scratch.path().join(GOOD_TICKET_KEY);
    fs::create_dir(&path).expect("the scratch directory is writable");

    assert!(
        !discard(&path),
        "a directory was reported as a key thrown away"
    );
    assert!(path.is_dir(), "the directory was removed");
}
