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

echo "==> Building five slices (profile ${PROFILE})"
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
