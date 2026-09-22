//! The dial refuses what it should refuse, and a real pipe behaves the way
//! the app's seam says it must.
//!
//! These need no far machine. `connect` returns once the local port is bound,
//! so every property below — the base URL's shape, the status starting at
//! `Idle`, shutdown being idempotent and terminal — is observable against a
//! ticket that names a machine which does not exist. That is the contract, not
//! a shortcut: a pipe to an absent machine is a perfectly good pipe.

use super::*;

use crate::identity_file::identity_file_tests::{GOOD_TICKET_KEY, Scratch};

/// modelpipe's normative ticket vector 1: well-formed, and names an endpoint
/// nothing is listening on.
const GOOD_TICKET: &str = "pipeadlvvgabqkyqvn6vjp7nhslea45a5yls6pnkmizfv4bbu2hxa5iruaaauhlp2na";

/// modelpipe's `accept/one-ipv6` vector: a second well-formed ticket, so a
/// test can tell one machine's key from another's.
const OTHER_TICKET: &str = "pipeadlvvgabqkyqvn6vjp7nhslea45a5yls6pnkmizfv4bbu2hxa5iruaicaajcaainxaaaaaaaaaaaaaaaaaaach4qaabstehw";

/// [`offline_options`] with a directory to keep the key in.
fn keeping_a_key_in(dir: &Scratch) -> MpConnectOptions {
    MpConnectOptions {
        identity_dir: Some(dir.as_str().to_owned()),
        ..offline_options()
    }
}

/// Dial with discovery and port mapping off, so a test never touches n0's
/// discovery service or asks the router for anything.
fn offline_options() -> MpConnectOptions {
    MpConnectOptions {
        discovery: false,
        port_mapping: false,
        ..MpConnectOptions::default()
    }
}

/// A ticket that is not one is refused before anything is bound or dialled.
/// The distinction matters: this failure should cost nothing and should not
/// look like a machine that is away.
#[tokio::test]
async fn a_ticket_that_is_not_one_is_refused_without_dialling() {
    let error = mp_connect("nope".to_owned(), offline_options())
        .await
        .expect_err("`nope` is not a ticket");

    assert!(
        matches!(error, MpError::BadTicket { .. }),
        "got {error:?}, wanted a bad-ticket refusal"
    );
    assert!(!error.is_retryable());
}

/// An empty string is the other thing a paste produces, and takes the same
/// path rather than panicking somewhere inside the parser.
#[tokio::test]
async fn an_empty_ticket_is_refused() {
    let error = mp_connect(String::new(), offline_options())
        .await
        .expect_err("an empty string is not a ticket");
    assert!(matches!(error, MpError::BadTicket { .. }), "{error:?}");
}

/// A ticket with the right shape but a corrupted body fails the checksum
/// rather than being dialled at whatever the bad bytes decode to.
#[tokio::test]
async fn a_corrupted_ticket_is_refused() {
    let mut corrupted = GOOD_TICKET.to_owned();
    corrupted.replace_range(10..11, "z");

    let error = mp_connect(corrupted, offline_options())
        .await
        .expect_err("a flipped character breaks the checksum");
    assert!(matches!(error, MpError::BadTicket { .. }), "{error:?}");
}

/// The seam's behaviours 2 and 3, together: `connect` returns with the
/// listener up, the URL is loopback and ends in `/v1`, and the pipe starts at
/// `Idle` because the far machine has not answered and never will.
#[tokio::test]
async fn a_dial_returns_a_bound_loopback_url_and_starts_idle() {
    let pipe = mp_connect(GOOD_TICKET.to_owned(), offline_options())
        .await
        .expect("the listener binds even though nothing is out there");

    let url = pipe.base_url();
    assert!(
        url.starts_with("http://127.0.0.1:"),
        "the seam requires a loopback base URL: {url}"
    );
    assert!(url.ends_with("/v1"), "the app appends nothing: {url}");
    assert_eq!(
        url,
        format!("http://127.0.0.1:{}/v1", pipe.port()),
        "the port accessor and the URL disagree"
    );

    assert_eq!(
        pipe.status(),
        MpPipeStatus::Idle,
        "a pipe with no answer yet is Idle, not Closed"
    );
    assert!(
        pipe.close_reason().is_none(),
        "an open pipe has no close reason"
    );

    pipe.shutdown().await;
}

