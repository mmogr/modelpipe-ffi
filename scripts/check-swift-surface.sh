#!/usr/bin/env bash
#
# Every member the binding exports must be exercised by the Swift smoke test.
#
# THE FAILURE THIS EXISTS FOR
#
# `MpError::is_retryable` was a `pub fn`, documented, and covered by four Rust
# tests. None of that put it across the boundary — the `impl` block had no
# `#[uniffi::export]`, so Swift got a bare enum and `error.isRetryable()` did
# not compile. It was found by hand, in Xcode, on the one build that mattered.
#
# The smoke test could not have caught it, because the smoke test never called
# it. A green Apple job meant "everything I remembered to call still works",
# which is a weaker claim than it looks and gets weaker every time the surface
# grows.
#
# This runs anywhere: the scaffolding describes the API rather than the
# machine, so the binding generates from the HOST library on Linux, seconds
# after a build, with no Xcode anywhere near it.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BINDING="${1:-${ROOT_DIR}/generated/modelpipe_ffi.swift}"
SMOKE="${ROOT_DIR}/scripts/swift-smoke.sh"

if [[ ! -f "${BINDING}" ]]; then
    echo "error: ${BINDING} not found. Run \`make swift\` first." >&2
    exit 1
fi

# Every exported callable and every field of an exported record. `open` is
# what UniFFI emits for methods on an interface, `public` for everything else.
#
# Dropped: UniFFI's own plumbing — the `FfiConverter*` lift/lower pairs and
# the `uniffiEnsure...` initialiser, which are generated for the binding's use
# rather than the app's — and `errorDescription`, which is generated as
# `String(reflecting: self)` and is the thing the smoke test asserts we are
# NOT showing anyone.
mapfile -t members < <(
    grep -oE '^[[:space:]]*(open|public) (func|var) [a-zA-Z_][a-zA-Z0-9_]*' "${BINDING}" \
        | awk '{print $2, $NF}' \
        | grep -vE ' ([Uu]niffi|FfiConverter|errorDescription$)' \
        | sort -u
)

if [[ "${#members[@]}" -eq 0 ]]; then
    echo "error: found no exported members in ${BINDING}." >&2
    echo "       That is a parsing failure in this script, not an empty API." >&2
    exit 1
fi

# Match against the smoke PROGRAM, with its comments removed.
#
# Both narrowings are load-bearing. An earlier version searched the whole
# script and passed `port` and `message` on the strength of the words "binds a
# port" and `fail(_ message:)` in its own prose. A gate that green-lights
# itself on comments is worse than no gate, because it reads as coverage.
#
# The Package.swift heredoc above the program is excluded for the same reason:
# a name appearing in a linker setting is not a call.
program="$(awk "/<<'SMOKE_SWIFT'/{on=1;next} /^SMOKE_SWIFT\$/{on=0} on" "${SMOKE}" \
    | sed 's://.*::')"

missing=()
for entry in "${members[@]}"; do
    kind="${entry%% *}"
    name="${entry#* }"

    # A method has to be CALLED: `name(` or `.name(`. A record's field has to
    # be read or passed: `.name` or `name:`. Spelling a field out at the call
    # site is what pins UniFFI's memberwise initialiser, whose argument order
    # follows declaration order and has already broken this build once.
    if [[ "${kind}" == "func" ]]; then
        pattern="(^|[^A-Za-z0-9_])${name}[[:space:]]*\("
    else
        pattern="\.${name}\b|(^|[^A-Za-z0-9_])${name}:"
    fi

    if ! grep -qE "${pattern}" <<<"${program}"; then
        missing+=("${kind} ${name}")
    fi
done

if [[ "${#missing[@]}" -ne 0 ]]; then
    echo "error: exported but never used by the Swift smoke test:" >&2
    printf '         %s\n' "${missing[@]}" >&2
    echo >&2
    echo "       Something crossing the boundary that CI never calls is" >&2
    echo "       something the consuming app finds out about in Xcode." >&2
    echo "       Add it to scripts/swift-smoke.sh." >&2
    exit 1
fi

echo "All ${#members[@]} exported members are exercised by the smoke test."
