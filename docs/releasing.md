# Releasing Pennel

## Automatic releases (shared app)

The `release` job in `.github/workflows/desktop.yml` publishes the shared Tauri app. It runs after a push to `main` once every other job in that run passed, and only when `core/`, `apps/desktop/`, `apps/macos-platform/` or `assets/` changed since the newest `v*` tag.

- **Version.** `scripts/desktop-auto-release.py plan` bumps the patch of the newest `v*` tag. For a minor or major release, raise `version` in `apps/desktop/src-tauri/tauri.conf.json` (and `package.json`, `Cargo.toml`); a configured version higher than the bump wins. The first release uses the configured version.
- **Assets.** The tag `v<version>` and a GitHub release with `Pennel_<version>_aarch64.dmg`, the Windows NSIS `.exe` and `.msi`, the Linux `.deb` and `.AppImage`, and `SHA256SUMS.txt`.
- **Homebrew.** The job rewrites `Casks/pennel.rb` in [`pennel-app/homebrew-pennel`](https://github.com/pennel-app/homebrew-pennel) through the `HOMEBREW_TAP_DEPLOY_KEY` secret (a write deploy key on that repository). Users install with `brew install --cask pennel-app/pennel/pennel`.
- **Signing.** Mac builds are signed with the self-signed certificate *Pennel Release Signing* (hardened runtime, `Pennel.entitlements`), not notarized. macOS ties Microphone, Accessibility, Input Monitoring and Audio Capture approvals to the signature's designated requirement, `identifier "dev.pennel.Pennel" and certificate root = H"<sha1>"`; because the certificate stays the same, approvals survive every `brew upgrade`. Gatekeeper does not trust the certificate, so the cask still removes the quarantine attribute after install. Windows installers are unsigned; no in-app updater manifest is published. Once a Developer ID, notarization credentials, a Windows certificate and the Tauri updater key exist, move the build onto the signed path of `scripts/desktop-release.py` (switching to the Developer ID changes the requirement once, so users grant permissions one more time).
- **Release certificate.** Stored as the secrets `APPLE_CERTIFICATE` (base64 `.p12`) and `APPLE_CERTIFICATE_PASSWORD` and the variable `APPLE_SIGNING_IDENTITY` (`Pennel Release Signing`); valid until 2046. No copy exists outside GitHub. `scripts/import-mac-signing-cert.sh` imports it into a throwaway keychain on the runner, because Tauri's own `APPLE_CERTIFICATE` import only accepts Apple-issued identities. The bundle inspection fails when the designated requirement differs from `PENNEL_RELEASE_CERT_SHA1` in `desktop.yml`. Runs without the secret (forks) sign ad hoc. Replacing the certificate resets every user's approvals: create it like `scripts/create-dev-cert.sh` does, update the secrets and `PENNEL_RELEASE_CERT_SHA1` together, and say so in the release notes.
- Main runs queue instead of cancelling each other, so a release is never interrupted halfway.

## Native reference app (frozen)

Releases of the legacy Swift app were signed with a Developer ID, notarized by Apple, published as a GitHub release and announced to existing installs through [Sparkle](https://sparkle-project.org) (the app checks `https://pennel.app/appcast.xml` once a day and installs signed updates).

## One-time setup

1. **Developer ID certificate.** Join the Apple Developer Program, then in Xcode → Settings → Accounts → Manage Certificates add a *Developer ID Application* certificate. Check it with:
   ```sh
   security find-identity -v -p codesigning | grep "Developer ID Application"
   ```
2. **Notarization credentials** stored in the keychain under the profile name the script uses:
   ```sh
   xcrun notarytool store-credentials pennel-notary --apple-id <apple-id> --team-id <TEAMID>
   ```
   (uses an app-specific password from appleid.apple.com).
3. **Sparkle update key.** The EdDSA private key lives in the login keychain (account `pennel`); its public half is `SUPublicEDKey` in `apps/macos/Resources/Info.plist`. Back it up once — losing it means existing installs can no longer verify updates:
   ```sh
   apps/macos/.build/artifacts/sparkle/Sparkle/bin/generate_keys --account pennel -x pennel-sparkle-key.txt
   ```
   Store that file in a password manager and delete it from disk.

## Cutting a release

```sh
export PENNEL_SIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)"
scripts/release.sh 0.2.0
```

The script sets the version in `Info.plist` (build number = commit count), builds, signs inside-out (Sparkle helpers first), notarizes and staples both the `.zip` (used by Sparkle) and the `.dmg` (for first downloads), writes `appcast.xml`, and creates a **draft** GitHub release with both files.

Then:

1. Review and publish the draft release on GitHub.
2. Copy `build/release/<version>/appcast.xml` to the website (`pennel-cloud`, `apps/web/public/appcast.xml`) and deploy.
3. Commit the version bump in `Info.plist`.

`scripts/release.sh <version> --dry-run` runs everything locally with your development certificate, without notarization or GitHub, to check the pipeline.

## Notes

- GitHub release downloads only work for everyone once the repository is public. Until then, host the `.zip`/`.dmg` on pennel.app and change `download_prefix` in the script.
- Sparkle adds ~3 MB to the app.