/// Two pipes at once get two ports. The app can hold several providers, and a
/// collision would route one machine's traffic to another.
#[tokio::test]
async fn two_pipes_get_different_ports() {
    let first = mp_connect(GOOD_TICKET.to_owned(), offline_options())
        .await
        .expect("first binds");
    let second = mp_connect(GOOD_TICKET.to_owned(), offline_options())
        .await
        .expect("second binds");

    assert_ne!(
        first.port(),
        second.port(),
        "two live pipes shared a loopback port"
    );

    first.shutdown().await;
    second.shutdown().await;
}

/// Seam behaviour 6: `shutdown` is idempotent, and afterwards the pipe is
/// closed and says why.
#[tokio::test]
async fn shutdown_is_idempotent_and_terminal() {
    let pipe = mp_connect(GOOD_TICKET.to_owned(), offline_options())
        .await
        .expect("binds");

    pipe.shutdown().await;
    assert_eq!(pipe.status(), MpPipeStatus::Closed);
    assert_eq!(
        pipe.close_reason(),
        Some(MpCloseReason::Shutdown),
        "a shutdown this side asked for reports itself as one"
    );

    // Twice is fine, and the second returns rather than hanging.
    pipe.shutdown().await;
    assert_eq!(pipe.status(), MpPipeStatus::Closed);
}

/// Seam behaviour 6, the half a mock cannot assert: after shutdown the port
/// **refuses** rather than accepting and hanging. A caller that retries
/// against a stale base URL must get a connection error immediately.
#[tokio::test]
async fn after_shutdown_the_port_refuses_rather_than_hanging() {
    let pipe = mp_connect(GOOD_TICKET.to_owned(), offline_options())
        .await
        .expect("binds");
    let addr = format!("127.0.0.1:{}", pipe.port());
    pipe.shutdown().await;

    // Bounded, because the failure this guards against is a hang: an
    // unbounded connect that never returns would hang the suite instead of
    // failing it.
    let attempt = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::net::TcpStream::connect(&addr),
    )
    .await;

    match attempt {
        // The one correct outcome: connect returned, and it returned an error.
        Ok(Err(_)) => {}
        Ok(Ok(_)) => panic!("{addr} still accepts connections after shutdown"),
        Err(elapsed) => {
            panic!("{addr} hung instead of refusing after shutdown ({elapsed})")
        }
    }
}

/// The status sequence *ends*. Handed the terminal value it already holds,
/// `status_changed_since` returns `None` rather than answering `Closed`
/// forever — which is what stops a generated `next()` loop spinning a core on
/// a pipe that has been over for an hour.
#[tokio::test]
async fn the_status_sequence_ends_after_a_close() {
    let pipe = mp_connect(GOOD_TICKET.to_owned(), offline_options())
        .await
        .expect("binds");
    pipe.shutdown().await;

    let next = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        pipe.status_changed_since(MpPipeStatus::Closed),
    )
    .await
    .expect("a snapshot that already says Closed ends the sequence at once");

    assert_eq!(
        next, None,
        "the sequence ended rather than repeating Closed"
    );
}

/// A snapshot that is *behind* still gets the close delivered, exactly once.
/// Nothing is lost by watching this way, which is the property that makes the
/// caller-supplied snapshot safe.
#[tokio::test]
async fn a_stale_snapshot_is_still_told_about_the_close() {
    let pipe = mp_connect(GOOD_TICKET.to_owned(), offline_options())
        .await
        .expect("binds");
    pipe.shutdown().await;

    let next = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        pipe.status_changed_since(MpPipeStatus::Idle),
    )
    .await
    .expect("a stale snapshot is answered rather than waited on");

    assert_eq!(next, Some(MpPipeStatus::Closed));
}

