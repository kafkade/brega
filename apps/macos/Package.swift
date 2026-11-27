// swift-tools-version: 5.9

import PackageDescription

let package = Package(
    name: "BregaMac",
    platforms: [
        .macOS(.v14),
    ],
    dependencies: [
        .package(path: "../../bindings/swift"),
    ],
    targets: [
        .executableTarget(
            name: "BregaMac",
            dependencies: [
                .product(name: "BregaSwift", package: "Brega"),
            ],
            path: "Sources/BregaMac",
            swiftSettings: [
                .enableExperimentalFeature("StrictConcurrency"),
            ]
        ),
    ]
)
