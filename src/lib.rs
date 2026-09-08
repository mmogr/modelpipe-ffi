//! Swift bindings for [`modelpipe`], so an Apple device can be the connecting
//! side of a pipe.
//!
//! # What this is, and what it deliberately is not
//!
//! This crate binds **the connect half only**. A phone dials a machine that is
//! already serving; it never serves. `modelpipe::serve`, `ServeHandle`, and
//! `TokenPolicy` are therefore absent from everything below, and that is a
//! decision rather than an omission — half the surface is half the API to keep
//! working across a modelpipe release, and nothing on an iPhone wants to be a
//! backend.
//!
//! # The shape the caller sees
//!
//! One free function and one object:
//!
//! ```text
//! mp_connect(ticket, options) -> MpPipe
//!     MpPipe.base_url()             -> String     "http://127.0.0.1:<port>/v1"
//!     MpPipe.status()               -> MpPipeStatus
//!     MpPipe.status_changed_since() -> MpPipeStatus?   (async; None ends it)
//!     MpPipe.close_reason()         -> MpCloseReason?
//!     MpPipe.notify_network_change()                   (async)
//!     MpPipe.network_metrics()      -> MpNetworkMetrics
//!     MpPipe.shutdown()                                (async)
//! ```
//!
//! Every type is prefixed `Mp`. Swift has no namespacing within a module, and
//! the app consuming this already owns a `PipeStatus` of its own
//! (`GGChatCore.PipeStatus`); an unprefixed one here would make every
//! unqualified mention in the connector file ambiguous. The prefix costs a
//! reader two characters and saves the consumer a class of compile error.
//!
//! # No credential crosses this boundary
//!
//! [`modelpipe::ConnectOptions`] has no token field, and `connect` takes no
//! credential of any kind: the connecting side is a plain local HTTP listener
//! that forwards `Authorization` verbatim, and the serve edge is what checks
//! it. So the token belongs to whatever HTTP client the app already points at
//! [`MpPipe::base_url`], and this crate never sees, stores, or logs one. The
//! only secret-shaped thing that reaches it is the ticket, which is redacted
//! wherever it is rendered — see [`MpError`].
//!
//! # Status is polled, not streamed
//!
//! There is no callback and no stream across the boundary. [`MpPipe::status`]
//! answers now, and [`MpPipe::status_changed_since`] waits for the next value
//! *different from the snapshot the caller supplies*, returning `None` once
//! the pipe is closed and the caller already knows it.
//!
//! That is modelpipe's `status_changed_since`, whose own documentation names
//! it "the form to reach for from a language binding" for the reason this
//! crate would otherwise hit: the coalescing sibling `status_changed` snapshots
//! *inside itself*, so a generated `next()` built on it drops the transition it
//! was woken to report, and then spins a core forever answering `Closed`. The
//! caller-supplied snapshot is what makes the sequence terminate.
//!
//! Rebuilding that as a Swift `AsyncStream` is three lines and belongs on the
//! Swift side, where the app's own multicast relay already lives.
//!
//! # The runtime, and who owns it
//!
//! Recorded here because it is a decision with alternatives, not an
//! implementation detail. A phone has no tokio runtime: an app links a static
//! library and calls a function, and there is no `#[tokio::main]` anywhere
//! above it. Something has to own the worker threads.
//!
//! **This library owns one, process-wide, built on first use** — see
//! `src/runtime.rs`. Two workers, multi-threaded, never torn down.
//!
//! Three things follow, each of which was the reason for a rejected
//! alternative:
//!
//! - **Not per-call.** modelpipe spawns the dial, the path watcher and the
//!   forwarding loop onto whatever runtime was current when `connect` was
//!   called. A runtime dropped when `connect` returns takes all three with it,
//!   and the pipe dies the moment it is handed over.
//! - **Not torn down on `shutdown`.** An app holds several providers; one
//!   pipe closing must not stop another's forwarding loop. The runtime
//!   outlives every pipe deliberately, and costs two idle threads to do it.
//! - **Async across the boundary, not a callback interface.** The exported
//!   methods are `async fn`, which `UniFFI` bridges to Swift `async` — so
//!   `statusChangedSince` is awaited from a `Task` like any other Swift
//!   async call, and cancellation, structured concurrency and actor hopping
//!   all work as the app already expects. A callback interface would invert
//!   that, hand Swift a completion handler to bridge back into `AsyncStream`
//!   by hand, and make the ffi responsible for a threading contract the
//!   language already has.
//!
//! Two idle threads for the life of the process is the price. It is the right
//! one on a device where the alternative is a pipe that stops working when a
//! screen is dismissed.

mod error;
mod options;
mod pipe;
mod runtime;
mod status;

pub use error::MpError;
pub use options::MpConnectOptions;
pub use pipe::{MpPipe, mp_connect};
pub use status::{MpCloseReason, MpNetworkMetrics, MpPipeStatus};

// Defines `UniFfiTag` and the `extern "C"` entry points. Must be at the crate
// root: every `uniffi` derive below resolves that tag there.
//
// Note what is *not* here: an `#[allow(unsafe_code)]`. The crate denies it,
// and this still compiles, because UniFFI's own expansion carries the
// exception on the items that need it. So the deny costs nothing and means
// what it says — every `unsafe` in this crate is one UniFFI generated, and a
// hand-written one would not build.
uniffi::setup_scaffolding!();

/// Compile-time pins on the traits the generated binding depends on.
///
/// `UniFFI` hands objects to Swift as `Arc<T>` and may touch them from any
/// thread, so `MpPipe` losing `Send + Sync` would turn a green build here into
/// a failure inside macro expansion, where the error names a generated file
/// nobody wrote. modelpipe pins the same properties on the handle underneath
/// for the same reason; this is that promise restated at the boundary that
/// actually depends on it.
#[expect(dead_code, reason = "compile-time pin; never called")]
const fn auto_trait_promises() {
    const fn assert<T: Send + Sync + 'static>() {}
    const fn assert_copy_eq<T: Copy + Eq>() {}

    assert::<MpPipe>();
    assert::<MpError>();
    assert::<MpConnectOptions>();

    assert_copy_eq::<MpPipeStatus>();
    assert_copy_eq::<MpCloseReason>();
    assert_copy_eq::<MpNetworkMetrics>();
}
