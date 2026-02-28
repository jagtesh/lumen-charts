// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "ChartDemo",
    platforms: [.macOS(.v14)],
    targets: [
        .systemLibrary(
            name: "CChartCore",
            path: "Sources/CChartCore"
        ),
        .executableTarget(
            name: "ChartDemo",
            dependencies: ["CChartCore"],
            path: "Sources/ChartDemo",
            linkerSettings: [
                .unsafeFlags([
                    "-L", "../chart-core/target/release",
                    "-lchart_core",
                ]),
                .linkedFramework("QuartzCore"),
                .linkedFramework("Metal"),
                .linkedFramework("AppKit"),
            ]
        ),
    ]
)
