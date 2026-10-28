# Expo Device Hub for Pennel mobile QA

Pennel uses `expo-device-hub` 0.18.1 as a pinned development dependency. It is not imported by the application or native runtime. Its Expo module manifest supports only `devtools`, so it does not add an Apple native module. Expo SDK 57 or newer is required; the mobile workspace remains on SDK 58.

Sources: [official README](https://github.com/expo/expo-device-hub), [package and DevTools manifest](https://github.com/expo/expo-device-hub/tree/main/packages/expo-device-hub).

## Start the dashboard

From `apps/mobile`:

```sh
npm run device:hub
```

The project launcher runs the documented standalone CLI with `--host 127.0.0.1 --port 8879 --platform ios --transport mjpeg --hide-boot-device`. Open [the local dashboard](http://127.0.0.1:8879/). The explicit port fails if occupied; it does not stop or replace another service. To select a different unused port, run `npm run device:hub -- --port 8880`.

The launcher sets `EXPO_DEVICE_HUB_SCREENSHOT_DIRECTORY` to the ignored `build/mobile/qa/device-hub` directory. It does not launch Metro, boot a simulator, enable recording/network capture, configure credentials, or start a remote service. Stop only this launcher with Ctrl+C after QA.

Normal `npm start` also discovers Device Hub automatically and advertises `http://localhost:8081/_expo/plugins/expo-device-hub` when Metro uses port 8081. No app-config plugin entry or application import is needed. The actual Expo SDK 58 DevTools plugin manager and autolinking resolver were checked for this registration.

## Select the assigned simulator

The current exclusive visual-QA target is iPhone 17 / iOS 26.5, UUID `64C462E2-0904-4E38-98C0-F1F19690D2E4`. Parent hosted tests use a separate iOS 27 simulator. Do not boot, stop, rotate, type into, or change settings on other devices.

A fresh Hub dashboard automatically selects the first available device, and its store has no documented device selector in the dashboard URL. Select the assigned iOS 26.5 row and confirm the device before sending any input. The hidden boot controls are a UI convenience, not an API authorization boundary.

`GET /api/devices?booted=true` lists booted devices. `GET /readyz` and `/healthz` verify the Hub process; Hub readiness alone does not establish a connected device stream.

## Screenshots and interaction

Use the dashboard for live preview, taps, swipes, scrolling, typing, appearance and accessibility inspection. Inspect actual control labels before acting.

To save an explicitly targeted PNG through Device Hub into the project QA directory:

```sh
npm run device:hub:screenshot -- \
  --device 64C462E2-0904-4E38-98C0-F1F19690D2E4 \
  --name pennel-preview.png
```

The helper first checks the documented device-scoped discovery response, then calls the documented PNG screenshot endpoint. It refuses an unconfirmed device or an existing output filename. It does not use Argent or invoke Simulator screenshots directly. Capture only Pennel's owned preview/test profile.

The upstream screenshot environment variable supports session artifacts, but a loopback preview may retain its Desktop-save behavior. The project screenshot helper saves the HTTP PNG response explicitly to the ignored QA directory, independently of that UI behavior.

Supported source-backed native paths under the standalone Hub:

| Path | Purpose |
| --- | --- |
| `/vendor/serve-sim/api?device=<UUID>` | Selected simulator/stream descriptor, advertised endpoints and execution token |
| `POST /vendor/serve-sim/api/screenshot?device=<UUID>` | PNG response without the host action's Desktop copy |
| `/vendor/serve-sim/exec-ws` | Typed simulator actions, using the advertised execution token and origin policy |
| `/vendor/serve-sim/helper/ws?device=<UUID>` | Device HID channel |

Keep execution tokens in memory and out of logs. With the Expo-start plugin, prefix native routes with `/_expo/plugins/expo-device-hub` instead of the standalone origin root.

The official action contract uses `{id, action, params}`. Documented input actions include `tap` with `{udid, x, y}` normalized to 0..1, `gesture` with a complete begin/move/end sequence, and `type` with `{udid, text}`. Request `actions.describe` with `{}` parameters to discover current action names and JSON Schemas rather than inventing commands. The dashboard's bundled client already handles the stream and input protocols.

References: [Hub client API](https://github.com/expo/expo-device-hub/blob/main/packages/@expo/hub-client/README.md), [serve-sim HTTP/WebSocket specification](https://github.com/expo/expo-device-hub/blob/main/llp/0003-serve-sim-http-api.spec.md), [standalone CLI options](https://github.com/expo/expo-device-hub/blob/main/packages/expo-device-hub/src/server/cli/options.ts).

## Evidence limits

Local Hub previews and screenshots support simulator QA. They do not establish physical-device keyboard behavior, microphone/background acceptance, memory/battery measurements on iPhone, App Review approval, or Cloud deployment.
