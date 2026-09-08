//! The dial refuses what it should refuse, and a real pipe behaves the way
//! the app's seam says it must.
//!
//! These need no far machine. `connect` returns once the local port is bound,
//! so every property below — the base URL's shape, the status starting at
//! `Idle`, shutdown being idempotent and terminal — is observable against a
//! ticket that names a machine which does not exist. That is the contract, not
//! a shortcut: a pipe to an absent machine is a perfectly good pipe.

use super::*;

/// modelpipe's normative ticket vector 1: well-formed, and names an endpoint
/// nothing is listening on.
const GOOD_TICKET: &str = "pipeadlvvgabqkyqvn6vjp7nhslea45a5yls6pnkmizfv4bbu2hxa5iruaaauhlp2na";

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
