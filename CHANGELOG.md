# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.5](https://github.com/mmogr/modelpipe-ffi/compare/build/v0.4.4...build/v0.4.5) - 2026-11-10

### Other

- *(release)* pin the v0.4.4 XCFramework checksum

## [0.4.4](https://github.com/mmogr/modelpipe-ffi/compare/build/v0.4.3...build/v0.4.4) - 2026-11-10

### Other

- *(deps)* the lock resolves every dependency to the newest version its range allows ([#84](https://github.com/mmogr/modelpipe-ffi/pull/84))
- *(release)* pin the v0.4.3 XCFramework checksum

## [0.4.3](https://github.com/mmogr/modelpipe-ffi/compare/build/v0.4.2...build/v0.4.3) - 2026-11-10

Set on 2026-10-02 by [#42](https://github.com/mmogr/modelpipe-ffi/pull/42)
and released five weeks later: GitHub fired no push event for that merge, so
it was never tagged, and release-plz does not reopen a version it has already
written up. A check now fails when that happens
([#79](https://github.com/mmogr/modelpipe-ffi/pull/79)). Everything merged in
between ships here.

### Fixed

- *(identity)* a refused dial no longer removes a key another dial has just put in place ([#44](https://github.com/mmogr/modelpipe-ffi/pull/44))
- *(build)* a release build works on macOS 27 ([#68](https://github.com/mmogr/modelpipe-ffi/pull/68))
- *(release)* a version on main that was never tagged fails a check, daily and on every push ([#79](https://github.com/mmogr/modelpipe-ffi/pull/79))
- *(ci)* a smoke test that fails fails the script, wherever timeout is installed ([#63](https://github.com/mmogr/modelpipe-ffi/pull/63))
- *(ci)* every commit on main gets a CI run of its own ([#64](https://github.com/mmogr/modelpipe-ffi/pull/64))
- *(gates)* the credential check reads every exported signature whole, and the records calls take ([#66](https://github.com/mmogr/modelpipe-ffi/pull/66))

### Changed

- *(build)* the binding generator stays out of the static library ([#69](https://github.com/mmogr/modelpipe-ffi/pull/69))
- *(build)* the slices carry no LLVM bitcode: each is 60 MB rather than 222, and the zip 50 MB rather than 186 ([#70](https://github.com/mmogr/modelpipe-ffi/pull/70))
- *(deps)* built against modelpipe 0.8.8, up from 0.8.1 ([#47](https://github.com/mmogr/modelpipe-ffi/pull/47)) ([#48](https://github.com/mmogr/modelpipe-ffi/pull/48)) ([#60](https://github.com/mmogr/modelpipe-ffi/pull/60)) ([#62](https://github.com/mmogr/modelpipe-ffi/pull/62))
- *(deps)* uniffi 0.32.2 ([#43](https://github.com/mmogr/modelpipe-ffi/pull/43)), tokio 1.54.1 ([#49](https://github.com/mmogr/modelpipe-ffi/pull/49)) ([#59](https://github.com/mmogr/modelpipe-ffi/pull/59)), and every other dependency at the newest version its range allows ([#51](https://github.com/mmogr/modelpipe-ffi/pull/51)) ([#52](https://github.com/mmogr/modelpipe-ffi/pull/52)) ([#55](https://github.com/mmogr/modelpipe-ffi/pull/55)) ([#62](https://github.com/mmogr/modelpipe-ffi/pull/62))

### Other

- *(consumers)* consumers are told to pin up to the next minor, not from: ([#71](https://github.com/mmogr/modelpipe-ffi/pull/71))
- *(docs)* the comments and README say what the code does ([#72](https://github.com/mmogr/modelpipe-ffi/pull/72))
- *(cleanup)* no config that does nothing ([#73](https://github.com/mmogr/modelpipe-ffi/pull/73)); the on-device spike is retired ([#74](https://github.com/mmogr/modelpipe-ffi/pull/74))
- *(ci)* only the publishing job can write, and every job has a timeout ([#65](https://github.com/mmogr/modelpipe-ffi/pull/65)); every Linux job runs on ubuntu-26.04 ([#50](https://github.com/mmogr/modelpipe-ffi/pull/50))
- *(ci)* the Apple job skips a change that cannot affect it ([#75](https://github.com/mmogr/modelpipe-ffi/pull/75)); rust caches are saved from main only ([#76](https://github.com/mmogr/modelpipe-ffi/pull/76)); Dependabot bumps arrive grouped ([#77](https://github.com/mmogr/modelpipe-ffi/pull/77))
- *(deps)* an Update dependencies run updates the lock on demand and opens a PR for it ([#45](https://github.com/mmogr/modelpipe-ffi/pull/45))
- *(release)* pin the v0.4.2 XCFramework checksum

## [0.4.2](https://github.com/mmogr/modelpipe-ffi/compare/build/v0.4.1...build/v0.4.2) - 2026-09-26

### Fixed

- *(release)* a release is tagged on the commit its artifact was built from ([#40](https://github.com/mmogr/modelpipe-ffi/pull/40))
- *(runtime)* every export runs with the library runtime entered, so a pipe's tasks land on its two workers ([#39](https://github.com/mmogr/modelpipe-ffi/pull/39))

### Other

- *(deps)* the binding is built against modelpipe 0.8.1 ([#41](https://github.com/mmogr/modelpipe-ffi/pull/41))
- *(release)* pin the v0.4.1 XCFramework checksum

## [0.4.1](https://github.com/mmogr/modelpipe-ffi/compare/build/v0.4.0...build/v0.4.1) - 2026-09-23

### Other

- *(deps)* bump release-plz/action from 0.5.136 to 0.5.139 ([#34](https://github.com/mmogr/modelpipe-ffi/pull/34))
- *(release)* pin the v0.4.0 XCFramework checksum

## [0.4.0](https://github.com/mmogr/modelpipe-ffi/compare/build/v0.3.0...build/v0.4.0) - 2026-09-22

### Added

- *(options)* [**breaking**] a dial is told which directory to keep its key in, not which file ([#30](https://github.com/mmogr/modelpipe-ffi/pull/30))
- *(pair)* [**breaking**] a pairing answer's unexpected status crosses as a number ([#29](https://github.com/mmogr/modelpipe-ffi/pull/29))

### Other

- *(deps)* the lock resolves modelpipe 0.7.0 ([#32](https://github.com/mmogr/modelpipe-ffi/pull/32))
- *(deps)* bump release-plz/action from 0.5.133 to 0.5.136 ([#25](https://github.com/mmogr/modelpipe-ffi/pull/25))
- *(release)* pin the v0.3.0 XCFramework checksum

## [0.3.0](https://github.com/mmogr/modelpipe-ffi/compare/build/v0.2.0...build/v0.3.0) - 2026-09-16

### Added

- *(ffi)* a pairing string can be read without pairing ([#27](https://github.com/mmogr/modelpipe-ffi/pull/27))

### Fixed

- *(release)* merging a release PR pushes its build tag ([#22](https://github.com/mmogr/modelpipe-ffi/pull/22))

### Other

- *(deps)* bump uniffi from 0.32.0 to 0.32.1 ([#26](https://github.com/mmogr/modelpipe-ffi/pull/26))
- *(release)* pin the v0.2.0 XCFramework checksum

## [0.2.0](https://github.com/mmogr/modelpipe-ffi/compare/build/v0.1.4...build/v0.2.0) - 2026-09-15

### Added

- *(ffi)* pairing crosses the boundary, and a device can keep its identity ([#19](https://github.com/mmogr/modelpipe-ffi/pull/19))

### Fixed

- *(ffi)* a dropped pipe does not block, and a cancelled wait returns ([#18](https://github.com/mmogr/modelpipe-ffi/pull/18))

### Other

- *(deps)* build against modelpipe 0.6, the release that carries pairing ([#17](https://github.com/mmogr/modelpipe-ffi/pull/17))

## [0.1.4](https://github.com/mmogr/modelpipe-ffi/compare/build/v0.1.3...build/v0.1.4) - 2026-09-12

### Other

- *(release)* pin the v0.1.3 XCFramework checksum

## [0.1.3](https://github.com/mmogr/modelpipe-ffi/compare/build/v0.1.2...build/v0.1.3) - 2026-09-12

### Fixed

- *(release)* a stalled download fails in minutes, not hours ([#12](https://github.com/mmogr/modelpipe-ffi/pull/12))

### Other

- *(deps)* build against modelpipe 0.5, the crate the serving side runs ([#14](https://github.com/mmogr/modelpipe-ffi/pull/14))
- *(deps)* bump release-plz/action from 0.5.131 to 0.5.133 ([#13](https://github.com/mmogr/modelpipe-ffi/pull/13))
- *(readme)* the install snippet names a version that exists ([#11](https://github.com/mmogr/modelpipe-ffi/pull/11))
- *(release)* pin the v0.1.2 XCFramework checksum

## [0.1.2](https://github.com/mmogr/modelpipe-ffi/compare/build/v0.1.0...build/v0.1.2) - 2026-09-08

The first published release of the SwiftPM package. 0.1.1 was prepared and then
abandoned unreleased: its build tag pointed at a commit whose release workflow
carried a shell quoting bug, and the session that would have moved the tag ran
behind a sandbox egress policy that refused ref updates with a 403. That policy
is external to GitHub — this repository has never carried tag protection, and
the tag for this release was pushed without incident from a terminal outside
that sandbox. Nothing was ever published under 0.1.1, so it is folded into this
one rather than left as a gap.

### Fixed

- *(release)* a run block that parses, and a gate that would have said so ([#8](https://github.com/mmogr/modelpipe-ffi/pull/8))
- *(release)* read the version from tags, because the registry will never know ([#6](https://github.com/mmogr/modelpipe-ffi/pull/6))
- *(release)* give release-plz a baseline in the tag namespace it now reads ([#5](https://github.com/mmogr/modelpipe-ffi/pull/5))
- *(ffi)* export the error surface, and close the two gates that missed it ([#4](https://github.com/mmogr/modelpipe-ffi/pull/4))

### Added

- The connect half of modelpipe, bound for Swift: `mpConnect`, and a pipe
  object carrying `baseUrl`, `status`, `statusChangedSince`, `closeReason`,
  `notifyNetworkChange`, `networkMetrics` and `shutdown`.
- An XCFramework build covering five slices in three bundles, with a gate that
  asserts each slice carries the platform load command it claims.
- A CI job that links the generated Swift against the built framework and
  calls across the boundary, including an async call — the one that hangs
  rather than errors if the library's runtime was never started.
