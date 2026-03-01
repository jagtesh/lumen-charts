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
        .target(
            name: "LightweightCharts",
            dependencies: ["CChartCore"],
            path: "Sources/LightweightCharts"
        ),
        .executableTarget(
            name: "ChartDemo",
            dependencies: ["CChartCore", "LightweightCharts"],
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
