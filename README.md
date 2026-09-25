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
| `pipe.waitReachable(withinMs:)` | Waits until the far machine is reached and says how it is routed, or throws `MpUnreached`; a timeout tears nothing down. |
| `pipe.peerId()` | Who this device connects as: sixty-four hex characters, stable across launches when `identityDir` is set and the same ticket is dialled. |
| `mpPair(pairing:label:options:reachWithinMs:)` | Dial, wait to reach the far machine, redeem the code, and return `MpPaired`: the pipe still up, this device's key, the name it is held under, and the far machine's id. The key is returned once and never kept here. |
| `mpReadPairing(pairing:)` | Read a pairing string without pairing, synchronously: the ticket in canonical form and whether there is a code, so a form can accept a paste as it is typed and know whether to ask for a token. The code never comes back. Throws `MpPairError.BadPairingString`. |
| `MpConnectOptions.identityDir` | The directory this device keeps its endpoint keys in, so the far machine sees the same device every time — one file per far machine, named below. `nil` mints a key per process and writes nothing. |
| `pipe.watch()` | A cancellable wait on the status sequence: `watch.next(snapshot:)` answers like `statusChangedSince`, and `watch.cancel()` ends it, before or during the wait. |

### Where a device keeps its key

`identityDir` is a **directory**, and this library names the file in it: one
per far machine, called `<digest>.key`, where the digest is the first eight
bytes of SHA-256 over the ticket **in its canonical form**, as sixteen
lower-case hex characters. modelpipe's normative ticket vector 1 is
`0382e9033d890983.key`.

That is a contract with the consuming app rather than an implementation
detail. ggchat computed the same name in Swift before this library named
anything, and every device already paired holds a file under it — so a
different name would fail nowhere and simply make every paired phone
introduce itself to its desktop as a new device, for ever.
`src/identity_file_tests.rs` pins the bytes against modelpipe's own vectors,
including the one whose spelling changes on the way in.

**The canonical form, not the string that was passed in.** A ticket is parsed
before it is hashed, because modelpipe's `Display` sorts and deduplicates
addresses, drops an address tag it does not know, and lower-cases. So the
same machine written two ways is one file — and hashing the argument would
have given it two.

One file per machine because a relay allows a single live connection per
endpoint id: a device holding two machines has to meet each of them as a
different device, or the second dial takes the first's relay path away.

**The directory has to be there already.** Nothing here creates one. An app
that wants its keys unreadable by others and out of its backups sets both as
the directory is made, and neither survives being applied afterwards. A
directory that is missing or cannot be written is `MpError.Identity`, which
names the file and is not retryable.

**A key this side cannot use is thrown away and the dial tried once more.**
modelpipe refuses a key file that is not a key, or that somebody else can
read, and refuses it permanently. The remedies it names are deleting the file
and starting again, or `chmod 600` for the second — and on a phone there is
nobody to do either. The cost of throwing it away is this device's fingerprint
on the far machine, which records fingerprints and does not pin them; the
alternative is a device that can never dial that machine again. A file half
written by a process that was killed is enough to earn it, and that is the
shape modelpipe before 0.7.0-rc.1 could leave behind.

Once, and only when there was a file to throw away, and only when the
refusal was about the key: a dial that fails for any other reason leaves the
key untouched. A pairing does the same, which it could not do before —
`MpPairError` folds every transport failure into one sentence, so the retry is
written underneath that, against modelpipe's own error, and no new case
crosses into Swift. It is safe there because `modelpipe::pair` dials, waits to
reach the far machine, and only then presents the code, so a dial that failed
on the key has spent nothing.

### No credential is accepted across this boundary

`mpConnect` takes **no token**, because `modelpipe::connect` takes none either.
The connecting side is a plain local HTTP listener that forwards
`Authorization` verbatim; the serve edge is the only thing that checks it. So
the bearer token belongs to whatever HTTP client you point at `baseUrl()`, and
this library never stores or logs one.

