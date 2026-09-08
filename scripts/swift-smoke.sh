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
#   - an ASYNC call returns, which is the one that fails if the library's
#     tokio runtime was never started: that shows up as a hang, not an error,
#     and nothing else in CI would catch it
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

# The generated Swift is an output of the framework build, not a checked-in
# file, so regenerate rather than assume it is lying around.
if [[ ! -f "${ROOT_DIR}/generated/modelpipe_ffi.swift" ]]; then
    echo "error: generated/modelpipe_ffi.swift not found. Run \`make xcframework\` first." >&2
    exit 1
fi

rm -rf "${WORK_DIR}"
mkdir -p "${WORK_DIR}/Sources/Smoke"

cp "${ROOT_DIR}/generated/modelpipe_ffi.swift" "${WORK_DIR}/Sources/Smoke/"

cat > "${WORK_DIR}/Package.swift" <<'SWIFT'
// swift-tools-version: 6.2
import PackageDescription

// Matches the consuming app's settings on the two that matter: Swift 6
// language mode and complete strict concurrency. Generated code that is not
// `Sendable`-clean fails here rather than in ggchat.
//
// THE LINKER SETTINGS ARE NOT OPTIONAL, AND ggchat WILL NEED THE SAME ONES.
//
// A static library does not carry its own dependencies. When rustc links a
// binary it passes the system frameworks itself; a `.a` handed to someone
// else records that it *references* those symbols and nothing about where
// they live. Omit them and the build gets all the way to the last step:
//
//     __RNvMs_...system_configuration...SCNetworkInterfaceType13from_cfstring
//         in libmodelpipe_ffi.a[arm64]
//     ld: symbol(s) not found for architecture arm64
//
// This list is read off rustc's own link invocation for the iOS target, minus
// the ones SwiftPM already passes (System, c, m).
let package = Package(
    name: "Smoke",
    platforms: [.macOS(.v14)],
    targets: [
        .binaryTarget(name: "ModelpipeFFI", path: "ModelpipeFFI.xcframework"),
        .executableTarget(
            name: "Smoke",
            dependencies: ["ModelpipeFFI"],
            linkerSettings: [
                // iroh's transport: interface enumeration and reachability.
                .linkedFramework("SystemConfiguration"),
                // rustls-platform-verifier, via security-framework — the
                // Apple trust store, which is why there is no bundled CA set.
                .linkedFramework("Security"),
                .linkedFramework("Network"),
                .linkedFramework("CoreFoundation"),
                .linkedFramework("Foundation"),
                // objc2's runtime calls, and iconv from the C dependencies.
                .linkedLibrary("objc"),
                .linkedLibrary("iconv"),
            ]
        ),
    ],
    swiftLanguageModes: [.v6]
)
SWIFT

cat > "${WORK_DIR}/Sources/Smoke/main.swift" <<'SWIFT'
import Foundation

// modelpipe's normative ticket vector 1. Well-formed, and names an endpoint
// nothing is listening on — so the dial binds a port and then sits at `idle`,
// which is exactly the contract: `connect` returns once the LISTENER is up,
// not once the far machine answers.
let ticket = "pipeadlvvgabqkyqvn6vjp7nhslea45a5yls6pnkmizfv4bbu2hxa5iruaaauhlp2na"

func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data("smoke: \(message)\n".utf8))
    exit(1)
}

// 1. A refusal crosses the boundary as a typed Swift error.
do {
    _ = try await mpConnect(ticket: "nope", options: MpConnectOptions())
    fail("a malformed ticket was accepted")
} catch let error as MpError {
    print("ok  a bad ticket is refused: \(error)")
} catch {
    fail("unexpected error type: \(error)")
}

// 2. A real dial binds, and the sync accessors return.
let pipe = try await mpConnect(
    ticket: ticket,
    options: MpConnectOptions(portMapping: false, discovery: false)
)

let base = pipe.baseUrl()
guard base.hasPrefix("http://127.0.0.1:"), base.hasSuffix("/v1") else {
    fail("base URL is not a loopback /v1 URL: \(base)")
}
print("ok  bound \(base)")

guard pipe.status() == .idle else {
    fail("a pipe with nothing at the far end should be idle, got \(pipe.status())")
}
print("ok  status is idle")

guard pipe.closeReason() == nil else {
    fail("an open pipe reported a close reason")
}

_ = pipe.networkMetrics()
print("ok  metrics read")

// 3. The async path. This is the one that hangs rather than errors if the
//    library's runtime was never started, so it is the reason this script
//    exists at all.
await pipe.notifyNetworkChange()
print("ok  async notifyNetworkChange returned")

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

print("smoke: the binding links and answers across the boundary")
SWIFT

cp -R "${FRAMEWORK}" "${WORK_DIR}/ModelpipeFFI.xcframework"

echo "==> Building and running the smoke executable"
cd "${WORK_DIR}"
swift run Smoke
