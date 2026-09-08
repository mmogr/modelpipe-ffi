#!/usr/bin/env bash
#
# Assert every slice in an XCFramework is genuinely the platform it claims.
#
# The failure this exists for: a cross-compile that succeeds while producing
# objects for the host. It is invisible to `cargo build`, invisible to
# `xcodebuild -create-xcframework`, and shows up as a link failure in whatever
# project consumes the framework — somewhere else, later, to someone who did
# not build it.
#
# modelpipe#45's comment established the check by hand on 2026-09-06, reading
# `LC_VERSION_MIN_IPHONEOS` on the device build and `platform IOSSIMULATOR` on
# the simulator one. This is that reading, kept as a gate so it holds for every
# build rather than the one somebody remembered to look at.
set -euo pipefail

FRAMEWORK="${1:?usage: check-slices.sh <path to .xcframework>}"

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "error: needs otool and lipo, so macOS only." >&2
    exit 1
fi

fail=0

# Report a slice, then assert one fact about it.
#   $1 the library inside the framework
#   $2 a human name for the slice
#   $3 the platform string `otool -l` must show
check() {
    local lib="$1" name="$2" want="$3"

    if [[ ! -f "${lib}" ]]; then
        echo "  MISSING  ${name}: ${lib}"
        fail=1
        return
    fi

    local arches
    arches="$(lipo -archs "${lib}")"

    # `otool -l` on a static library prints the load commands of every member.
    # The platform is uniform across them, so one grep answers for the slice.
    local platforms
    platforms="$(otool -l "${lib}" 2>/dev/null \
        | awk '/^ *platform /{print $2}' \
        | sort -u \
        | tr '\n' ' ')"

    if [[ "${platforms}" == *"${want}"* ]]; then
        echo "  ok       ${name}  [${arches}]  platform ${platforms}"
    else
        echo "  WRONG    ${name}  [${arches}]  platform '${platforms}', wanted '${want}'"
        fail=1
    fi
}

echo "Checking slices in ${FRAMEWORK}"

check "${FRAMEWORK}/ios-arm64/libmodelpipe_ffi.a" \
    "ios-arm64" "IOS"
check "${FRAMEWORK}/ios-arm64_x86_64-simulator/libmodelpipe_ffi.a" \
    "ios-simulator" "IOSSIMULATOR"
check "${FRAMEWORK}/macos-arm64_x86_64/libmodelpipe_ffi.a" \
    "macos" "MACOS"

# The device slice is the one a mistake is most expensive in, so its
# architecture is asserted too: an arm64e or x86_64 device slice would install
# and then fail to launch.
device="${FRAMEWORK}/ios-arm64/libmodelpipe_ffi.a"
if [[ -f "${device}" ]] && [[ "$(lipo -archs "${device}")" != "arm64" ]]; then
    echo "  WRONG    ios-arm64 carries $(lipo -archs "${device}"), wanted exactly arm64"
    fail=1
fi

if [[ "${fail}" -ne 0 ]]; then
    echo
    echo "error: at least one slice is not the platform it claims." >&2
    echo "       A build that ships this fails at link time in the consuming app." >&2
    exit 1
fi

echo "All slices carry the platform they claim."
