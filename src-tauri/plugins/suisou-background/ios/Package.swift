// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "tauri-plugin-suisou-background",
    platforms: [
        .iOS(.v26),
    ],
    products: [
        .library(
            name: "tauri-plugin-suisou-background",
            type: .static,
            targets: ["SuisouBackground"]
        ),
    ],
    dependencies: [],
    targets: [
        .target(
            name: "SuisouBackground",
            dependencies: [],
            path: "Sources"
        ),
    ]
)