/// The resume hook is safe on a closed pipe. The app calls it from every
/// foreground without checking, which is what its own documentation asks for,
/// so a pipe that died while backgrounded must not make it panic or hang.
#[tokio::test]
async fn the_resume_hook_is_harmless_on_a_closed_pipe() {
    let pipe = mp_connect(GOOD_TICKET.to_owned(), offline_options())
        .await
        .expect("binds");
    pipe.shutdown().await;

    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        pipe.notify_network_change(),
    )
    .await
    .expect("notifying a closed pipe returns rather than hanging");
}

/// Metrics read as three zeroes on a pipe that never reached a relay, rather
/// than failing or reporting something invented.
#[tokio::test]
async fn a_pipe_that_reached_nothing_reports_zeroes() {
    let pipe = mp_connect(GOOD_TICKET.to_owned(), offline_options())
        .await
        .expect("binds");

    assert_eq!(pipe.network_metrics(), MpNetworkMetrics::default());
    pipe.shutdown().await;
}

/// Dropping the last reference outside the runtime returns at once: the
/// courtesy close is handed to the runtime, not run on the dropping thread.
/// The bound is wide on purpose; a `spawn` costs microseconds, and a loaded
/// machine must not fail this. (A close that ran on this thread instead is
/// caught by the test below, not by this timing.)
#[test]
fn dropping_a_pipe_outside_a_runtime_does_not_block_the_dropping_thread() {
    let pipe = crate::runtime::runtime()
        .block_on(mp_connect(GOOD_TICKET.to_owned(), offline_options()))
        .expect("binds");
    let handle = Arc::clone(&pipe.handle);

    let started = std::time::Instant::now();
    drop(pipe);
    let returned_after = started.elapsed();
    assert!(
        returned_after < std::time::Duration::from_secs(1),
        "the drop took {returned_after:?}"
    );

    // The timeout is built inside the runtime: a `Sleep` needs one to exist.
    let closed = crate::runtime::runtime().block_on(async {
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            handle.status_changed_since(modelpipe::PipeStatus::Idle),
        )
        .await
    });
    let closed_after = started.elapsed();
    assert_eq!(
        closed.expect("the courtesy close runs"),
        Some(modelpipe::PipeStatus::Closed)
    );
    eprintln!("drop returned after {returned_after:?}; the close finished after {closed_after:?}");
}

/// Dropped from inside the runtime, the pipe is still closed. The guard that
/// used to skip the close there is gone, and a `block_on` there would panic,
/// so this is the test a restored `block_on` fails.
#[tokio::test]
async fn dropping_a_pipe_from_inside_the_runtime_still_closes_it() {
    let pipe = crate::runtime::runtime()
        .spawn(mp_connect(GOOD_TICKET.to_owned(), offline_options()))
        .await
        .expect("the dial task finished")
        .expect("binds");
    let handle = Arc::clone(&pipe.handle);

    crate::runtime::runtime()
        .spawn(async move { drop(pipe) })
        .await
        .expect("the drop task finished");

    let closed = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        handle.status_changed_since(modelpipe::PipeStatus::Idle),
    )
    .await
    .expect("the courtesy close runs from inside the runtime too");
    assert_eq!(closed, Some(modelpipe::PipeStatus::Closed));
}

/// The key lands in the directory it was given, under the name the app
/// computes for itself.
///
/// The literal is the point. `GOOD_TICKET_KEY` is what ggchat's own
/// `Ticket.digest` produced in Swift before this crate named anything, so a
/// device paired on 0.3.x already holds a file under it. Asserting the exact
/// contents of the directory rather than that the file exists, because a
/// leftover `private_file` temporary is also a failure and an `exists` check
/// steps straight over one.
#[tokio::test]
async fn a_dial_keeps_its_key_in_the_directory_it_was_given() {
    let scratch = Scratch::new("dial-keeps-its-key");

    let pipe = mp_connect(GOOD_TICKET.to_owned(), keeping_a_key_in(&scratch))
        .await
        .expect("binds");
    pipe.shutdown().await;

    assert_eq!(scratch.entries(), vec![GOOD_TICKET_KEY.to_owned()]);
    let key = std::fs::read(scratch.path().join(GOOD_TICKET_KEY)).expect("the key is readable");
    assert!(!key.is_empty(), "the key file is empty");
}

