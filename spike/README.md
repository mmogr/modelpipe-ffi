# The on-device spike

A throwaway iPhone app that puts a real pipe on a real phone. It is not
ggchat, is not meant to become it, and nothing here is written to last.

## Why it exists

Everything else about this library is now proven: the crate's suite runs
against real loopback sockets, the XCFramework builds and each slice is
checked for the platform and deployment floor it claims, and CI links the
generated Swift and calls across the boundary including the async path.

None of that touches a network, and none of it runs on a phone. Four
questions remain, and only a device answers them:

1. **Does iroh hole-punch from behind carrier-grade NAT at all?**
2. **Do `portmapper`, `igd-next`, `netdev` and `hickory-resolver` work under
   iOS entitlements**, or does one fail silently? modelpipe's own issue #45
   flags this: *"Compiling is not running."*
3. **Does a tokio runtime survive an iOS process suspension?** A pipe that
   comes back dead looks exactly like a network problem.
4. **What is the RTT, and is the path `direct` or `relayed`?**

## Expect `relayed`

CGNAT punches far less often than a home network. A relayed pipe is a
**working** pipe — the relay carries ciphertext it cannot read — so `relayed`
is a pass, not a failure. It matters because ggchat's status pill copy has to
read as a path rather than a warning, and this is where that gets decided.

## Running it

Needs a Mac with Xcode, `xcodegen`, and an iPhone. A **free personal Apple ID
team** is enough — 7-day builds, no paid Apple Developer Program.

```bash
# From the repository root. Builds the XCFramework, then generates and opens
# the Xcode project.
make spike
```

Then in Xcode: select your device, set the signing team on the target if
`DEVELOPMENT_TEAM` in `project.yml` is still empty, and run.

On the desktop:

```bash
gglib remote enable            # prints the ticket and a six-digit code
gglib config settings show     # prints proxy_api_key, unmasked
```

In the app: paste the pairing string (or just the ticket — it splits a
trailing six-digit code off for you) and the API key, then **Dial**.

It does not redeem the pairing code. That path is already covered by ggchat's
own tests, and adding it here would put a second thing that can fail in front
of the thing being measured.

## What to record

**Put the phone on cellular, not wifi.** On wifi this measures your LAN and
answers none of the four questions.

| Reading | Where |
|---|---|
| Does it form at all | The `FIRST PATH` line |
| `direct` or `relayed` | Same line |
| Time to first path | Same line, seconds since the dial |
| Whether the relay is throttling | The `relay:` line — the rate-limited counter |
| End-to-end request | **GET models** → a `200` proves the desktop answered |
| Survives suspension | Background the app for a minute, return. The resume hook fires automatically and logs the status |

Every line is selectable, so the transcript can be copied straight out.

## If it fails

A dial that refuses immediately is a bad ticket or a stale one — the ticket
dies when the desktop's daemon stops, and `gglib remote enable` mints a fresh
one each time.

A pipe that binds and then sits at `idle` forever is the interesting failure,
and the one worth capturing carefully: it means `connect` worked and the peer
was never reached. Note whether the `relay:` counters move at all, because a
relay that is refusing this endpoint and a network that is silently dropping
the traffic look identical from the outside and are not the same problem.
