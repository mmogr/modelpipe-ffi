# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
