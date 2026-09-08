# Security

## What this library handles

A **ticket**, which is a bearer credential: anyone holding one can dial the
machine it names, for as long as that machine is serving. It is the only
secret-shaped value that reaches this library.

It handles **no bearer token**. `mpConnect` takes none, because
`modelpipe::connect` takes none: the connecting side forwards `Authorization`
verbatim and the serve edge is the only thing that checks it. The token belongs
to whatever HTTP client the app points at `baseUrl()`.

## What is asserted rather than promised

- No exported function takes a credential-shaped parameter, and nothing
  formats a ticket or a token into output. `scripts/check_no_credentials.sh`
  fails the build otherwise, and runs in CI.
- No error variant renders a ticket, asserted by
  `no_error_renders_the_ticket`. Errors are the values most likely to reach a
  log, a crash report or a screenshot.
- `Debug` on the pipe object is hand-written and carries a port and a status,
  nothing else.

## What this library does not defend against

A device that is already compromised. The ticket and the token live in the
consuming app's Keychain, and this library holds the ticket in memory for the
life of a dial. Neither is defensible against code running as the app.

## Reporting

Open a
[security advisory](https://github.com/mmogr/modelpipe-ffi/security/advisories/new).
Please do not open a public issue for a vulnerability, and do not include a
real ticket in a report — a fingerprint or a redacted form is enough.

For the transport's own security model, see
[modelpipe's SECURITY.md](https://github.com/mmogr/modelpipe/blob/main/SECURITY.md).
