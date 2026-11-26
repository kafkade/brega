// swift-tools-version: 5.9

import PackageDescription

let package = Package(
    name: "Brega",
    platforms: [
        .iOS(.v17),
        .macOS(.v14),
    ],
    products: [
        .library(
            name: "BregaSwift",
            targets: ["BregaSwift"]
        ),
    ],
    targets: [
        // Pre-compiled Rust static library (libbrega_uniffi.a) packaged as an
        // XCFramework, exposing the C FFI module `brega_uniffiFFI` consumed by
        // the generated Swift bindings. Built by `cargo xtask xcframework`
        // (gitignored — regenerated on demand).
        .binaryTarget(
            name: "BregaFFIBinary",
            path: "BregaFFI.xcframework"
        ),

        // UniFFI-generated Swift bindings. The single file
        // `Sources/BregaFFI/brega_uniffi.swift` is emitted by
        // `cargo xtask xcframework` (gitignored). `Placeholder.swift` keeps
        // the directory tracked before generation has run.
        //
        // Regenerate with:
        //
        //     cargo xtask xcframework
        //
        .target(
            name: "BregaFFI",
            dependencies: ["BregaFFIBinary"],
            path: "Sources/BregaFFI"
        ),

        // Idiomatic Swift wrapper: async/await, Sendable conformances,
        // and SwiftUI-friendly extensions.
        .target(
            name: "BregaSwift",
            dependencies: ["BregaFFI"],
            path: "Sources/BregaSwift"
        ),

        .testTarget(
            name: "BregaSwiftTests",
            dependencies: ["BregaSwift"],
            path: "Tests/BregaSwiftTests"
        ),
    ]
)
