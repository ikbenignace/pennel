// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "PennelMacPlatform",
    platforms: [.macOS("14.4")],
    products: [.library(name: "PennelMacPlatform", type: .static, targets: ["PennelMacPlatform"])],
    targets: [
        .target(name: "PennelMacPlatform", swiftSettings: [.swiftLanguageMode(.v5)]),
        .testTarget(name: "PennelMacPlatformTests", dependencies: ["PennelMacPlatform"]),
    ]
)
