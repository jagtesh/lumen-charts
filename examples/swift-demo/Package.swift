// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "ChartDemo",
    platforms: [.macOS(.v14)],
    dependencies: [
        .package(path: "../../sdks/swift"),
    ],
    targets: [
        .executableTarget(
            name: "ChartDemo",
            dependencies: [
                .product(name: "LightweightCharts", package: "swift"),
            ],
            path: "Sources/ChartDemo",
            linkerSettings: [
                .unsafeFlags([
                    "-L", "../../core/target/release",
                    "-lchart_core",
                ]),
                .linkedFramework("QuartzCore"),
                .linkedFramework("Metal"),
                .linkedFramework("AppKit"),
            ]
        ),
    ]
)