/// The whole point of keeping a key: the far machine sees one device.
#[tokio::test]
async fn two_dials_on_one_ticket_are_one_device() {
    let scratch = Scratch::new("two-dials-one-device");

    let first = mp_connect(GOOD_TICKET.to_owned(), keeping_a_key_in(&scratch))
        .await
        .expect("binds");
    let first_id = first.peer_id();
    first.shutdown().await;

    let second = mp_connect(GOOD_TICKET.to_owned(), keeping_a_key_in(&scratch))
        .await
        .expect("binds");
    let second_id = second.peer_id();
    second.shutdown().await;

    assert_eq!(first_id, second_id, "one key file, and yet two devices");
    assert_eq!(scratch.entries(), vec![GOOD_TICKET_KEY.to_owned()]);
}

/// Two machines are two keys, and therefore two devices.
///
/// A name that ignored the ticket would pass every other test in this file
/// and fail this one: both dials would share a key and the phone would
/// present one endpoint to two desktops, which is the thing a relay's
/// one-connection-per-endpoint rule makes unworkable.
#[tokio::test]
async fn two_tickets_keep_two_keys() {
    let scratch = Scratch::new("two-tickets-two-keys");

    let one = mp_connect(GOOD_TICKET.to_owned(), keeping_a_key_in(&scratch))
        .await
        .expect("binds");
    let one_id = one.peer_id();
    one.shutdown().await;

    let other = mp_connect(OTHER_TICKET.to_owned(), keeping_a_key_in(&scratch))
        .await
        .expect("binds");
    let other_id = other.peer_id();
    other.shutdown().await;

    let entries = scratch.entries();
    assert_eq!(
        entries.len(),
        2,
        "two machines did not get two files: {entries:?}"
    );
    assert!(entries.contains(&GOOD_TICKET_KEY.to_owned()), "{entries:?}");
    assert_ne!(one_id, other_id, "two machines met the same device");
}

/// No directory, no file — and a fresh device every time, which is what
/// every version before this one did.
#[tokio::test]
async fn no_directory_leaves_nothing_behind() {
    let scratch = Scratch::new("no-directory-nothing-behind");

    let first = mp_connect(GOOD_TICKET.to_owned(), offline_options())
        .await
        .expect("binds");
    let first_id = first.peer_id();
    first.shutdown().await;

    let second = mp_connect(GOOD_TICKET.to_owned(), offline_options())
        .await
        .expect("binds");
    let second_id = second.peer_id();
    second.shutdown().await;

    assert_ne!(
        first_id, second_id,
        "a dial keeping no key reported the same device twice, so this test proves nothing"
    );
    assert!(scratch.entries().is_empty());
}

/// A directory that is not there is not made here.
///
/// The app creates it, `0o700` at creation and out of its backups, and one
/// made here would have neither property until the app's next call. So this
/// is a refusal that names the file, and it is permanent.
#[tokio::test]
async fn a_directory_that_is_not_there_is_not_created() {
    let scratch = Scratch::new("directory-not-created");
    let absent = scratch.path().join("not-made-here");

    let error = mp_connect(
        GOOD_TICKET.to_owned(),
        MpConnectOptions {
            identity_dir: Some(absent.to_str().expect("UTF-8").to_owned()),
            ..offline_options()
        },
    )
    .await
    .expect_err("there is nowhere to write the key");

    assert!(matches!(error, MpError::Identity { .. }), "{error:?}");
    assert!(!error.is_retryable());
    assert!(
        error.message().contains(GOOD_TICKET_KEY),
        "the refusal does not name the file: {}",
        error.message()
    );
    assert!(!absent.exists(), "the directory was created after all");
}

