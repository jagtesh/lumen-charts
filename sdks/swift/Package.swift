// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "LightweightCharts",
    platforms: [.macOS(.v14), .iOS(.v17)],
    products: [
        .library(
            name: "LightweightCharts",
            targets: ["LightweightCharts"]
        ),
    ],
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
    ]
)
