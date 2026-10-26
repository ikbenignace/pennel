# README artwork

The root README uses the existing Pennel wordmark and a dark-background variant with the same geometry. The variant uses Paper (`#FCFBF8`) and Soft wave (`#F2B48E`) from the [brand guide](../brand/brand.md).

`meeting-light.png` and `meeting-dark.png` render the shared application's actual meeting-notes view with the existing synthetic fixture. They contain demonstration content, not a user's meeting. The interface is Dutch; the README describes the product in English.

Source revision: `8be90ee037c538ccbfcf1ab91d8cc76c050dce0f`. These are owned Chromium renders of the application interface, not native-window captures or real-device acceptance evidence.

To refresh them from the repository root, install the locked frontend dependencies and use an available Chromium browser:

```sh
(cd apps/desktop && npm ci && npm run build)
node scripts/capture-desktop-reference.mjs --case meeting-notes --appearance both --output build/readme-captures
cp build/readme-captures/light/meeting-notes.png assets/readme/meeting-light.png
cp build/readme-captures/dark/meeting-notes.png assets/readme/meeting-dark.png
```

The capture script accepts `--browser /absolute/path/to/chromium` if automatic browser discovery does not find it. Check the capture manifest for failures, inspect both images and update the source revision above. Keep real meeting content, credentials and personal data out of artwork.
