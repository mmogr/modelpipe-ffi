# Security

## What this library handles

A **ticket**, which is a bearer credential: anyone holding one can dial the
machine it names, for as long as that machine is serving. A **pairing code**,
which arrives inside a pairing string, is spent on one redemption, and is never
rendered by an error. And a **device key** leaves: `mpPair` returns the key the
far machine minted, once, as `MpPaired.apiKey`, and this library does not keep
it.

It accepts **no bearer token**. `mpConnect` takes none, because
`modelpipe::connect` takes none: the connecting side forwards `Authorization`
verbatim and the serve edge is the only thing that checks it. The token belongs
to whatever HTTP client the app points at `baseUrl()`; after a pairing, that is
the key `mpPair` returned.

An **identity file**, when `identityPath` is set: this device's endpoint key,
written readable only by this user and refused when others can read it, as
modelpipe's serve side does with its own. On Windows there is no mode to set,
so the directory is the only protection. Deleting it changes the fingerprint
the far machine records for this device.

## What is asserted rather than promised

- No exported function takes a credential-shaped parameter, and nothing
  formats a ticket or a token into output. `scripts/check_no_credentials.sh`
  fails the build otherwise, and runs in CI. `mpPair` passes it unchanged: a
  returned field is neither.
- No error variant renders a ticket or a pairing code, asserted by
  `no_error_renders_the_ticket` and `no_pair_error_renders_the_code_or_the_key`;
  none carries a key, so none can render one. Errors are the values most
  likely to reach a log, a crash report or a screenshot.
- `Debug` on the pipe object is hand-written and carries a port and a status,
  nothing else. `Debug` on `MpPaired` is hand-written and withholds the key,
  asserted by `the_debug_of_a_paired_device_never_shows_its_key`; the Swift
  `description` and `debugDescription` of `MpPaired` are a hand-written
  extension beside the generated file that does the same, asserted by the
  smoke test.

## What is not covered

`Mirror`, `dump()`, a direct read of `MpPaired.apiKey`, and any consumer code
that logs the field. The key is a plain `String` the app owns from the moment
`mpPair` returns; keeping it in the Keychain is the app's job, as it is for the
ticket.

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
