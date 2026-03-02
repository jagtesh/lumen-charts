// swift-tools-version: 5.9
import PackageDescription
import Foundation

// Resolve the library path from env or default to the repo's build output
let repoRoot = URL(fileURLWithPath: #filePath)
    .deletingLastPathComponent()  // Sources dir → swift-demo/
    .deletingLastPathComponent()  // swift-demo/ → examples/
    .deletingLastPathComponent()  // examples/ → repo root
    .path

let libPath = ProcessInfo.processInfo.environment["LUMEN_LIB_PATH"]
    ?? "\(repoRoot)/core/target/release"

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
                    "-L", libPath,
                    "-llumen_charts",
                ]),
                .linkedFramework("QuartzCore"),
                .linkedFramework("Metal"),
                .linkedFramework("AppKit"),
            ]
        ),
    ]
)
