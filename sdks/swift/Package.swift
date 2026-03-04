// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "LumenCharts",
    platforms: [.macOS(.v14), .iOS(.v17)],
    products: [
        .library(
            name: "LumenCharts",
            targets: ["LumenCharts"]
        ),
    ],
    targets: [
        .systemLibrary(
            name: "CChartCore",
            path: "Sources/CChartCore"
        ),
        .target(
            name: "LumenCharts",
            dependencies: ["CChartCore"],
            path: "Sources/LightweightCharts"
        ),
    ]
)
