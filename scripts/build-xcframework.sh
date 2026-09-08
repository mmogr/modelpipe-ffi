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
PROFILE="${PROFILE:-release}"

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "error: this needs Xcode, so it only runs on macOS." >&2
    echo "       The crate itself builds anywhere: \`cargo test\` is the check to run here." >&2
    exit 1
fi

echo "==> Building five slices (${PROFILE})"
TARGETS=(
    aarch64-apple-ios
    aarch64-apple-ios-sim
    x86_64-apple-ios
    aarch64-apple-darwin
    x86_64-apple-darwin
)
for target in "${TARGETS[@]}"; do
    echo "    ${target}"
    cargo build --lib --profile "${PROFILE}" --target "${target}"
done

echo "==> Fattening the two multi-architecture slices"
rm -rf "${BUILD_DIR}"
mkdir -p "${BUILD_DIR}/ios-sim" "${BUILD_DIR}/macos"

lipo -create \
    "target/aarch64-apple-ios-sim/${PROFILE}/${LIB_NAME}" \
    "target/x86_64-apple-ios/${PROFILE}/${LIB_NAME}" \
    -output "${BUILD_DIR}/ios-sim/${LIB_NAME}"

lipo -create \
    "target/aarch64-apple-darwin/${PROFILE}/${LIB_NAME}" \
    "target/x86_64-apple-darwin/${PROFILE}/${LIB_NAME}" \
    -output "${BUILD_DIR}/macos/${LIB_NAME}"

echo "==> Generating the Swift binding and its headers"
# Read out of the dylib rather than a UDL file: the scaffolding compiled into
# the library is the only description of it that cannot drift from the library.
rm -rf "${GENERATED_DIR}"
mkdir -p "${GENERATED_DIR}"
cargo run --bin uniffi-bindgen -- generate \
    --library "target/aarch64-apple-ios/${PROFILE}/libmodelpipe_ffi.dylib" \
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
    -library "target/aarch64-apple-ios/${PROFILE}/${LIB_NAME}" -headers "${HEADERS_DIR}" \
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
