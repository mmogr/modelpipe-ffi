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
#   $4 the deployment floor this build asked for
check() {
    local lib="$1" name="$2" want="$3" want_min="$4"

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

    # Deployment floors, read from BOTH load commands.
    #
    # A modern object carries LC_BUILD_VERSION, whose floor `otool` prints as
    # `minos`. An object built for a low enough target carries the older
    # LC_VERSION_MIN_*, whose floor prints as `version` — a different word for
    # the same fact. Reading only the first hides the second completely.
    #
    # That is not hypothetical, and it is why this reads both. Every slice
    # contains 390 objects from rustup's PRECOMPILED standard library — core,
    # alloc, std, addr2line, compiler_builtins' outline-atomics — which the
    # Rust project built with its own deployment targets and which no
    # environment variable here can move short of `-Z build-std`. On the
    # simulator and macOS slices they read 14.0 and 11.0. On the device slice
    # they read **iOS 10.0**, in the old-style command: the exact value this
    # gate exists to reject, sitting in the shipped v0.1.0 artifact while this
    # script reported `minos 26.0` and passed.
    #
    # The two are therefore asserted differently. The floor we asked for must
    # be present — that is our objects, and their absence is precisely what
    # broke when the deployment target went unset and the link failed on
    # `___chkstk_darwin`. The toolchain's own floor is reported and not
    # failed: it is lower rather than higher, a mixed archive is resolved by
    # the consuming app's target, and it is how every Rust iOS binary is
    # built.
    local built toolchain
    built="$(otool -l "${lib}" 2>/dev/null \
        | awk '/^ *minos /{print $2}' \
        | sort -u \
        | tr '\n' ' ')"
    built="${built% }"
    toolchain="$(otool -l "${lib}" 2>/dev/null \
        | awk '
            /^ *cmd LC_VERSION_MIN_/ { want = 1; next }
            /^ *cmd / { want = 0; next }
            want && /^ *version / { print $2; want = 0 }
        ' \
        | sort -u \
        | tr '\n' ' ')"
    toolchain="${toolchain% }"

    case " ${built} " in
        *" ${want_min} "*) ;;
        *)
            echo "  WRONG    ${name}  [${arches}]  asked for a floor of ${want_min}, compiled ${built:-nothing}"
            fail=1
            ;;
    esac

    local floor
    for floor in ${built}; do
        if [[ "${floor}" == 10.* ]]; then
            echo "  WRONG    ${name}  [${arches}]  compiled an object at ${floor}, which is rustc's default rather than a chosen floor"
            fail=1
        fi
    done

    local minos="${built}"
    [[ -n "${toolchain}" ]] && minos="${built} (toolchain: ${toolchain})"

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

# The floors these must have been built with. Same defaults as
# build-xcframework.sh, and overridden by the same variables, so running this
# on its own asserts what the build asked for rather than a second opinion.
IOS_MIN="${IPHONEOS_DEPLOYMENT_TARGET:-26.0}"
MACOS_MIN="${MACOSX_DEPLOYMENT_TARGET:-26.0}"

check "${FRAMEWORK}/ios-arm64/libmodelpipe_ffi.a" \
    "ios-arm64" "IOS" "${IOS_MIN}"
check "${FRAMEWORK}/ios-arm64-simulator/libmodelpipe_ffi.a" \
    "ios-simulator" "IOSSIMULATOR" "${IOS_MIN}"
check "${FRAMEWORK}/macos-arm64/libmodelpipe_ffi.a" \
    "macos" "MACOS" "${MACOS_MIN}"

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
