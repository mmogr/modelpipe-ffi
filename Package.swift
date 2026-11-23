// swift-tools-version: 6.2
import Foundation
import PackageDescription

// This crate is `publish = false` and always will be: the product is an Apple
// binary artifact, not a source tarball. THIS is how it is consumed —
// `.package(url: "https://github.com/mmogr/modelpipe-ffi.git", .upToNextMinor(from: "0.4.0"))`
// — up to the next minor, because below 1.0 a minor release can break a
// consumer (README.md, Consuming it). The two halves below are why the package
// exists at all rather than a pair of release assets a consumer assembles by
// hand.
//
// `ModelpipeFFI` is the XCFramework: three arm64 slices of a static library.
// `Modelpipe` is the generated Swift that calls into it. They are not
// independently useful and they are not independently safe: the binding
// carries UniFFI's API checksums and calls
// `fatalError("UniFFI API checksum mismatch")` when they disagree, which is a
// crash on a device at first dial rather than an error at build time. Pinning
// one version pins both, and SwiftPM's resolution is what enforces the pairing.

// The two lines `release.yml` rewrites, on their own, existing only to be
// rewritten. A sed against a manifest is fragile when it has to match a line
// that is also doing something else; these do nothing else, and the workflow
// asserts the substitution actually changed the file before it commits.
let version = "0.4.6"
let checksum = "3b05cf99932b2fbe287d7ec2f154ee482fcb621740d5dd0ebe3dfe086cfa40af"

// Between releases these two name the artifact of the last version pinned on
// `main` while `Sources/Modelpipe/modelpipe_ffi.swift` can already be ahead of
// it, so `main` is not consumable. The URL below resolves only if that
// version's release was published; README.md, Releasing, says when a pinned
// version is left unpublished. Resolve a tag, where the two are consistent by
// construction.

// Local development: `MODELPIPE_FFI_LOCAL_XCFRAMEWORK=1` swaps the download for
// whatever `make xcframework` just built. An environment switch rather than a
// `let useLocalFramework = false` toggle in the file, because the default is
// then the published one and there is nothing to remember to set back — a
// manifest accidentally committed in development mode is not a state this can
// reach.
let framework: Target = ProcessInfo.processInfo.environment["MODELPIPE_FFI_LOCAL_XCFRAMEWORK"] == nil
    ? .binaryTarget(
        name: "ModelpipeFFI",
        url: "https://github.com/mmogr/modelpipe-ffi/releases/download/v\(version)/ModelpipeFFI.xcframework.zip",
        checksum: checksum
    )
    : .binaryTarget(
        name: "ModelpipeFFI",
        path: "build/ModelpipeFFI.xcframework"
    )

let package = Package(
    name: "modelpipe-ffi",
    // Matching ggchat's own floor. A library with a lower one buys reach no
    // consumer of this can use; a higher one is a link error in their project
    // rather than ours.
    platforms: [.iOS(.v26), .macOS(.v26)],
    products: [
        .library(name: "Modelpipe", targets: ["Modelpipe"])
    ],
    targets: [
        framework,
        .target(
            name: "Modelpipe",
            dependencies: ["ModelpipeFFI"],
            // THE ONE AUTHORITATIVE COPY. A static library does not carry its
            // own dependencies: when rustc links a binary it passes these
            // frameworks itself, but a `.a` handed to someone else records
            // only that it *references* the symbols, not where they live.
            // Omit one and everything compiles, right up to the last step:
            //
            //     "_res_9_ninit", referenced from:
            //         ...netdev..os..ios..dns..get_system_dns_conf
            //         in libmodelpipe_ffi.a[251]
            //     ld: symbol(s) not found for architecture arm64
            //
            // `.binaryTarget` accepts no build settings, so they can only live
            // on a source target — and this is the one that knows which
            // frameworks iroh's transitive dependencies reach for. Declaring
            // them here rather than in each consumer is most of the reason
            // this is a package.
            //
            // The list is not typed. `scripts/link-settings.sh` asks rustc
            // what each of the three slices was declared to need
            // (`--print native-static-libs`) and writes it between the two
            // markers, minus the ones SwiftPM already passes (System, c, m);
            // CI fails when the lines here are not the ones it writes. It was
            // typed once, and 0.4.5 is what that cost: a lock update moved
            // netdev to a version that calls libresolv on iOS, the list stood
            // still, and the release linked on a Mac and nowhere else.
            //
            // A condition is on a setting only some slices need, so the other
            // platform's linker is never handed a library it may not have.
            //
            // What reaches for what, as of the list below: SystemConfiguration
            // and Network are iroh's transport, interface enumeration and
            // reachability; resolv is netdev reading the DNS servers on iOS;
            // Security is rustls-platform-verifier, via security-framework —
            // the Apple trust store, which is why there is no bundled CA set;
            // objc is objc2's runtime calls; iconv is the C dependencies'.
            // BEGIN GENERATED LINKER SETTINGS
            // Written by scripts/link-settings.sh from rustc's native-static-libs.
            // Do not edit: run `make link-settings`.
            linkerSettings: [
                .linkedFramework("CoreFoundation"),
                .linkedFramework("Foundation"),
                .linkedFramework("Network", .when(platforms: [.iOS])),
                .linkedFramework("Security"),
                .linkedFramework("SystemConfiguration"),
                .linkedLibrary("iconv"),
                .linkedLibrary("objc"),
                .linkedLibrary("resolv", .when(platforms: [.iOS])),
            ]
            // END GENERATED LINKER SETTINGS
        ),
    ],
    // The consuming app compiles this under Swift 6 with complete strict
    // concurrency. Generated code that is not `Sendable`-clean has to fail
    // here rather than in ggchat.
    swiftLanguageModes: [.v6]
)
