# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1](https://github.com/mmogr/modelpipe-ffi/compare/build/v0.1.0...build/v0.1.1) - 2026-09-08

### Fixed

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