One credential is **returned**, once: `mpPair` hands back the device key the
far machine minted, as `MpPaired.apiKey`, and this library does not keep it.
Its Rust `Debug` and its Swift `description` and `debugDescription` render the
key redacted (the Swift ones are a hand-written extension beside the generated
file, pinned by the smoke test). `Mirror`, `dump()` and a direct read of
`apiKey` are not covered: the value is a plain `String` the app owns from the
moment `mpPair` returns. The pairing code arrives inside the pairing string and
no error renders it.

That is a gate, not a promise: `scripts/check_no_credentials.sh` fails the
build if an exported function grows a credential-shaped parameter, or if
anything formats a ticket or a token into output. `mpPair` passes it
unchanged: a returned field is neither.

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

A wait ends when the pipe closes, or when a watch is cancelled. Cancelling
the Swift `Task` that awaits `statusChangedSince` does not cross the boundary
(the generated Swift never calls `rust_future_cancel`), so the Rust future
stays parked until the next status change. To end a wait while the pipe stays
up, hold a `pipe.watch()` and call `cancel()` on it; its `next(snapshot:)`
answers `nil` from then on.

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
    .package(url: "https://github.com/mmogr/modelpipe-ffi.git", from: "0.1.2"),
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

At a tag the two are consistent by construction. On `main` they need not be:
`Package.swift` names the artifact of the last version pinned on `main` while
the binding can already be ahead of it, so `main` is not consumable. That
artifact's URL resolves only if its version's release was published;
[Releasing](#releasing) says when a pinned version is left unpublished.

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

The pin commit's parent is the commit the artifact was built from, it reaches
`main` only while `main` is still that commit, and the release tag is checked
to be the pin. That is what makes the binding at a release tag and the zip its
manifest names one build.

`build/vX.Y.Z` never appears as a package version: SwiftPM's `Version(tag:)`
strips at most one leading `v` and cannot parse the rest.

The last step of a release is a consumer resolving it and calling into it.
`release.yml` synthesises a throwaway executable depending on the version just
published, builds it, checks that the tag resolved to the pin, and runs it.
That step meets the release as a consumer does: the tag resolves to the pin,
so its manifest names the zip just uploaded; the download matches the
checksum; the binding compiles against it; the linker settings the package
declares are enough to link a consumer that declares none; and the library
answers a call. The call adds UniFFI's own check: `mpReadPairing` is the first
call into the library, which is where UniFFI compares the binding's API
checksums with the library's and calls `fatalError` when they differ, so a
pair that links but disagrees on a checksum fails here, where a gate that only
linked would pass it. Everything before it tests the repository; this tests
the release.

That step has not yet passed on the runner. In every release run from v0.1.2
to v0.4.1 the runner's download of the zip stalled and the step was stopped
([#24](https://github.com/mmogr/modelpipe-ffi/issues/24)). Until that is
fixed, a red last step after a green "Create the release" means the release
exists and its tag is the pin, and the release has to be checked from a
machine: resolve the version from a scratch package, as the step does.
A re-run publishes nothing further: `verify` and the pin step refuse a version
whose pin or release already exists.

Nothing is published to crates.io. The product here is a binary, so there
are two guards: `release-plz.toml` says `publish = false`, which stops the
automation, and `Cargo.toml` says `publish = ["nowhere"]`, a registry
allow-list naming no registry that exists, which stops a human's
`cargo publish` before it uploads anything. The manifest deliberately does not
say `false`: release-plz's `release` command ignores a package whose manifest
says it can be published nowhere, and while it did, merging a release PR never
pushed the build tag: every tag up to `build/v0.1.4` was pushed by hand, and
`build/v0.2.0` is too, because its merge commit carries the old manifest
([#21](https://github.com/mmogr/modelpipe-ffi/issues/21)). Never configure a
registry named `nowhere`; cargo selects the only allowed registry by itself.

Worth knowing:

- **A tag whose release already exists stops the workflow.** The build is not
  byte-reproducible, so a second run of a version would compute a different
  checksum, pin it, and only then find the release there — and SwiftPM refuses
  a version whose checksum ever changes. The guard is also what made
  `build/v0.1.0` safe to create as release-plz's baseline; see
  `release-plz.toml` for why that was needed.
- **`release.yml` commits to `main`.** One commit per release, pinning the
  checksum, authored as the repository owner, on top of the commit the artifact
  was built from. The push is plain, never forced, so it lands only as a
  fast-forward of that commit. It needs the ruleset protecting `main` to allow a
  repository-admin bypass, because `RELEASE_PAT` authenticates as its owner. The
  push happens *before* the release is created, so if it is ever refused, no tag
  and no release exist. One release runs at a time: a second waits for the
  first rather than cancelling it.
- **Merge nothing between the release PR and the end of its run.** If `main`
  moves before the pin is pushed, the release stops: `verify` refuses a build
  tag `main` has moved past, and the pin step checks again after the build,
  because a pin on the newer `main` would put this zip beside a binding it was
  not built from. Nothing is published — no pin, no `vX.Y.Z`, no release — but
  `build/vX.Y.Z` exists, and release-plz reads versions off those tags, so it
  counts X.Y.Z as released. **Leave X.Y.Z unpublished and take the version
  the next release PR proposes** (X.Y.Z+1, or X.(Y+1).0 if a feature or a
  breaking change landed);
  0.1.1 and 0.1.4 have build tags and no release too. The one exception is a
  hotfix that has to ship as X.Y.Z: move the build tag to `main`'s tip and
  push it again
  (`git fetch origin && git tag -f build/vX.Y.Z origin/main && git push -f origin build/vX.Y.Z`).
  That releases everything on `main` under X.Y.Z's changelog, and works only
  while `Cargo.toml` on `main` still says X.Y.Z.
- **If the pin landed and the release did not,** `main` carries the pin and
  `vX.Y.Z` does not exist. Do not re-run the workflow to recover. A re-run
  publishes nothing further: `verify` and the pin step refuse a version whose
  pin or release already exists. "Re-run failed jobs" also rebuilds, and
  reaches "Upload the artifact", under the same artifact name, before it
  reaches the pin step; the zip the pin names exists only in the attempt that
  built it. Within 14 days, the artifact's retention, publish that zip by
  hand. List the run's artifacts of that name by id:

  ```sh
  gh api repos/mmogr/modelpipe-ffi/actions/runs/<run-id>/artifacts \
    --jq '.artifacts[] | select(.name == "ModelpipeFFI.xcframework.zip") | "\(.id) \(.created_at) \(.expired)"'
  ```

  Download each one by its id, and compare its checksum with the one
  `Package.swift` names at the pin:

  ```sh
  gh api repos/mmogr/modelpipe-ffi/actions/artifacts/<id>/zip > artifact.zip && unzip -o artifact.zip
  swift package compute-checksum ModelpipeFFI.xcframework.zip
  git show <pin>:Package.swift | grep '^let checksum'
  ```

  Publish only the zip whose checksum is the pin's:
  `gh release create vX.Y.Z ModelpipeFFI.xcframework.zip --target <pin> --title vX.Y.Z --generate-notes`.
  Never use `gh run download -n` here: it chooses an artifact by its name,
  and only the checksum identifies the zip the pin names. If no artifact
  matches — they expired, or were replaced — leave X.Y.Z unpublished and take
  the version the next release PR proposes, as when `main` moved first; the
  re-pushed build tag is for a hotfix only. The resolve gate does not run for
  a release published by hand, so resolve the version from a scratch package
  before relying on it.
- **It needs a `RELEASE_PAT` secret** — a fine-grained PAT scoped to this
  repository, Contents and Pull requests read/write. Not a preference: events
  created with the default `GITHUB_TOKEN` trigger no workflows, so the release
  PR would get no CI and the build tag would never start `release.yml`.

## Status

Early. The crate builds, its suite passes against real loopback sockets, and
the generated Swift has the shape ggchat's FFI seam specifies, plus pairing, a
lasting identity and a cancellable watch, which ggchat does not use yet. What has **not**
happened yet is the measurement that matters: a build on a physical iPhone,
on a carrier network, dialling a desktop. Compiling is not running —
`portmapper`, `igd-next`, `netdev` and `hickory-resolver` all want entitlements
or permissions a simulator never asks for. Nothing here should be read as
saying that works until it has been seen to.

## Licence

MIT, matching modelpipe.
