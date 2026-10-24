# Contributing to Pennel

Thanks for helping. Pennel aims to stay small, fast and private, so a few ground rules:

## Before you start

- For anything bigger than a bug fix, open an issue first so we can agree on the approach.
- Keep the app light: no new dependency without a clear reason, no Electron/web views, no telemetry.
- Privacy is a feature: never log or send dictated text or meeting content anywhere the user didn't choose.

## Workflow

1. Fork, create a branch from `main`.
2. `make core && make test` must pass. Add tests for new logic: Rust unit tests in `core/`, pure Swift logic in `PennelKit` with XCTest.
3. Run `cargo fmt` and `cargo clippy` in `core/`.
4. Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`, `docs:` …).
5. Open a pull request describing what changed and how you tested it (include macOS version and Mac model for audio/permission changes).

## Code style

- Clear names and small functions over comments; comment only non-obvious intent, constraints or workarounds.
- UI copy is Dutch-first today; keep strings short and plain.

## License

By contributing you agree that your contributions are licensed under the [Apache License 2.0](LICENSE).
