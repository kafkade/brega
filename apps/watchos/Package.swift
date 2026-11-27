// swift-tools-version: 5.9

import PackageDescription

let package = Package(
    name: "BregaWatch",
    platforms: [
        .watchOS(.v10),
    ],
    dependencies: [
        .package(path: "../../bindings/swift"),
    ],
    targets: [
        .target(
            name: "BregaWatch",
            dependencies: [
                .product(name: "BregaSwift", package: "Brega"),
            ],
            path: "Sources/BregaWatch",
            swiftSettings: [
                .enableExperimentalFeature("StrictConcurrency"),
            ]
        ),
    ]
)
