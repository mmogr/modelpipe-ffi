// swift-tools-version: 6.2
import Foundation
import PackageDescription

// This crate is `publish = false` and always will be: the product is an Apple
// binary artifact, not a source tarball. THIS is how it is consumed —
// `.package(url: "https://github.com/mmogr/modelpipe-ffi.git", from: "0.1.2")`
// — and the two halves below are why the package exists at all rather than a
// pair of release assets a consumer assembles by hand.
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
let version = "0.1.3"
let checksum = "227596e6564744a1509b8d4fc6b215436dccfc66ad6b4684dd5e788bd100d05a"

// Between releases these two name the PREVIOUS release's artifact while
// `Sources/Modelpipe/modelpipe_ffi.swift` is ahead of it. That is a true
// record — it names a zip that exists — but it means `main` is not consumable.
// Resolve a tag, where the two are consistent by construction. A consumer who
// pinned a branch would link mismatched halves and hit the checksum
// `fatalError` above, on a phone, at the first dial.

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
            //     __RNvMs_...system_configuration...SCNetworkInterfaceType13from_cfstring
            //         in libmodelpipe_ffi.a[arm64]
            //     ld: symbol(s) not found for architecture arm64
            //
            // `.binaryTarget` accepts no build settings, so they can only live
            // on a source target — and this is the one that knows which
            // frameworks iroh's transitive dependencies reach for. Declaring
            // them here rather than in each consumer is most of the reason
            // this is a package.
            //
            // Read off rustc's own link invocation for the iOS target, minus
            // the ones SwiftPM already passes (System, c, m).
            linkerSettings: [
                // iroh's transport: interface enumeration and reachability.
                .linkedFramework("SystemConfiguration"),
                // rustls-platform-verifier, via security-framework — the Apple
                // trust store, which is why there is no bundled CA set.
                .linkedFramework("Security"),
                .linkedFramework("Network"),
                .linkedFramework("CoreFoundation"),
                .linkedFramework("Foundation"),
                // objc2's runtime calls, and iconv from the C dependencies.
                .linkedLibrary("objc"),
                .linkedLibrary("iconv"),
            ]
        ),
    ],
    // The consuming app compiles this under Swift 6 with complete strict
    // concurrency. Generated code that is not `Sendable`-clean has to fail
    // here rather than in ggchat.
    swiftLanguageModes: [.v6]
)
