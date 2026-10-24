// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "Pennel",
    platforms: [.macOS("14.4")],
    targets: [
        .binaryTarget(name: "PennelCoreFFI", path: "Frameworks/PennelCoreFFI.xcframework"),
        .target(
            name: "PennelCore",
            dependencies: ["PennelCoreFFI"],
            swiftSettings: [.swiftLanguageMode(.v5)],
            // whisper.cpp inside the Rust core runs on Metal/Accelerate and is C++.
            linkerSettings: [
                .linkedFramework("Metal"),
                .linkedFramework("MetalKit"),
                .linkedFramework("Accelerate"),
                .linkedFramework("Foundation"),
                .linkedLibrary("c++"),
            ]
        ),
        .target(name: "PennelKit"),
        .executableTarget(name: "Pennel", dependencies: ["PennelCore", "PennelKit"]),
        .testTarget(name: "PennelKitTests", dependencies: ["PennelKit"]),
        .testTarget(name: "PennelCoreSmokeTests", dependencies: ["PennelCore"]),
    ]
)
