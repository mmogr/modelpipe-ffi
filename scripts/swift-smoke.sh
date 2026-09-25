#!/usr/bin/env bash
#
# Prove the generated Swift links against the XCFramework and that a call
# actually crosses the boundary.
#
# WHAT THIS PROVES
#   - the generated Swift compiles under the same strict-concurrency settings
#     the consuming app uses
#   - the static library links and every symbol resolves
#   - a synchronous call returns a value
#   - the ASYNC calls return. Without the library's tokio runtime entered,
#     the first dial (`mpConnect` in step 2) panics in Rust and Swift stops
#     on a fatal error. With a runtime whose workers never started, the dial
#     returns and the first call after it that waits on the runtime hangs,
#     not errors. The Rust tests poll every async export from a thread with
#     no runtime too, but only this polls them through the generated
#     scaffolding, as an app does
#
# WHAT THIS DOES NOT PROVE
#   Nothing about a network. No hole punching, no NAT traversal, no relay, no
#   phone. A GitHub runner is not a device behind carrier-grade NAT. The dial
#   below names a machine that does not exist and is expected to sit at `idle`
#   forever. That measurement is the on-device spike, and a green run here is
#   not a substitute for it.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FRAMEWORK="${ROOT_DIR}/build/ModelpipeFFI.xcframework"
WORK_DIR="${ROOT_DIR}/build/smoke"

if [[ ! -d "${FRAMEWORK}" ]]; then
    echo "error: ${FRAMEWORK} not found. Run \`make xcframework\` first." >&2
    exit 1
fi

if [[ ! -f "${ROOT_DIR}/Sources/Modelpipe/modelpipe_ffi.swift" ]]; then
    echo "error: Sources/Modelpipe/modelpipe_ffi.swift not found. Run \`make swift\` first." >&2
    exit 1
fi

rm -rf "${WORK_DIR}"
mkdir -p "${WORK_DIR}/Sources/Smoke"

cat > "${WORK_DIR}/Package.swift" <<'SWIFT'
// swift-tools-version: 6.2
import PackageDescription

// Depends on the repository's OWN manifest rather than restating it.
//
// This used to be a hand-written copy: a binaryTarget pointing at a framework
// copied in beside it, the generated Swift copied into this target's sources,
// and the seven linkerSettings spelled out again in a heredoc. It passed, and
// it proved the wrong thing. README.md claimed those settings were "executed
// rather than merely written down" while what CI executed was a duplicate of
// them — so the real manifest could have been wrong in any way at all and this
// would still have gone green.
//
// Consuming the package makes the claim literal, and it puts one more thing
// under test that nothing else here checks: whether SwiftPM propagates a
// dependency's linkerSettings to the consumer's link. If it does not, this
// fails at `ld` and the answer arrives as a red build rather than as a
// discovery in somebody's app.
let package = Package(
    name: "Smoke",
    platforms: [.macOS(.v26)],
    dependencies: [.package(path: "../..")],
    targets: [
        .executableTarget(
            name: "Smoke",
            dependencies: [.product(name: "Modelpipe", package: "modelpipe-ffi")]
        )
    ],
    swiftLanguageModes: [.v6]
)
SWIFT

cat > "${WORK_DIR}/Sources/Smoke/main.swift" <<'SMOKE_SWIFT'
import Foundation
import Modelpipe

// modelpipe's normative ticket vector 1. Well-formed, and names an endpoint
// nothing is listening on — so the dial binds a port and then sits at `idle`,
// which is exactly the contract: `connect` returns once the LISTENER is up,
// not once the far machine answers.
let ticket = "pipeadlvvgabqkyqvn6vjp7nhslea45a5yls6pnkmizfv4bbu2hxa5iruaaauhlp2na"

func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data("smoke: \(message)\n".utf8))
    exit(1)
}

