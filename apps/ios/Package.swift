// swift-tools-version: 5.9

import PackageDescription

let package = Package(
    name: "BregaApp",
    platforms: [
        .iOS(.v17),
        .macOS(.v14),
    ],
    dependencies: [
        .package(path: "../../bindings/swift"),
    ],
    targets: [
        .target(
            name: "BregaApp",
            dependencies: [
                .product(name: "BregaSwift", package: "swift"),
            ],
            path: "Sources/BregaApp",
            exclude: [
                "App/BregaApp.swift",
            ],
            swiftSettings: [
                .enableExperimentalFeature("StrictConcurrency"),
            ]
        ),
        .testTarget(
            name: "BregaAppTests",
            dependencies: ["BregaApp"],
            path: "Tests/BregaAppTests"
        ),
    ]
)
