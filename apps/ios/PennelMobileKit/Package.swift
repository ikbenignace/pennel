// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "PennelMobileKit",
    platforms: [.iOS("26.0"), .macOS("14.4")],
    products: [
        .library(name: "PennelMobileKit", targets: ["PennelMobileKit"]),
    ],
    targets: [
        .target(name: "PennelMobileKit"),
        .testTarget(name: "PennelMobileKitTests", dependencies: ["PennelMobileKit"]),
    ]
)