/// A key this side cannot use is thrown away and the dial tried once more.
///
/// modelpipe refuses a key file that is not a key, or that somebody else can
/// read, and says so permanently. The remedies it names are deleting the file
/// and starting again, or `chmod 600` for the second — and on a phone there
/// is nobody to do either. The cost of throwing it away is this device's
/// fingerprint on the far machine, which records fingerprints and does not
/// pin them; the alternative is a device that can never dial that machine
/// again.
#[tokio::test]
async fn a_key_this_side_cannot_use_is_replaced_and_the_dial_succeeds() {
    let scratch = Scratch::new("unusable-key-replaced");
    let path = scratch.path().join(GOOD_TICKET_KEY);
    std::fs::write(&path, b"not a key at all\n").expect("writable");

    let pipe = mp_connect(GOOD_TICKET.to_owned(), keeping_a_key_in(&scratch))
        .await
        .expect("the unusable key was thrown away and the dial tried again");
    pipe.shutdown().await;

    let now = std::fs::read(&path).expect("a key was minted in its place");
    assert_ne!(
        now, b"not a key at all\n",
        "the unusable key is still there"
    );
    assert_eq!(scratch.entries(), vec![GOOD_TICKET_KEY.to_owned()]);
}

/// A key half written is the shape a crash used to leave behind.
///
/// modelpipe before 0.7.0-rc.1 wrote the key into the file in two steps, so a
/// process killed between them left nothing in it — and an empty file is not
/// a key, so every later dial to that machine was refused for ever. 0.6 is
/// what the devices upgrading to this release are running, so this is the
/// file that is actually out there. 0.7 writes atomically and cannot produce
/// one any more; it still refuses one, deliberately, rather than minting over
/// a path it does not own. Throwing it away is this side's job because this
/// side owns the name.
#[tokio::test]
async fn an_empty_key_file_is_replaced() {
    let scratch = Scratch::new("empty-key-replaced");
    let path = scratch.path().join(GOOD_TICKET_KEY);
    std::fs::write(&path, b"").expect("writable");

    let pipe = mp_connect(GOOD_TICKET.to_owned(), keeping_a_key_in(&scratch))
        .await
        .expect("the empty key was thrown away and the dial tried again");
    pipe.shutdown().await;

    assert!(
        !std::fs::read(&path)
            .expect("a key was minted in its place")
            .is_empty(),
        "the key file is still empty"
    );
}

/// Once, and only when something was thrown away.
///
/// A directory standing where the key belongs cannot be removed, so there is
/// nothing to throw away and the refusal goes to the caller rather than
/// starting a second dial. It is the portable way to reach that arm:
/// `remove_file` on a directory fails on both hosts this is built for, where
/// a read-only directory would simply not stop a superuser.
#[tokio::test]
async fn a_key_file_that_is_a_directory_is_not_discarded() {
    let scratch = Scratch::new("key-is-a-directory");
    let path = scratch.path().join(GOOD_TICKET_KEY);
    std::fs::create_dir(&path).expect("writable");

    let error = mp_connect(GOOD_TICKET.to_owned(), keeping_a_key_in(&scratch))
        .await
        .expect_err("a directory is not a key and cannot be thrown away");

    assert!(matches!(error, MpError::Identity { .. }), "{error:?}");
    assert!(!error.is_retryable());
    assert!(path.is_dir(), "the directory was removed after all");
}

/// A failure that is not about the key leaves the key alone.
///
/// The discard is the one destructive thing this crate does, so what triggers
/// it is worth a test of its own: only modelpipe saying it could not use the
/// key file. A dial refused for any other reason — here an unusable relay —
/// must not cost this device its fingerprint on a machine it has already
/// introduced itself to.
#[tokio::test]
async fn a_failure_that_is_not_about_the_key_leaves_it_alone() {
    let scratch = Scratch::new("other-failure-keeps-the-key");
    let path = scratch.path().join(GOOD_TICKET_KEY);

    let pipe = mp_connect(GOOD_TICKET.to_owned(), keeping_a_key_in(&scratch))
        .await
        .expect("binds");
    pipe.shutdown().await;
    let minted = std::fs::read(&path).expect("a key was minted");

    let error = mp_connect(
        GOOD_TICKET.to_owned(),
        MpConnectOptions {
            relay_url: Some("not a relay url".to_owned()),
            ..keeping_a_key_in(&scratch)
        },
    )
    .await
    .expect_err("an unusable relay is not a dial");

    assert!(
        !matches!(error, MpError::Identity { .. }),
        "this test needs a failure that is not about the key, got {error:?}"
    );
    assert_eq!(
        std::fs::read(&path).expect("the key is still there"),
        minted,
        "a failure that had nothing to do with the key threw it away"
    );
}
