#!/usr/bin/env bash
#
# Prove the two iOS slices link in a consumer that declares no linker
# settings of its own.
#
# THE FAILURE THIS EXISTS FOR
#
# 0.4.5 was a lock update. It took netdev to 0.47.0, which reads the DNS
# servers on iOS through libresolv, and `Package.swift` did not list `resolv`.
# Every check in this repository went green and the release was published,
# because every one of them linked for macOS: the smoke test in CI, the smoke
# test in the release, and the consumer the release synthesises afterwards.
# macOS takes a different path through netdev and never needed the library.
# The first iOS link was ggchat's, a day later:
#
#     Undefined symbols for architecture arm64:
#       "_res_9_getservers", referenced from:
#           ...netdev..os..ios..interface..interfaces in libmodelpipe_ffi.a
#     ld: symbol(s) not found for architecture arm64
#
# A static library does not carry its own dependencies, so the only proof
# that the package's linker settings are enough for a slice is a link of that
# slice. The framework holds three and the smoke test links one.
#
# WHAT THIS PROVES
#   For the device slice and the simulator slice: an executable that depends
#   on this package, and on nothing else, links. The binding is compiled into
#   it whole and names every exported function, so every object the library
#   can reach through its API is pulled out of the archive and every symbol
#   those objects reference has to be found.
#
# WHAT THIS DOES NOT PROVE
#   That anything runs. Nothing here is launched: no simulator is booted and
#   no phone is attached. swift-smoke.sh is the check that calls across the
#   boundary, on macOS, and ggchat on a phone is the check that it works.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FRAMEWORK="${ROOT_DIR}/build/ModelpipeFFI.xcframework"
WORK_DIR="${ROOT_DIR}/build/ios-link"

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "error: this links for iOS, so it needs Xcode and only runs on macOS." >&2
    exit 1
fi

if [[ ! -d "${FRAMEWORK}" ]]; then
    echo "error: ${FRAMEWORK} not found. Run \`make xcframework\` first." >&2
    exit 1
fi

if [[ ! -f "${ROOT_DIR}/Sources/Modelpipe/modelpipe_ffi.swift" ]]; then
    echo "error: Sources/Modelpipe/modelpipe_ffi.swift not found. Run \`make swift\` first." >&2
    exit 1
fi

rm -rf "${WORK_DIR}"
mkdir -p "${WORK_DIR}/Sources/IOSLink"

# Depends on the repository's own manifest, as the smoke test does and for the
# smoke test's reason: a copy of the linker settings in here would be the
# thing under test, and the real ones could be wrong in any way at all.
cat > "${WORK_DIR}/Package.swift" <<'SWIFT'
// swift-tools-version: 6.2
import PackageDescription

let package = Package(
    name: "IOSLink",
    platforms: [.iOS(.v26)],
    dependencies: [.package(path: "../..")],
    targets: [
        .executableTarget(
            name: "IOSLink",
            dependencies: [.product(name: "Modelpipe", package: "modelpipe-ffi")]
        )
    ],
    swiftLanguageModes: [.v6]
)
SWIFT

# One call is enough to make the dependency real. What pulls the library in
# is the binding, which is linked whole.
cat > "${WORK_DIR}/Sources/IOSLink/main.swift" <<'SWIFT'
import Modelpipe

_ = try? mpReadPairing(pairing: "nope-12345")
SWIFT

# The framework just built, not the last one published.
export MODELPIPE_FFI_LOCAL_XCFRAMEWORK=1

cd "${WORK_DIR}"

# `generic/` destinations: a link needs an SDK, not a device, so nothing is
# booted and the runner image's list of simulators does not matter.
#
# ARCHS because a generic simulator build asks for x86_64 as well, and the
# framework has had no Intel slice since the two x86_64 targets were dropped;
# ggchat's project sets the same. Signing is off because nothing here is
# installed anywhere.
link() {
    local name="$1" destination="$2" log="${WORK_DIR}/$3.log"

    echo "==> Linking a consumer for ${name}"
    if xcodebuild build \
        -scheme IOSLink \
        -destination "${destination}" \
        -derivedDataPath "${WORK_DIR}/DerivedData" \
        ARCHS=arm64 ONLY_ACTIVE_ARCH=NO \
        CODE_SIGNING_ALLOWED=NO \
        > "${log}" 2>&1; then
        echo "ok  ${name} links"
        return 0
    fi

    # The whole log is thousands of lines of compile commands. What says why
    # a link failed is the linker's own few lines, so those are what is shown;
    # a failure that is not a link gets its error lines and the log's tail.
    echo "error: a consumer of this package does not link for ${name}." >&2
    if grep -q '^Undefined symbols' "${log}"; then
        sed -n '/^Undefined symbols/,/^ld: /p' "${log}" | head -60 >&2
    else
        grep -E 'error: ' "${log}" | sort -u | head -20 >&2 || true
        echo "       --- the end of ${log}:" >&2
        tail -15 "${log}" >&2
    fi
    echo >&2
    echo "       If a symbol is undefined, the library needs a system library or" >&2
    echo "       framework that Package.swift does not list. \`make link-settings\`" >&2
    echo "       writes the list from what rustc says each slice needs." >&2
    return 1
}

status=0
link "the iPhone" "generic/platform=iOS" device || status=1
link "the simulator" "generic/platform=iOS Simulator" simulator || status=1

if [[ "${status}" -ne 0 ]]; then
    exit "${status}"
fi
echo "ios-link: both iOS slices link in a consumer that declares no linker settings"
