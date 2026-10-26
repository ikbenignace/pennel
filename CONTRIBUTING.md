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
- UI copy is Dutch-first today; keep strings short and plain.

## License

By contributing you agree that your contributions are licensed under the [Apache License 2.0](LICENSE).
