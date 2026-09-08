# modelpipe-ffi

[![CI](https://github.com/mmogr/modelpipe-ffi/actions/workflows/ci.yml/badge.svg)](https://github.com/mmogr/modelpipe-ffi/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

**Swift bindings for [modelpipe](https://github.com/mmogr/modelpipe), so an
iPhone or a Mac can reach a model server at home.**

modelpipe puts a local model server on your other devices over an end-to-end
encrypted peer-to-peer connection — no VPN, no account, no cloud in the path.
It is a Rust library. This repository is what lets a Swift app call it: a
static library with a C ABI, a generated Swift wrapper, and an XCFramework
that packages both for every Apple platform.

```swift
let pipe = try await mpConnect(ticket: pairingTicket, options: MpConnectOptions())
// pipe.baseUrl() is now "http://127.0.0.1:<port>/v1", and it *is* the
// machine at home. Point any OpenAI-compatible client at it.
```

## Why this is its own repository

modelpipe is a portable Rust library that publishes to crates.io. Its CI runs
`--workspace` clippy and tests on Windows, its release job runs
`cargo publish --workspace`, and it denies `unsafe_code` across the workspace.
An Apple-only static library fights all three: it does not compile on Windows,
it is useless on a registry, and crossing a language boundary is unsafe code
by definition.

[ggchat](https://github.com/mmogr/ggchat), the app that consumes this, has the
opposite constraint — a CI gate that fails the build if any Rust appears in
the repository at all, which is what keeps it a pure Swift app that builds on
any Mac without a Rust toolchain.

So this sits between them: it depends on `modelpipe` from crates.io like any
other consumer, and ships an XCFramework the app downloads. Both repositories'
invariants stay literally true.

## What it binds

**The connect half only.** A phone dials a machine that is already serving; it
never serves. `serve`, `ServeHandle` and `TokenPolicy` are absent by decision —
half the surface is half the API to keep working across a modelpipe release,
and nothing on an iPhone wants to be a backend.

| Swift | What it does |
|---|---|
| `mpConnect(ticket:options:)` | Dial. Returns once the **local port is bound**, not once the far machine answers. |
| `pipe.baseUrl()` | `http://127.0.0.1:<port>/v1`. Stable for the pipe's life. |
| `pipe.status()` | `idle`, `direct`, `relayed` or `closed`, now. |
| `pipe.statusChangedSince(snapshot:)` | Waits for the next value different from `snapshot`. `nil` ends the sequence. |
| `pipe.closeReason()` | Why it closed, or `nil` while open. |
| `pipe.notifyNetworkChange()` | Call on every app resume. See below. |
| `pipe.networkMetrics()` | Relay counters, including rate limiting. |
| `pipe.shutdown()` | Idempotent and terminal. |

### No credential crosses this boundary

`mpConnect` takes **no token**, because `modelpipe::connect` takes none either.
The connecting side is a plain local HTTP listener that forwards
`Authorization` verbatim; the serve edge is the only thing that checks it. So
the bearer token belongs to whatever HTTP client you point at `baseUrl()`, and
this library never sees, stores or logs one.

That is a gate, not a promise: `scripts/check_no_credentials.sh` fails the
build if an exported function grows a credential-shaped parameter, or if
anything formats a ticket or a token into output.

### Status is polled, not streamed

There is no callback across the boundary. Rebuild it as an `AsyncStream` on
the Swift side:

```swift
var held = pipe.status()
continuation.yield(held)                        // current value first
while let next = await pipe.statusChangedSince(snapshot: held) {
    held = next
    continuation.yield(held)
}
continuation.finish()                           // only after a close
```

The caller supplies the snapshot deliberately. modelpipe's own documentation
calls this "the form to reach for from a language binding": the coalescing
alternative snapshots *inside itself*, so a generated `next()` drops the
transition it was woken to report and then spins a core answering `Closed`
forever. The caller-supplied snapshot is what makes the sequence terminate.

### Call `notifyNetworkChange()` on every resume

iOS is one of the hosts iroh cannot watch for itself — sleep/wake detection
there is disabled in favour of a poll measured in the hour. A phone that
resumes on a new cellular bearer has a pipe with nothing to repair it until
that poll comes round, unless the app says so. It is harmless when nothing
changed, so call it every time rather than trying to be clever about when.

## Building

```bash
make pre-commit    # everything CI checks on Linux
make xcframework   # the five slices and the framework (macOS, needs Xcode)
make checksum      # zip it and print the SwiftPM checksum
```

`make xcframework` produces three bundles covering five slices:

| Bundle | Architectures |
|---|---|
| `ios-arm64` | iPhone |
| `ios-arm64_x86_64-simulator` | Simulator on Apple silicon and Intel |
| `macos-arm64_x86_64` | The Mac app |

Every build then asserts each slice carries the platform load command it
claims (`scripts/check-slices.sh`). That check exists because a cross-compile
which silently produces host objects succeeds everywhere else and fails at
link time in the consuming app, days later and to someone who did not build
it.

## Consuming it

Add the XCFramework from a release as a `binaryTarget`:

```swift
.binaryTarget(
    name: "ModelpipeFFI",
    url: "https://github.com/mmogr/modelpipe-ffi/releases/download/vX.Y.Z/ModelpipeFFI.xcframework.zip",
    checksum: "<the checksum in the release notes>"
)
```

## Status

Early. The crate builds, its suite passes against real loopback sockets, and
the generated Swift has the shape ggchat's FFI seam specifies. What has **not**
happened yet is the measurement that matters: a build on a physical iPhone,
on a carrier network, dialling a desktop. Compiling is not running —
`portmapper`, `igd-next`, `netdev` and `hickory-resolver` all want entitlements
or permissions a simulator never asks for. Nothing here should be read as
saying that works until it has been seen to.

## Licence

MIT, matching modelpipe.
