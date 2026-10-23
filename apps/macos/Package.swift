// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "Musli",
    platforms: [.macOS("14.4")],
    targets: [
        .binaryTarget(name: "MusliCoreFFI", path: "Frameworks/MusliCoreFFI.xcframework"),
        .target(
            name: "MusliCore",
            dependencies: ["MusliCoreFFI"],
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
        .target(name: "MusliKit"),
        .executableTarget(name: "Musli", dependencies: ["MusliCore", "MusliKit"]),
        .testTarget(name: "MusliKitTests", dependencies: ["MusliKit"]),
        .testTarget(name: "MusliCoreSmokeTests", dependencies: ["MusliCore"]),
    ]
)
