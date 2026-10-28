# Pennel mobile

Expo SDK 58 application UI with native iPhone services and the shared Rust core. iOS 26+ uses native Liquid Glass tabs. Android native implementation follows iPhone acceptance.

From the repository root:

```sh
npm --prefix apps/mobile ci
make mobile-test
make mobile-ios
```

The default candidate uses isolated preview identifiers. Native projects are generated and ignored; edit the local Expo module, shared native sources or config plugins instead. Rust libraries and bindings are prepared outside generated projects.

From this directory, `npm start` runs Metro for development builds; `npm run device:hub` opens the task-local device dashboard server for simulator QA, including Release builds without Metro. Select the assigned simulator before interacting.

See [build commands and acceptance](../../docs/mobile.md), [Expo Device Hub](../../docs/mobile-device-hub.md), [simulator evidence](../../docs/mobile-simulator-qa.md), [keyboard feasibility](../../docs/mobile-keyboard-feasibility.md) and [widget integration](../../docs/mobile-widgets-feasibility.md). The native SwiftUI iPhone shell remains the comparison until migration acceptance passes.
