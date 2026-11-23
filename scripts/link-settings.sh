#!/usr/bin/env bash
#
# Write Package.swift's linker settings from what rustc says each slice needs.
#
# Usage: ./scripts/link-settings.sh [--self-test]
#
# THE FAILURE THIS EXISTS FOR
#
# The list used to be typed by hand, "read off rustc's own link invocation",
# once. A static library does not carry its own dependencies, so that list is
# the only thing telling a consumer's linker where the library's system
# symbols live, and nothing held it to the library. 0.4.5 took netdev to
# 0.47.0, which declares `#[link(name = "resolv")]` on iOS; the list did not
# move, and the release did not link on a phone or a simulator.
#
# rustc already knows the answer. Asked to build a static library with
# `--print native-static-libs` it names every library and framework the
# archive's objects were declared to need:
#
#     note: native-static-libs: -lresolv -framework Network -lSystem -lobjc ...
#
# This asks it for each of the three slices and writes the result between two
# marker lines in Package.swift. `make link-settings-check` then fails when
# the committed file is not what this writes, which is how a lock update that
# changes what a slice links is caught in the PR that makes it.
#
# A setting every slice needs is written bare. One that only the iOS slices
# need, or only the macOS slice, carries a platform condition, so a library
# that exists on one platform is not handed to the other's linker. The device
# and the simulator are one platform to SwiftPM and get one list between them.
#
# WHAT THIS DOES NOT COVER
#   A symbol something references without declaring where it lives. rustc
#   lists what crates declare with `#[link]` and what build scripts print,
#   and no more. ios-link.sh is the check for that: it links both iOS slices
#   in a consumer, and swift-smoke.sh links the macOS one.
set -euo pipefail

# The block is sorted, and `sort` orders by the locale it finds. The bytes
# committed must not depend on whose machine wrote them.
export LC_ALL=C

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="${ROOT_DIR}/Package.swift"
BEGIN_MARK="// BEGIN GENERATED LINKER SETTINGS"
END_MARK="// END GENERATED LINKER SETTINGS"
# The indentation of the block inside the manifest's `.target(`.
INDENT="            "

# What SwiftPM passes for every Apple link already. Listing them would not
# break anything; leaving them out keeps the block to what is this library's.
ALREADY_LINKED="System c m"

die() {
    echo "error: $*" >&2
    exit 1
}

# Turn one `native-static-libs` line into settings, one per line, as
# `framework NAME` or `library NAME`, sorted and without repeats.
#
# Anything that is not `-lNAME` or `-framework NAME` fails, and every caller
# stops on that. It returns rather than exits because bash clears `set -e`
# inside a command substitution, where this is always called. rustc can
# also say `-weak_framework`, or pass a path, and a form this does not know
# how to write as a SwiftPM setting must not be dropped on the floor: that is
# a missing link setting with a green check beside it.
parse_libs() {
    local line="$1" expecting_framework=0 token name already skip out=""
    # Unquoted on purpose: word splitting is the parse.
    for token in ${line}; do
        if [[ "${expecting_framework}" -eq 1 ]]; then
            out+="framework ${token}"$'\n'
            expecting_framework=0
            continue
        fi
        case "${token}" in
            -framework)
                expecting_framework=1
                ;;
            -l?*)
                name="${token#-l}"
                skip=0
                for already in ${ALREADY_LINKED}; do
                    [[ "${name}" == "${already}" ]] && skip=1
                done
                [[ "${skip}" -eq 1 ]] || out+="library ${name}"$'\n'
                ;;
            *)
                echo "error: rustc named a link argument this script cannot write as a SwiftPM setting: '${token}' in '${line}'" >&2
                return 1
                ;;
        esac
    done
    if [[ "${expecting_framework}" -ne 0 ]]; then
        echo "error: '-framework' with no name after it in '${line}'" >&2
        return 1
    fi
    printf '%s' "${out}" | sort -u
}

# Render the block from two lists in parse_libs's form: what the iOS slices
# need and what the macOS slice needs.
render() {
    local ios="$1" mac="$2" kind name condition call all
    all="$(printf '%s\n%s\n' "${ios}" "${mac}" | sort -u | sed '/^$/d')"

    echo "${INDENT}${BEGIN_MARK}"
    echo "${INDENT}// Written by scripts/link-settings.sh from rustc's native-static-libs."
    echo "${INDENT}// Do not edit: run \`make link-settings\`."
    echo "${INDENT}linkerSettings: ["
    while read -r kind name; do
        [[ -n "${kind}" ]] || continue
        condition=""
        if ! grep -qxF "${kind} ${name}" <<<"${mac}"; then
            condition=", .when(platforms: [.iOS])"
        elif ! grep -qxF "${kind} ${name}" <<<"${ios}"; then
            condition=", .when(platforms: [.macOS])"
        fi
        case "${kind}" in
            framework) call="linkedFramework" ;;
            library)   call="linkedLibrary" ;;
            *)         die "unknown kind '${kind}'" ;;
        esac
        echo "${INDENT}    .${call}(\"${name}\"${condition}),"
    done <<<"${all}"
    echo "${INDENT}]"
    echo "${INDENT}${END_MARK}"
}

