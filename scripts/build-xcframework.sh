#!/usr/bin/env bash
#
# Build the XCFramework ggchat consumes.
#
# Five slices in three bundles, because that is what Apple's packaging can
# express: a device library, a simulator library fat across two architectures,
# and a macOS library fat across two more. A single `lipo` of device and
# simulator arm64 is not a thing — same architecture, different platform, and
# `lipo` has no way to say so. The XCFramework is the format that does.
#
#   ios-arm64                     iPhone, the one that matters
#   ios-arm64_x86_64-simulator    Simulator on Apple silicon and on Intel
#   macos-arm64_x86_64            The Mac app
#
# Everything here needs Xcode, so it runs on macOS only. CI runs it on
# macos-26; there is no Linux path and no attempt at one.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

BUILD_DIR="${ROOT_DIR}/build"
GENERATED_DIR="${ROOT_DIR}/generated"
FRAMEWORK="${BUILD_DIR}/ModelpipeFFI.xcframework"
LIB_NAME="libmodelpipe_ffi.a"
# `PROFILE` is the cargo profile name; the output directory is not always the
# same word. Cargo's debug profile is called `dev` and lands in `target/*/debug`,
# which `--profile debug` does not even accept. Getting this wrong produces a
# script that works in release and fails in debug with a path that does not
# exist, so the two are separate variables rather than one string used twice.
PROFILE="${PROFILE:-release}"
if [[ "${PROFILE}" == "dev" || "${PROFILE}" == "debug" ]]; then
    PROFILE="dev"
    PROFILE_DIR="debug"
else
    PROFILE_DIR="${PROFILE}"
fi

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "error: this needs Xcode, so it only runs on macOS." >&2
    echo "       The crate itself builds anywhere: \`cargo test\` is the check to run here." >&2
    exit 1
fi

# Deployment targets, and why they are set at all.
#
# rustc's `aarch64-apple-ios` target still defaults to a deployment target of
# **iOS 10.0**, while the C and assembly in the dependency tree — blake3's
# NEON, ring's — are compiled by `cc` against whatever SDK is installed. That
# mismatch is not cosmetic. `___chkstk_darwin` arrived in iOS 13 and does not
# exist in an iOS 10 runtime, so linking the cdylib fails outright:
#
#     Undefined symbols for architecture arm64:
#       "___chkstk_darwin", referenced from:
#           _blake3_hash4_neon in libblake3...
#
# It hid in release, where optimisation drops the NEON path so the symbol is
# never referenced, and appeared the moment CI started building unoptimised.
# The bug was there the whole time.
#
# 26.0 rather than the 13.0 that would merely make it link: ggchat declares
# `platforms: [.iOS(.v26), .macOS(.v26)]`, so a library with a lower floor
# buys reach no consumer of this can use, and a floor *above* the consumer's
# would be a link error in their project rather than ours. Matching is the
# only setting that cannot be wrong. Overridable, for anyone whose app is not
# ggchat.
export IPHONEOS_DEPLOYMENT_TARGET="${IPHONEOS_DEPLOYMENT_TARGET:-26.0}"
export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-26.0}"

echo "==> Building five slices (profile ${PROFILE})"
echo "    iOS ${IPHONEOS_DEPLOYMENT_TARGET}, macOS ${MACOSX_DEPLOYMENT_TARGET}"
TARGETS=(
    aarch64-apple-ios
    aarch64-apple-ios-sim
    x86_64-apple-ios
    aarch64-apple-darwin
    x86_64-apple-darwin
)
# One cargo invocation with five `--target` flags, not five invocations.
#
# Sequential invocations are not merely tidier-looking; they are slower for a
# specific reason. Each target's build graph has a long narrow tail — the
# final few crates, then the link — where the dependency graph has collapsed
# to one or two units and most cores sit idle. Five builds in a row means
# paying that tail five times. One build plan spanning all five lets cargo
# start the next target's wide base while the previous one's tail finishes,
# so the idle cores get filled.
#
# It cannot be done by backgrounding five `cargo build` calls: cargo takes an
# exclusive lock on the target directory, so concurrent invocations block on
# each other and the result is the sequential version plus lock contention.
# Multiple `--target` flags in ONE invocation is the supported way, stable
# since 1.64.
#
# Output paths are unchanged — each target still lands in
# `target/<triple>/<profile>/`.
printf '    %s\n' "${TARGETS[@]}"
cargo build --lib --profile "${PROFILE}" "${TARGETS[@]/#/--target=}"

echo "==> Fattening the two multi-architecture slices"
rm -rf "${BUILD_DIR}"
mkdir -p "${BUILD_DIR}/ios-sim" "${BUILD_DIR}/macos"

lipo -create \
    "target/aarch64-apple-ios-sim/${PROFILE_DIR}/${LIB_NAME}" \
    "target/x86_64-apple-ios/${PROFILE_DIR}/${LIB_NAME}" \
    -output "${BUILD_DIR}/ios-sim/${LIB_NAME}"

lipo -create \
    "target/aarch64-apple-darwin/${PROFILE_DIR}/${LIB_NAME}" \
    "target/x86_64-apple-darwin/${PROFILE_DIR}/${LIB_NAME}" \
    -output "${BUILD_DIR}/macos/${LIB_NAME}"

echo "==> Generating the Swift binding and its headers"
# Read out of a built library rather than a UDL file: the scaffolding compiled
# into the library is the only description of it that cannot drift from it.
#
# Read out of the HOST library specifically. The scaffolding is identical
# across targets — it describes the API, not the machine — and the host build
# is the one `uniffi-bindgen` can always load. Pointing this at the iOS dylib
# instead makes the generator's ability to parse a foreign-platform binary a
# load-bearing assumption, for no benefit.
rm -rf "${GENERATED_DIR}"
mkdir -p "${GENERATED_DIR}"
cargo build --lib --profile "${PROFILE}"
cargo run --bin uniffi-bindgen -- generate \
    --library "target/${PROFILE_DIR}/libmodelpipe_ffi.dylib" \
    --language swift \
    --out-dir "${GENERATED_DIR}"

# Xcode wants the modulemap under this exact name, and wants the header
# alongside it. UniFFI emits `<name>FFI.modulemap`; renaming is the whole of
# the adaptation.
HEADERS_DIR="${BUILD_DIR}/headers"
mkdir -p "${HEADERS_DIR}"
cp "${GENERATED_DIR}"/*.h "${HEADERS_DIR}/"
cp "${GENERATED_DIR}"/*.modulemap "${HEADERS_DIR}/module.modulemap"

echo "==> Assembling the XCFramework"
rm -rf "${FRAMEWORK}"
xcodebuild -create-xcframework \
    -library "target/aarch64-apple-ios/${PROFILE_DIR}/${LIB_NAME}" -headers "${HEADERS_DIR}" \
    -library "${BUILD_DIR}/ios-sim/${LIB_NAME}" -headers "${HEADERS_DIR}" \
    -library "${BUILD_DIR}/macos/${LIB_NAME}" -headers "${HEADERS_DIR}" \
    -output "${FRAMEWORK}"

echo "==> Checking each slice is what it claims to be"
# Not ceremony. A cross-compile that silently produced host objects is the
# failure mode modelpipe#45 went looking for, and the only thing that tells
# the difference is the load command inside the object. A build that says
# "succeeded" while linking a Linux or macOS object into the iOS slice would
# fail at `import` time in Xcode, a week later, in someone else's project.
"${ROOT_DIR}/scripts/check-slices.sh" "${FRAMEWORK}"

echo
echo "Built ${FRAMEWORK}"
