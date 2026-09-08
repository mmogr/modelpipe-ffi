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

# Apple's platform constants, from <mach-o/loader.h>. `otool -l` prints the
# NUMBER, not the name — which is the whole reason this table exists. The
# first version of this script grepped for "IOS" and duly failed a perfectly
# good build three ways, reporting `platform '2 '` as wrong when 2 is exactly
# what an iOS slice should say.
platform_name() {
    case "$1" in
        1)  echo "MACOS" ;;
        2)  echo "IOS" ;;
        3)  echo "TVOS" ;;
        4)  echo "WATCHOS" ;;
        5)  echo "BRIDGEOS" ;;
        6)  echo "MACCATALYST" ;;
        7)  echo "IOSSIMULATOR" ;;
        8)  echo "TVOSSIMULATOR" ;;
        9)  echo "WATCHOSSIMULATOR" ;;
        10) echo "DRIVERKIT" ;;
        # Some toolchains print the name directly. Pass anything
        # non-numeric through rather than mangling it.
        *)  echo "$1" ;;
    esac
}

# Report a slice, then assert one fact about it.
#   $1 the library inside the framework
#   $2 a human name for the slice
#   $3 the platform name the slice must carry
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
    # The platform is uniform across them, so one reading answers for the
    # slice. Every distinct value is resolved to a name and collected, so a
    # slice that somehow mixed two platforms reports both rather than the
    # first.
    local raw names=""
    for raw in $(otool -l "${lib}" 2>/dev/null \
        | awk '/^ *platform /{print $2}' \
        | sort -u); do
        names="${names}$(platform_name "${raw}") "
    done
    names="${names% }"

    # The deployment target the slice was actually built with. Reported
    # always, and failed only on the one value known to be broken: rustc's
    # apple targets default to iOS 10.0, which cannot resolve
    # `___chkstk_darwin` and so fails to link against a modern SDK's C
    # objects. Asserting an exact value here would make this gate fail on
    # any legitimate bump; asserting against the known-bad default cannot.
    local minos
    minos="$(otool -l "${lib}" 2>/dev/null \
        | awk '/^ *minos /{print $2}' \
        | sort -u \
        | tr '\n' ' ')"
    minos="${minos% }"

    if [[ "${minos}" == 10.* ]]; then
        echo "  WRONG    ${name}  [${arches}]  minos ${minos} is rustc's default, not a chosen floor"
        fail=1
    fi

    if [[ -z "${names}" ]]; then
        echo "  NO DATA  ${name}  [${arches}]  otool reported no platform load command"
        fail=1
    elif [[ "${names}" == "${want}" ]]; then
        echo "  ok       ${name}  [${arches}]  platform ${names}  minos ${minos:-?}"
    else
        echo "  WRONG    ${name}  [${arches}]  platform '${names}', wanted '${want}'"
        fail=1
    fi
}

echo "Checking slices in ${FRAMEWORK}"

check "${FRAMEWORK}/ios-arm64/libmodelpipe_ffi.a" \
    "ios-arm64" "IOS"
check "${FRAMEWORK}/ios-arm64-simulator/libmodelpipe_ffi.a" \
    "ios-simulator" "IOSSIMULATOR"
check "${FRAMEWORK}/macos-arm64/libmodelpipe_ffi.a" \
    "macos" "MACOS"

# Every bundle is single-architecture now that the x86_64 targets are gone, so
# each one is asserted to carry exactly `arm64` and nothing else. A fat slice
# reappearing here means a target crept back into the build without the
# framework layout being updated to match — which `-create-xcframework` would
# accept silently, renaming the bundle underneath the checks above.
for slice in ios-arm64 ios-arm64-simulator macos-arm64; do
    lib="${FRAMEWORK}/${slice}/libmodelpipe_ffi.a"
    [[ -f "${lib}" ]] || continue
    archs="$(lipo -archs "${lib}")"
    if [[ "${archs}" != "arm64" ]]; then
        echo "  WRONG    ${slice} carries '${archs}', wanted exactly arm64"
        fail=1
    fi
done

if [[ "${fail}" -ne 0 ]]; then
    echo
    echo "error: at least one slice is not the platform it claims." >&2
    echo "       A build that ships this fails at link time in the consuming app." >&2
    exit 1
fi

echo "All slices carry the platform they claim."
