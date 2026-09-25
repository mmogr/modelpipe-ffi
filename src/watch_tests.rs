//! A watch ends its wait when the status moves on, when the pipe closes, or
//! when it is cancelled. The last is the property the boundary cannot express
//! for `status_changed_since`, and the one these tests pin from both sides of
//! the wait's first poll.

use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use super::*;
use crate::options::MpConnectOptions;
use crate::pipe::mp_connect;
use crate::runtime::runtime_tests::poll_on_this_thread;

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

/// Cancelled before its first poll, a wait answers `None` on that poll: the
/// token holds the cancel, so nothing about the order matters.
#[tokio::test]
async fn a_watch_cancelled_before_its_first_poll_returns_at_once() {
    let pipe = mp_connect(GOOD_TICKET.to_owned(), offline_options())
        .await
        .expect("binds");
    let watch = pipe.watch();
    let mut next = pin!(watch.next(MpPipeStatus::Idle));
    watch.cancel();

    let mut cx = Context::from_waker(Waker::noop());
    assert!(
        matches!(next.as_mut().poll(&mut cx), Poll::Ready(None)),
        "a wait cancelled before it started answers on its first poll"
    );
}

/// Cancelled after a first poll registered the wait, it answers `None` on
/// the next poll. Between the two polls is exactly where a notification
/// with no waiter would have been lost.
#[tokio::test]
async fn a_watch_cancelled_after_its_first_poll_returns_on_the_next_poll() {
    let pipe = mp_connect(GOOD_TICKET.to_owned(), offline_options())
        .await
        .expect("binds");
    let watch = pipe.watch();
    let mut next = pin!(watch.next(MpPipeStatus::Idle));
    let mut cx = Context::from_waker(Waker::noop());
    assert!(
        next.as_mut().poll(&mut cx).is_pending(),
        "nothing has changed, so the first poll waits"
    );

    watch.cancel();
    assert!(
        matches!(next.as_mut().poll(&mut cx), Poll::Ready(None)),
        "the cancel is seen by the next poll"
    );
    assert!(watch.is_cancelled());
}

/// End to end on the runtime: a wait in flight on another task ends when the
/// watch is cancelled, and a later wait ends at once.
#[tokio::test]
async fn a_cancelled_watch_ends_its_wait_rather_than_parking() {
    let pipe = mp_connect(GOOD_TICKET.to_owned(), offline_options())
        .await
        .expect("binds");
    let watch = pipe.watch();
    let waiting = {
        let watch = Arc::clone(&watch);
        tokio::spawn(async move { watch.next(MpPipeStatus::Idle).await })
    };
    tokio::time::sleep(Duration::from_millis(50)).await;
    watch.cancel();

    let next = tokio::time::timeout(Duration::from_secs(2), waiting)
        .await
        .expect("a cancelled watch returns rather than parking")
        .expect("the task finished");
    assert_eq!(next, None, "a cancelled watch answers None");
    assert_eq!(watch.next(MpPipeStatus::Idle).await, None);
}

/// The shape a Swift app uses: one thread with no tokio runtime waits on the
/// watch, and another cancels it. The wait is polled inline on the waiting
/// thread, and the cancel from the other thread is what brings it back, with
/// `None`.
#[test]
fn a_wait_on_a_thread_with_no_runtime_ends_when_another_thread_cancels() {
    let pipe =
        poll_on_this_thread(mp_connect(GOOD_TICKET.to_owned(), offline_options())).expect("binds");
    let watch = pipe.watch();
    let waiting = {
        let watch = Arc::clone(&watch);
        std::thread::spawn(move || poll_on_this_thread(watch.next(MpPipeStatus::Idle)))
    };
    std::thread::sleep(Duration::from_millis(50));
    watch.cancel();

    let next = waiting.join().expect("the waiting thread finished");
    assert_eq!(next, None, "a cancelled watch answers None");
    assert_eq!(pipe.status(), MpPipeStatus::Idle, "a cancel closes nothing");
}

/// Until it is cancelled, a watch reports what `status_changed_since` does:
/// the close, and then the end of the sequence.
#[tokio::test]
async fn a_watch_reports_the_transitions_the_pipe_makes() {
    let pipe = mp_connect(GOOD_TICKET.to_owned(), offline_options())
        .await
        .expect("binds");
    let watch = pipe.watch();
    let waiting = {
        let watch = Arc::clone(&watch);
        tokio::spawn(async move { watch.next(MpPipeStatus::Idle).await })
    };
    pipe.shutdown().await;

    let next = tokio::time::timeout(Duration::from_secs(5), waiting)
        .await
        .expect("the close is reported")
        .expect("the task finished");
    assert_eq!(next, Some(MpPipeStatus::Closed));
    assert_eq!(
        watch.next(MpPipeStatus::Closed).await,
        None,
        "the sequence ends after the close, as status_changed_since's does"
    );
    assert!(!watch.is_cancelled(), "ending is not cancelling");
}
