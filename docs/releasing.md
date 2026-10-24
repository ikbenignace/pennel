# Releasing Pennel

Releases are signed with a Developer ID, notarized by Apple, published as a GitHub release and announced to existing installs through [Sparkle](https://sparkle-project.org) (the app checks `https://pennel.app/appcast.xml` once a day and installs signed updates).

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
