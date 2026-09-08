//! The one tokio runtime this library owns.
//!
//! `UniFFI`'s `async_runtime = "tokio"` polls exported futures inside a tokio
//! context, but a *context* is not a *runtime*: something has to own the
//! worker threads, and on the Swift side nothing does. An app links a static
//! library and calls a function; there is no `#[tokio::main]` anywhere above
//! it.
//!
//! So the library owns one, lazily, for the life of the process. That is the
//! right lifetime for it — a pipe outlives any single call, and modelpipe
//! spawns the dial, the path watcher and the forwarding loop onto whatever
//! runtime was current when `connect` was called. A per-call runtime would be
//! dropped the moment `connect` returned, taking those tasks with it.
//!
//! It is also what makes teardown correct. `ConnectHandle`'s own `Drop` is
//! synchronous and needs nothing, but anything that reaches for
//! `Handle::try_current()` finds this one rather than nothing.

use std::sync::OnceLock;

use tokio::runtime::{Builder, Runtime};

/// The process-wide runtime, built on first use.
static RUNTIME: OnceLock<Runtime> = OnceLock::new();

/// Borrow the runtime, building it if this is the first call.
///
/// # Panics
///
/// If the runtime cannot be built, which means the OS refused to start
/// threads. There is no useful recovery: without a runtime nothing in this
/// crate can work, and returning an error per call would push an
/// unrecoverable condition into every signature to no benefit.
pub(crate) fn runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| {
        Builder::new_multi_thread()
            .enable_all()
            // Named so a thread in a crash report or a debugger's thread list
            // says which library it belongs to. An app carrying several Rust
            // libraries otherwise shows a stack of identical `tokio-runtime-
            // worker` entries.
            .thread_name("modelpipe-ffi")
            // Two is enough and the cap is the point: the work here is a dial,
            // a status watcher and a byte-forwarding loop, none of it CPU
            // bound. The default is one thread per core, which on a phone
            // means spawning workers that only ever compete for the same
            // socket.
            .worker_threads(2)
            .build()
            .expect("a tokio runtime; without one nothing in this library can run")
    })
}

/// Run a future to completion on the library's runtime, from a thread that is
/// not already inside it.
///
/// Used only by the synchronous accessors, which are cheap reads that happen
/// to sit behind an async upstream method. Never used for `connect` or for
/// waiting on a status change — those are `async` all the way to Swift, and
/// blocking a caller's thread on them is exactly the behaviour that would make
/// the app's main actor stutter.
pub(crate) fn block_on<F: Future>(future: F) -> F::Output {
    runtime().block_on(future)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The runtime is one runtime, not one per call: a pipe's spawned tasks
    /// outlive the call that created them, and a fresh runtime per call would
    /// drop them at the end of it.
    #[test]
    fn the_runtime_is_the_same_one_every_time() {
        let first = std::ptr::from_ref(runtime());
        let second = std::ptr::from_ref(runtime());
        assert!(
            std::ptr::eq(first, second),
            "runtime() handed out two different runtimes"
        );
    }

    /// It can actually run something, which the `OnceLock` alone does not say.
    #[test]
    fn the_runtime_runs_a_future() {
        assert_eq!(block_on(async { 2 + 2 }), 4);
    }
}
