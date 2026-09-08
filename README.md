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

## Status is polled, not streamed

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

`make xcframework` produces three bundles, one architecture each:

| Bundle | Architecture |
|---|---|
| `ios-arm64` | iPhone |
| `ios-arm64-simulator` | Simulator on Apple silicon |
| `macos-arm64` | The Mac app |

**Apple silicon only.** The two x86_64 targets were dropped deliberately —
they serve an Intel Mac running the simulator and an Intel Mac app, neither of
which this ships to. A consumer must agree: Xcode's Release configuration
leaves `ONLY_ACTIVE_ARCH` at its default `NO` and so asks for every standard
architecture unless `ARCHS` says otherwise.

Every build then asserts each slice carries the platform load command it
claims, a deployment floor that is not rustc's broken default, and exactly one
architecture (`scripts/check-slices.sh`). Those checks exist because a
cross-compile which silently produces host objects succeeds everywhere else
and fails at link time in the consuming app, days later and to someone who did
not build it.

## Consuming it

```swift
dependencies: [
    .package(url: "https://github.com/mmogr/modelpipe-ffi.git", from: "0.1.1"),
],
targets: [
    .target(
        name: "YourTarget",
        dependencies: [.product(name: "Modelpipe", package: "modelpipe-ffi")]
    ),
]
```

Then `import Modelpipe`. That is the whole of it — **the seven system
frameworks the static library needs ship with the package**, declared on its
own target, so a consumer declares none.

That is most of why this is a package rather than a pair of release assets.
A static library does not carry its own dependencies: when rustc links a
binary it passes those frameworks itself, but a `.a` handed to someone else
records only that it *references* the symbols, not where they live. Leave one
out and everything compiles, right up to the last step:

```
__RNvMs_...system_configuration...SCNetworkInterfaceType13from_cfstring
    in libmodelpipe_ffi.a[arm64]
ld: symbol(s) not found for architecture arm64
```

`.binaryTarget` accepts no build settings at all, so those settings can only
live on a source target — and the one that knows which frameworks iroh's
transitive dependencies reach for is this repository's, not yours. The list is
read off rustc's own link invocation for the iOS target, minus the ones SwiftPM
already passes (`System`, `c`, `m`).

`scripts/swift-smoke.sh` builds against **this package**, not a copy of it, on
every CI run — so the snippet above is executed rather than merely written
down, and so is the claim that the linker settings reach you.

### Pin a tag, never a branch

The binding and the `.a` are one thing in two files. `Sources/Modelpipe/modelpipe_ffi.swift`
carries UniFFI's API checksums and calls `fatalError("UniFFI API checksum mismatch")`
when they disagree with the library — a crash on a device at the first dial,
not an error at build time.

At a tag the two are consistent by construction. On `main` they are not:
`Package.swift` names the *previous* release's artifact while the binding is
already ahead of it. That is a true record — it names a zip that exists — but
it is not a consumable one.

### Showing an error: `message()`, never `localizedDescription`

`MpError` carries a sentence written to be read by whoever is holding the
phone, and `message()` is how to get it:

```swift
} catch let error as MpError {
    show(error.message())                     // "The other machine did not answer…"
    if error.isRetryable() { offerRetry() }
}
```

Reaching for `localizedDescription` instead compiles, type-checks, and is
wrong. UniFFI generates `errorDescription` for every error enum as
`String(reflecting: self)`, so it yields the Swift *debug* rendering of the
case and its payload:

```
modelpipe_ffi.MpError.Bind(reason: "Address already in use (os error 48)")
```

Nothing fails; a person is simply shown the inside of the binding. The smoke
test asserts `message()` contains none of that shape, so the two cannot
quietly become the same thing.

## Releasing

[release-plz](https://release-plz.dev) maintains a release PR on every push to
`main` — the version bump and the CHANGELOG entry, read off the conventional
commits since the last tag. Merging that PR *is* the decision to release.

What it pushes is **`build/vX.Y.Z`**, and that is not the release tag. It is a
build trigger. `release.yml` picks it up, builds the XCFramework optimised,
runs the Swift smoke test against the artifact it is about to publish, writes
that artifact's checksum into `Package.swift` on `main`, and only then creates
the public **`vX.Y.Z`** tag on that commit.

The order is the whole design. SwiftPM loads `Package.swift` from the file view
at whichever tag it resolves, so a release tag naming a commit whose checksum is
stale is a package nobody can build. There is no chicken and egg — `Package.swift`
is not an input to the artifact, since the build never opens a manifest and the
zip holds only the XCFramework — just an order: **build, pin, tag.**

`build/vX.Y.Z` never appears as a package version: SwiftPM's `Version(tag:)`
strips at most one leading `v` and cannot parse the rest.

The last step of a release is a consumer resolving it. `release.yml` synthesises
a throwaway package depending on the version just published, with a `.dynamic`
product so the link is forced, and builds it. That one step executes the entire
claim at once — the tag exists, its manifest names the uploaded zip, the
checksum matches, the binding compiles against it, and all seven linker settings
reach a consumer that declares none. Everything before it tests the repository;
this tests the release.

Nothing is published to crates.io. The product here is a binary, so
`publish = false` appears twice — in `Cargo.toml` to stop a human, and in
`release-plz.toml` to stop the automation.

Two things worth knowing:

- **`release.yml` commits to `main`.** One commit per release, pinning the
  checksum, authored as the repository owner. It needs the ruleset protecting
  `main` to allow a repository-admin bypass, because `RELEASE_PAT` authenticates
  as its owner. The push happens *before* the release is created, so if it is
  ever refused, no tag and no release exist and no version has been burned.
- **It needs a `RELEASE_PAT` secret** — a fine-grained PAT scoped to this
  repository, Contents and Pull requests read/write. Not a preference: events
  created with the default `GITHUB_TOKEN` trigger no workflows, so the release
  PR would get no CI and the build tag would never start `release.yml`.

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