// 1. A refusal crosses the boundary as a typed Swift error, carrying the two
//    things the app reads off it.
//
//    Both are `#[uniffi::export]`ed methods. They were ordinary `pub fn`s on
//    the Rust type first — documented, and covered by five Rust tests — and
//    neither crossed the boundary, because a `pub fn` on a type that crosses
//    the FFI is not part of the FFI. Swift got a bare enum, and
//    `error.isRetryable()` failed to compile in the spike. This check used to
//    catch the error and interpolate it, which proved the type crossed and
//    nothing whatever about its surface.
do {
    _ = try await mpConnect(ticket: "nope", options: MpConnectOptions())
    fail("a malformed ticket was accepted")
} catch let error as MpError {
    guard !error.isRetryable() else {
        fail("a malformed ticket was reported as worth dialling again")
    }

    // The same contract `every_error_is_a_sentence` holds on the Rust side:
    // a whole sentence, so a full stop or a closing bracket.
    let message = error.message()
    guard !message.isEmpty, message.hasSuffix(".") || message.hasSuffix(")") else {
        fail("the error message is not a sentence: \(message)")
    }
    // The trap: UniFFI generates `errorDescription` for every error enum as
    // `String(reflecting: self)`, so `localizedDescription` yields
    // `modelpipe_ffi.MpError.BadTicket(reason: "...")`. That compiles, reads
    // like a message, and shows somebody the inside of the binding. If
    // `message()` ever becomes that, it fails here rather than on a screen.
    for shape in ["MpError", "modelpipe_ffi", "reason:"] {
        guard !message.contains(shape) else {
            fail("the error message renders the variant, not a sentence: \(message)")
        }
    }
    print("ok  a bad ticket is refused, and not retryable: \(message)")
    print("    (localizedDescription would have given: \(error.localizedDescription))")
} catch {
    fail("unexpected error type: \(error)")
}

// 2. A real dial binds, and the sync accessors return.
//
//    Every field is spelled out rather than left to its default. UniFFI emits
//    the memberwise initialiser in DECLARATION order, so reordering the Rust
//    record silently reorders the Swift arguments — which has already broken
//    this build once, on `portMapping` and `discovery`.
let pipe = try await mpConnect(
    ticket: ticket,
    options: MpConnectOptions(
        port: nil,
        relayUrl: nil,
        portMapping: false,
        discovery: false,
        relayOnly: false,
        identityDir: nil
    )
)

let base = pipe.baseUrl()
guard base.hasPrefix("http://127.0.0.1:"), base.hasSuffix("/v1") else {
    fail("base URL is not a loopback /v1 URL: \(base)")
}
guard base.contains(":\(pipe.port())/") else {
    fail("port() disagrees with the base URL: \(pipe.port()) vs \(base)")
}
print("ok  bound \(base)")

guard pipe.status() == .idle else {
    fail("a pipe with nothing at the far end should be idle, got \(pipe.status())")
}
print("ok  status is idle")

guard pipe.closeReason() == nil else {
    fail("an open pipe reported a close reason")
}

// The rate-limited counter is the one nothing else surfaces: a relay
// refusing this endpoint and a network silently dropping the traffic look
// identical from the outside, and the spike's whole diagnosis rests on
// telling them apart.
let metrics = pipe.networkMetrics()
print("ok  metrics read: \(metrics.relayConnections) opened, "
    + "\(metrics.relayConnectionsFailed) failed, "
    + "\(metrics.relayConnectionsRatelimited) rate-limited")

// 2b. The key file lands under the name the consuming app computes for
//     itself, in a directory this library did not create.
//
//     The literal is written out here on purpose. ggchat names the same file
//     in Swift, from its own `Ticket.digest`, and every device paired before
//     this release holds a file under that name. If the two rules ever
//     disagree nothing errors anywhere: every paired phone simply introduces
//     itself to its desktop as a new device, for ever. This is the only place
//     in this repository where the name the generated binding produces meets
//     a hand-written expectation of what it should be, and it is deliberately
//     re-derived here rather than asked of the library.
//
//     What it does NOT prove is agreement with ggchat, which computes the
//     constant in its own language; that half is a test over there.
let keyName = "0382e9033d890983.key"
let keyDir = FileManager.default.temporaryDirectory
    .appendingPathComponent("smoke-identity-\(UUID().uuidString)")
try FileManager.default.createDirectory(at: keyDir, withIntermediateDirectories: true)
let keyDirPath = keyDir.path(percentEncoded: false)

func dialKeeping(identityDir: String?) async throws -> MpPipe {
    try await mpConnect(
        ticket: ticket,
        options: MpConnectOptions(
            port: nil, relayUrl: nil, portMapping: false, discovery: false,
            relayOnly: false, identityDir: identityDir
        )
    )
}

let kept = try await dialKeeping(identityDir: keyDirPath)
let keptId = kept.peerId()
await kept.shutdown()

