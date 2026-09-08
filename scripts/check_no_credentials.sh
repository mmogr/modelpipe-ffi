#!/usr/bin/env bash
#
# Assert that no credential can cross this boundary, structurally.
#
# The design says the FFI never touches a credential: `modelpipe::connect`
# takes none, the token belongs to whatever HTTP client the app points at the
# base URL, and the serve edge is the only thing that checks it. That is worth
# more as a gate than as a paragraph — a token parameter added "just to pass
# through" would be invisible in review and would put a bearer credential into
# a generated Swift signature, a crash log, and every `Debug` along the way.
#
# Two rules:
#   1. No exported function or method takes a credential-shaped parameter.
#   2. Nothing prints or formats a ticket or a token.
#
# Tests are exempt from rule 1 only: they name these words to assert their
# absence, which is the opposite of the failure being guarded.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC_DIR="${ROOT_DIR}/src"
fail=0

# Rule 1 — a credential-shaped parameter on anything crossing the boundary.
#
# Scoped to `pub fn`/`pub async fn` because a private helper taking a `key` is
# this crate's own business; the boundary is what this protects.
echo "==> No exported function takes a credential"
while IFS= read -r line; do
    [[ -z "${line}" ]] && continue
    echo "  ${line}"
    fail=1
done < <(
    grep -rnE '^\s*pub (async )?fn [a-z_]+\(' "${SRC_DIR}" \
        --include='*.rs' \
        | grep -v '_tests\.rs' \
        | grep -iE '[(,]\s*(token|api_key|apikey|secret|password|credential|bearer)\s*:' \
        || true
)

# Rule 2 — a ticket or token reaching output.
#
# `eprintln!` is permitted in general (status.rs uses one for an unknown
# upstream variant), so this looks for the interpolation rather than the macro.
echo "==> Nothing prints a ticket or a token"
while IFS= read -r line; do
    [[ -z "${line}" ]] && continue
    echo "  ${line}"
    fail=1
done < <(
    grep -rnE '(println!|eprintln!|write!|writeln!|format!|panic!)' "${SRC_DIR}" \
        --include='*.rs' \
        | grep -v '_tests\.rs' \
        | grep -iE '\{[^}]*(ticket|token|secret|api_key|bearer)[^}]*\}' \
        || true
)

if [[ "${fail}" -ne 0 ]]; then
    echo
    echo "error: a credential can reach the boundary or the output above." >&2
    echo "       The FFI takes no token by design: modelpipe's own \`connect\`" >&2
    echo "       has no credential parameter, and the app attaches the bearer" >&2
    echo "       to its HTTP client against the base URL instead." >&2
    exit 1
fi

echo
echo "No credential crosses the boundary, and none reaches output."
