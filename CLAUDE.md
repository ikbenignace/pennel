# Pennel — working rules

Pennel ships one product on three platforms from one Rust core:

- `core/` — `pennel-core`: all product logic (providers, cleanup, history, meetings, search, templates, export, local models, CLI agents). New behaviour goes here first.
- `apps/desktop/` — shared Tauri application for macOS, Windows and Linux.
- `apps/macos-platform/` — embedded Swift library for macOS system integrations; no product UI or second Rust engine.
- `apps/macos/` — legacy native Swift application, frozen: no new features or restyling. It is removed once the shared application passes packaged-app and real-device acceptance.

**Decision (2026-10-25): Pennel goes all-in on Tauri.** The shared application is the only product UI on every platform; there will be no separate Swift app. Swift remains only inside `apps/macos-platform/` for Apple system integrations.

## Roadmap tracking (mandatory)

Before implementing user-authorized features, fixes, or substantial documentation/configuration changes, read and follow the [agent workflow](docs/desktop-roadmap.md#agent-workflow). It governs issue reuse or creation, status updates, verification and handoff on the [Pennel roadmap](https://github.com/orgs/pennel-app/projects/1). If tracking access fails, continue authorized repository work and report the pending updates as the protocol describes.

## UI parity and look (mandatory)

The shared application must look and behave **near-identically** on every platform, and must look and feel native: real system window chrome (no in-content title bar), Figtree for UI text and Instrument Serif for large titles, one calm warm background with a barely darker sidebar, no bordered panels, native-looking controls, real app icons where apps are listed. The visual reference is the original Swift app at commit `91775c1`.

- Same windows, tabs, sections, order, labels and empty/error states on every platform. Platform differences are limited to what the OS forces (window chrome, hotkey names: `fn` on Mac, Right Ctrl on Windows/Linux, system permission prompts).
- Same visual system: palette, typography (Figtree + Instrument Serif), spacing, radii, component shapes, pill, icons. Values come from the brand guide `assets/brand/brand.md`; never hard-code a new colour or size in only one app.
- Same copy: UI text is Dutch-first and identical on every platform.
- When a feature exists in the core, the shared app exposes it on every platform. If one platform cannot support it yet (e.g. echo cancellation, Apple dictation), show the same control disabled with a short reason rather than hiding it silently, and record the gap in `docs/desktop.md`.
- Review the look with screenshots of the app's own windows or headless renders, never by capturing the user's screen.

## Build and test everything

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
- Tests first for logic (Rust unit tests in `core/`, Rust/TS tests in `apps/desktop`).
