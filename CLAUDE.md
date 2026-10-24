# Pennel — working rules

Pennel ships one product on three platforms from one Rust core:

- `core/` — `pennel-core`: all product logic (providers, cleanup, history, meetings, search, templates, export, local models, CLI agents). New behaviour goes here first.
- `apps/desktop/` — shared Tauri application for macOS, Windows and Linux.
- `apps/macos-platform/` — embedded Swift library for macOS system integrations; no product UI or second Rust engine.
- `apps/macos/` — native reference application, retained until the shared application passes packaged-app and real-device acceptance.

## UI parity (mandatory)

The shared application must look and behave **near-identically** on every platform. During migration, use the native Mac application as the existing product baseline, complete missing shared surfaces, and keep the native application available until acceptance passes. Product additions unrelated to the migration are out of scope.

- Same windows, tabs, sections, order, labels and empty/error states on every platform. Platform differences are limited to what the OS forces (window chrome, hotkey names: `fn` on Mac, Right Ctrl on Windows/Linux, system permission prompts).
- Same visual system: palette, typography (Figtree + Instrument Serif), spacing, radii, component shapes, pill, icons. Values come from the brand guide `assets/brand/brand.md`; never hard-code a new colour or size in only one app.
- Same copy: UI text is Dutch-first and identical in both apps.
- When a feature exists in the core, both UIs expose it. If one platform cannot support it yet (e.g. echo cancellation, Apple dictation), show the same control disabled with a short reason rather than hiding it silently, and record the gap in `docs/desktop.md`.
- Review parity with screenshots of both apps rendered side by side (headless or the app's own windows), never by capturing the user's screen.

## Build and test both

Before calling any change done, build and test everything it can affect:

```sh
make core && make test && make app                    # macOS app + core
cd apps/desktop && npm ci && npm run build            # desktop frontend
cd apps/desktop/src-tauri && cargo test               # desktop backend
cd core && cargo fmt -- --check && cargo clippy --lib --tests -- -D warnings
```

CI (`.github/workflows/ci.yml`, `desktop.yml`) must stay green on macOS, Windows and Linux.

## Product rules

- Private by default: no telemetry, no account required, API keys only in the OS credential store, meetings are local Markdown files.
- Never log or transmit dictated text or meeting content except to the provider the user chose; logs hold timings, lengths and errors only.
- Untrusted text (transcripts, selections) must never be able to trigger actions: CLI agents run without tools in an empty folder; prompts wrap user text as data.
- Keep it light: no Electron, no heavy dependencies without a clear reason. Retain native Swift services behind the shared Tauri interface rather than rewriting working Apple integrations.
- Only claim what ships (README, website, release notes).

## Conventions

- Conventional Commits; small, focused commits.
- Comments only for non-obvious intent, constraints or workarounds.
- Tests first for logic (Rust unit tests in `core/`, pure Swift logic in `PennelKit`, Rust/TS tests in `apps/desktop`).