# The parse and the rendering, held to fixed input. Runs before every real
# run, so the awk, sort and grep of whichever machine this is on are held to
# the same output, and alone with --self-test.
self_test() {
    local ios mac got want

    ios="$(parse_libs "-lresolv -framework Network -lSystem -lobjc -framework Foundation -liconv -lSystem -lc -lm")"
    want="$(printf 'framework Foundation\nframework Network\nlibrary iconv\nlibrary objc\nlibrary resolv')"
    [[ "${ios}" == "${want}" ]] || die "self-test: parse gave:"$'\n'"${ios}"

    mac="$(parse_libs "-framework Foundation -framework AppKit -lSystem -lobjc -liconv -lc -lm")"
    got="$(render "${ios}" "${mac}")"
    want="${INDENT}${BEGIN_MARK}
${INDENT}// Written by scripts/link-settings.sh from rustc's native-static-libs.
${INDENT}// Do not edit: run \`make link-settings\`.
${INDENT}linkerSettings: [
${INDENT}    .linkedFramework(\"AppKit\", .when(platforms: [.macOS])),
${INDENT}    .linkedFramework(\"Foundation\"),
${INDENT}    .linkedFramework(\"Network\", .when(platforms: [.iOS])),
${INDENT}    .linkedLibrary(\"iconv\"),
${INDENT}    .linkedLibrary(\"objc\"),
${INDENT}    .linkedLibrary(\"resolv\", .when(platforms: [.iOS])),
${INDENT}]
${INDENT}${END_MARK}"
    [[ "${got}" == "${want}" ]] || die "self-test: render gave:"$'\n'"${got}"

    # A form it cannot write must stop it, in a subshell so the exit is seen
    # rather than taken.
    if (parse_libs "-lobjc -weak_framework CoreNFC" >/dev/null 2>&1); then
        die "self-test: an argument it cannot write was let through"
    fi
}

self_test
if [[ "${1:-}" == "--self-test" ]]; then
    echo "link-settings: the self-test passes"
    exit 0
fi
[[ $# -eq 0 ]] || die "usage: link-settings.sh [--self-test]"

if [[ "$(uname -s)" != "Darwin" ]]; then
    die "this builds the Apple slices, so it needs Xcode and only runs on macOS."
fi

for mark in "${BEGIN_MARK}" "${END_MARK}"; do
    count="$(grep -cF "${mark}" "${MANIFEST}" || true)"
    [[ "${count}" -eq 1 ]] || die "Package.swift should hold '${mark}' once, and holds it ${count} times"
done

cd "${ROOT_DIR}"

# The deployment targets build-xcframework.sh sets, for its reasons, and so
# that this reuses the dependencies that build compiled rather than building
# every C dependency again for another floor.
export IPHONEOS_DEPLOYMENT_TARGET="${IPHONEOS_DEPLOYMENT_TARGET:-26.0}"
export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-26.0}"

# Unoptimised by default: what a slice links is decided by the lock, not by
# the optimiser, and `make xcframework-fast` has usually just built these.
PROFILE="${PROFILE:-dev}"
CARGO="${CARGO:-cargo}"

# Ask rustc what one slice needs.
#
# `--crate-type staticlib` because that is the artifact a consumer links, and
# the only one rustc prints the note for; without it cargo would also link the
# cdylib for a platform nothing loads it on.
#
# No answer is a failure, never an empty list. cargo exiting zero while this
# reads nothing would otherwise write a manifest with no linker settings and
# call it generated.
native_libs() {
    local triple="$1" out line
    # shellcheck disable=SC2086  # CARGO may be `rustup run <toolchain> cargo`
    if ! out="$(${CARGO} rustc --locked --lib --profile "${PROFILE}" --target "${triple}" \
        --crate-type staticlib -- --print native-static-libs 2>&1)"; then
        printf '%s\n' "${out}" >&2
        die "the build for ${triple} failed"
    fi
    line="$(grep -E 'native-static-libs: ' <<<"${out}" | tail -1 || true)"
    line="${line#*native-static-libs: }"
    if [[ -z "${line}" ]]; then
        printf '%s\n' "${out}" | tail -20 >&2
        die "rustc printed no native-static-libs note for ${triple}"
    fi
    echo "${line}"
}

echo "==> Asking rustc what each slice links (profile ${PROFILE})"
device="$(native_libs aarch64-apple-ios)"
echo "    aarch64-apple-ios      ${device}"
simulator="$(native_libs aarch64-apple-ios-sim)"
echo "    aarch64-apple-ios-sim  ${simulator}"
macos="$(native_libs aarch64-apple-darwin)"
echo "    aarch64-apple-darwin   ${macos}"

# One assignment per parse: a failure inside a nested substitution is lost.
device_settings="$(parse_libs "${device}")"
simulator_settings="$(parse_libs "${simulator}")"
mac_settings="$(parse_libs "${macos}")"
ios_settings="$(printf '%s\n%s\n' "${device_settings}" "${simulator_settings}" | sort -u | sed '/^$/d')"
[[ -n "${ios_settings}" ]] || die "the iOS slices need nothing at all, which no build of this library has ever said"
[[ -n "${mac_settings}" ]] || die "the macOS slice needs nothing at all, which no build of this library has ever said"

block="$(render "${ios_settings}" "${mac_settings}")"

# Everything outside the markers is kept byte for byte. Written beside the
# manifest and moved over it, so a run that stops halfway leaves the manifest
# as it was.
tmp="$(mktemp "${MANIFEST}.XXXXXX")"
trap 'rm -f "${tmp}"' EXIT
BLOCK="${block}" awk -v begin="${BEGIN_MARK}" -v end="${END_MARK}" '
    index($0, begin) { print ENVIRON["BLOCK"]; inside = 1; next }
    index($0, end)   { inside = 0; next }
    !inside          { print }
' "${MANIFEST}" > "${tmp}"
mv "${tmp}" "${MANIFEST}"
trap - EXIT

echo "link-settings: Package.swift lists what the three slices link"
