# Pennel for iPhone

The iPhone app is native SwiftUI on the shared Rust core (`pennel-core`), linked as an XCFramework.

## Build

```sh
make ios-core   # XCFramework (ios-arm64 + ios-arm64-simulator) and Swift bindings
make ios        # ios-core, then an unsigned device build when apps/ios/Pennel.xcodeproj exists
make ios-test   # ios-core, then swift test for PennelMobileKit and xcodebuild test on the iPhone 17 simulator, for the parts that exist
```

Requirements: Xcode, the Rust targets `aarch64-apple-ios` and `aarch64-apple-ios-sim`
(`rustup target add aarch64-apple-ios aarch64-apple-ios-sim`).

`scripts/build-core-ios.sh` builds the core with `--no-default-features` (no local models) and
`IPHONEOS_DEPLOYMENT_TARGET=26.0`, writes the XCFramework to `apps/ios/Frameworks/` and the generated
bindings to `apps/ios/PennelCore/` (both git-ignored). It fails when the generated `pennel_core.swift`
differs from the macOS build in `build/bindings/`, so the uniffi surface stays identical; run `make core`
first so that check runs.

## Sizes

Measured with `scripts/build-core-ios.sh` (unstripped archives are about 93 MB): each release `libpennel_core.a` is copied and stripped with
`strip -S -x`, then its byte size is read.

| Slice | Stripped `libpennel_core.a` |
| --- | --- |
| ios-arm64 | 23006680 bytes |
| ios-arm64-simulator | 22998400 bytes |

Archived app size: pending device acceptance.

## Device acceptance

Pending device acceptance (no physical device available yet): memory profile in Instruments, 60-minute
meeting, dictation through the keyboard on a device, archive and App Store size.
