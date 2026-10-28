# Contributing to Pennel

Thanks for helping. Pennel aims to stay small, fast and private, so a few ground rules:

## Before you start

- For anything bigger than a bug fix, open an issue first so we can agree on the approach. The [desktop roadmap](docs/desktop-roadmap.md) collects the current acceptance and release work.
- Keep the app light: no new dependency without a clear reason, no Electron, no telemetry.
- Privacy is a feature: never log or send dictated text or meeting content anywhere the user didn't choose.

## Where changes belong

- `core/` owns shared product behavior, providers, models, recording, storage and processing.
- `apps/desktop/` is the Tauri 2 application for macOS, Windows and Linux. Keep its interface and copy consistent across platforms; explain unavailable platform features visibly.
- `apps/macos-platform/` provides Apple system integrations through an embedded Swift library.
- `apps/macos/` is the frozen native reference and recovery app. Keep it buildable; add product features to the shared application.

See [desktop development](docs/desktop.md#build-and-test) for prerequisites, hot reload, previews and platform-specific setup. Shared design assets and writing guidance live in the [brand guide](assets/brand/brand.md).

## Workflow

1. Fork, create a branch from `main`.
2. Verify the parts your change affects. Add tests for new logic in the shared Rust core, Tauri host, frontend or Swift platform library.
3. Run the relevant checks below. Keep the native reference working when changing its shared core or bindings.
4. Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`, `docs:` …).
5. Open a pull request describing what changed and how you tested it. Include OS version, devices and package revision for audio, permission or installer changes; keep private meeting content and credentials out of reports.

Run checks from the repository root:

```sh
python3 scripts/generate-theme.py --check
(cd apps/desktop && npm ci && npm run build && npm test)
cargo test --manifest-path core/Cargo.toml --lib
cargo fmt --manifest-path core/Cargo.toml -- --check
cargo clippy --manifest-path core/Cargo.toml --lib --tests -- -D warnings
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets -- -D warnings
make packaging-test
```

On macOS, also check affected Apple integrations and the native reference:

```sh
swift test --package-path apps/macos-platform
make core && make test && make app
```

Documentation-only changes need link and rendering checks. UI changes need light/dark app-window captures or owned headless renders. Installed-package and real-device evidence are separate from unit tests and screenshots; use the [pilot checklist](docs/improvement-pilot.md) when recording that evidence.

## Pull request checks

PR jobs run only for affected components. README and documentation changes run
lightweight change-selection tests without building apps. Desktop changes build
the shared app on all three platforms; core changes also test the core and build
its legacy macOS consumer. macOS platform changes run platform tests and desktop
builds. Shared assets build both apps; Tauri bundle configuration also runs
packaging tests. Build scripts, workflows, the Makefile, Cargo configuration, and
unrecognized inputs conservatively run all checks.

Selection uses the complete merge-base diff, including deleted and renamed files.
Jobs are skipped within workflows so checks still report a result. Branch pushes
do not duplicate PR builds; pushes to `main` and manual desktop runs retain full
validation and existing release behavior. Rules live in `scripts/ci-changes.py`;
verify them with:

```sh
python3 -B -m unittest discover -s scripts/tests -p 'test_ci_changes.py'
actionlint -shellcheck=
```

## Code style

- Clear names and small functions over comments; comment only non-obvious intent, constraints or workarounds.
- Keep canonical defaults, application-owned diagnostics and tool messages in English. Localize UI copy through the shared English/Dutch catalogs; keep strings short and plain. Follow the [language policy](docs/language-policy.md) for generated content and compatibility aliases.

## License

By contributing you agree that your contributions are licensed under the [Apache License 2.0](LICENSE).