// The exact contents, not `fileExists`: a leftover temporary from the
// library's atomic write is also a failure, and an existence check walks
// straight past one.
let written = try FileManager.default.contentsOfDirectory(atPath: keyDirPath).sorted()
guard written == [keyName] else {
    fail("the key directory holds \(written), wanted [\(keyName)]")
}
let keyBytes = try Data(contentsOf: keyDir.appendingPathComponent(keyName))
guard !keyBytes.isEmpty else {
    fail("the key file is empty")
}
print("ok  the key is kept at \(keyName)")

// The property the file exists for.
let again = try await dialKeeping(identityDir: keyDirPath)
let againId = again.peerId()
await again.shutdown()
guard keptId == againId else {
    fail("one key file, and yet two devices: \(keptId) then \(againId)")
}
print("ok  a second dial is the same device")

// And the negative: a directory that is not there is not made here. The app
// creates it, private and out of its backups at the moment of creation, and
// one made here would have neither property.
let absent = keyDir.appendingPathComponent("not-made-here")
let absentPath = absent.path(percentEncoded: false)
do {
    _ = try await dialKeeping(identityDir: absentPath)
    fail("a dial into a directory that does not exist succeeded")
} catch let error as MpError {
    guard case .Identity = error, !error.isRetryable() else {
        fail("expected a permanent Identity refusal, got \(error)")
    }
    guard error.message().contains(keyName) else {
        fail("the refusal does not name the file: \(error.message())")
    }
}
guard !FileManager.default.fileExists(atPath: absentPath) else {
    fail("the library created the directory")
}
print("ok  a missing directory is refused, not created")

try? FileManager.default.removeItem(at: keyDir)

// 3. The async methods, awaited through the generated scaffolding like the
//    dial. A call that waits on the library's runtime is where a runtime
//    whose workers never started hangs rather than errors, which is the
//    failure the bound at the bottom of this script exists for.
await pipe.notifyNetworkChange()
print("ok  async notifyNetworkChange returned")

// 3b. A watch ends its wait when cancelled: the cancellation the generated
//     Swift cannot express for statusChangedSince, as an object the app holds.
let watch = pipe.watch()
let waiting = Task { await watch.next(snapshot: MpPipeStatus.idle) }
try? await Task.sleep(nanoseconds: 50_000_000)
guard !watch.isCancelled() else {
    fail("a fresh watch reports itself cancelled")
}
watch.cancel()
guard watch.isCancelled() else {
    fail("cancel did not take")
}
let cancelled = await waiting.value
guard cancelled == nil else {
    fail("a cancelled watch answered \(String(describing: cancelled))")
}
print("ok  a cancelled watch returns")

await pipe.shutdown()
guard pipe.status() == .closed else {
    fail("shutdown did not close the pipe")
}
print("ok  async shutdown returned and the pipe is closed")

// 4. The status sequence terminates rather than repeating a terminal value.
let next = await pipe.statusChangedSince(snapshot: MpPipeStatus.closed)
guard next == nil else {
    fail("the status sequence did not end after a close, got \(String(describing: next))")
}
print("ok  the status sequence ends")

// 5. Pairing crosses the boundary. Nothing listens at the vector ticket, so
//    a pairing string made from it and a fabricated code times out before the
//    code is ever presented; that is the whole exchange short of a far machine.
let fresh = try await mpConnect(
    ticket: ticket,
    options: MpConnectOptions(
        port: nil, relayUrl: nil, portMapping: false, discovery: false,
        relayOnly: false, identityDir: nil
    )
)
do {
    _ = try await fresh.waitReachable(withinMs: 150)
    fail("a pipe to nothing reported itself reached")
} catch let error as MpUnreached {
    guard case .TimedOut = error, error.isRetryable() else {
        fail("expected a retryable timeout, got \(error)")
    }
    print("ok  waitReachable times out: \(error.message())")
} catch {
    fail("unexpected error type: \(error)")
}
let id = fresh.peerId()
guard id.count == 64, id.allSatisfy({ $0.isHexDigit }) else {
    fail("peerId is not sixty-four hex characters: \(id)")
}
print("ok  peerId is sixty-four hex characters")
await fresh.shutdown()

// 5b. A pairing string is read without pairing: the form's question, answered
//     by modelpipe's own parse, synchronously. The code never comes back, and
//     the renderings withhold the ticket.
let read = try mpReadPairing(pairing: " \(ticket.uppercased())-123456 ")
guard read.hasCode, read.ticket == ticket.lowercased() else {
    fail("reading a pairing string with a code got hasCode \(read.hasCode)")
}
let bare = try mpReadPairing(pairing: ticket)
guard !bare.hasCode, bare.ticket == ticket.lowercased() else {
    fail("a ticket alone read as a first pairing")
}
do {
    _ = try mpReadPairing(pairing: "nope-12345")
    fail("reading a string that is not a pairing string succeeded")
} catch let error as MpPairError {
    guard case .BadPairingString = error, !error.isRetryable() else {
        fail("expected BadPairingString, got \(error)")
    }
    let message = error.message()
    guard message.hasSuffix(")."), !message.contains("nope"), !message.contains("12345") else {
        fail("the reading error is not a sentence, or shows the paste: \(message)")
    }
}
for rendering in [String(describing: read), "\(read)", String(reflecting: read)] {
    guard !rendering.contains(ticket.lowercased()), rendering.contains("hasCode: true") else {
        fail("a rendering of MpPairingString shows the ticket: \(rendering)")
    }
}
print("ok  a pairing string is read without pairing, and renders without its ticket")

do {
    _ = try await mpPair(
        pairing: "\(ticket)-123456", label: "smoke",
        options: MpConnectOptions(
            port: nil, relayUrl: nil, portMapping: false, discovery: false,
            relayOnly: false, identityDir: nil
        ),
        reachWithinMs: 150
    )
    fail("pairing with nothing listening succeeded")
} catch let error as MpPairError {
    guard case .Unreached = error, error.isRetryable() else {
        fail("expected a retryable Unreached, got \(error)")
    }
    let message = error.message()
    guard message.hasSuffix(".") || message.hasSuffix(")"), !message.contains("MpPairError") else {
        fail("the pairing error is not a sentence: \(message)")
    }
    print("ok  pairing with nothing listening is Unreached, retryable, and a sentence")
} catch {
    fail("unexpected error type: \(error)")
}

// 6. A constructed MpPaired pins the memberwise initialiser's order, and the
//    hand-written description withholds the key. A real redemption needs a
//    far machine and is the on-device spike's job.
let fake = MpPaired(
    pipe: MpPipe(noHandle: MpPipe.NoHandle()),
    apiKey: "secret-key-0123",
    device: "dev-smoke",
    serving: String(repeating: "0", count: 64)
)
// The placeholder pipe has no Rust handle behind it, so nothing here may call
// into it; the fields are what pin the initialiser's order.
guard fake.device == "dev-smoke", fake.serving.count == 64, fake.apiKey == "secret-key-0123"
else {
    fail("MpPaired's memberwise initialiser has been reordered")
}
for rendering in [String(describing: fake), "\(fake)", String(reflecting: fake)] {
    guard !rendering.contains("secret-key-0123"), rendering.contains("dev-smoke") else {
        fail("a rendering of MpPaired shows the key or hides the device: \(rendering)")
    }
}
print("ok  MpPaired renders without its key")

print("smoke: the binding links and answers across the boundary")
SMOKE_SWIFT

echo "==> Building and running the smoke executable"
cd "${WORK_DIR}"

# Point the dependency's binaryTarget at the framework just built rather than
# at the last published release, which is the whole point of running this
# before publishing anything.
export MODELPIPE_FFI_LOCAL_XCFRAMEWORK=1

# Bounded, and the bound is the point rather than caution.
#
# Every check below is either immediate or fails fast; nothing here waits on a
# network. So the one way this runs long is a failure the script exists to
# find — an async call that never returns because the library's tokio runtime
# has no running workers. Left unbounded that is indistinguishable from a slow
# runner until the job hits its own ceiling with no clue why.
#
# `timeout` exits 124 on expiry, which is caught here so the log names the
# behaviour instead of leaving a bare non-zero to interpret. The last `ok`
# line printed before this says which call hung.
#
# Not available as `timeout` on macOS without coreutils, so fall back to
# running unbounded rather than failing a build over a missing tool — the job
# ceiling still catches it.
if command -v timeout >/dev/null 2>&1; then
    if timeout 300 swift run Smoke; then
        exit 0
    fi
    status=$?
    if [[ "${status}" -eq 124 ]]; then
        echo "error: the smoke executable did not finish within 300s." >&2
        echo "       Nothing here waits on a network, so this is a call that" >&2
        echo "       never returned. The last 'ok' line above names the one" >&2
        echo "       before it." >&2
    fi
    exit "${status}"
fi

echo "note: no \`timeout\` on this machine; the job ceiling is the only bound."
swift run Smoke
