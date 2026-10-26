# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

Pennel is a desktop product. The shared Tauri interface targets macOS, Windows and Linux, with native services behind one Rust core. A native SwiftUI Mac application remains available for comparison and recovery until shared packaged-app acceptance passes.

## Users

Professionals and small teams who write and meet regularly, including people with no knowledge of speech models, providers or API keys. Everyday use happens inside other applications as well as Pennel's own windows. Dutch, English and mixed-language work are initial evaluation cases; quality claims require measured evidence.

## Product Purpose

Turn spoken words into text where the user is writing, and keep meetings as local transcripts and notes. First use starts with the job the person wants to do: try dictation or record a meeting. Guide only the relevant setup and prove the task with an actual result, rather than require knowledge of the settings model.

## Positioning

Local speech models and user-chosen providers share the same product logic. No account is required. API keys remain in the OS credential store, meeting files stay readable as Markdown, and logs contain timings, lengths and errors rather than dictated or meeting content.

## Operating Context

- The app stays available through its menu/tray and shortcuts after the main window closes.
- Global shortcut dictation serves the external app where the user is working. On Wayland, the configured desktop shortcut and clipboard path remain explicit.
- Meetings use microphone and, where available, system audio. Calendar, speaker recognition, model availability and permissions depend on the platform and current setup.
- The shared UI uses the same selected-language copy, layout and capabilities across macOS, Windows and Linux. Interface language follows the first supported system language, with English fallback and an explicit English/Dutch choice; it never changes speech language or user content. OS chrome, shortcut names and system permission prompts may differ.

## Capabilities and Constraints

Providers, processing, dictation history, meeting lifecycle, search, cited questions, templates, exports and model management belong in the Rust core. Tauri is the product interface and uses native capture, clipboard, focus, permissions and credential services. Swift remains behind Apple integrations; the frozen native application remains a buildable comparison and recovery reference.

The main window is the meeting library, with visible dictation, history and recording entry points. Dictation, History and Settings retain separate native windows and the dictation pill never activates. Settings opens to task readiness and keeps detailed model/provider configuration available through advanced categories. Setup detours return to the originating task with its draft, selection and reading position intact.

New profiles recommend compatible local speech recognition and Fast / Snel, with advanced alternatives available. First use carries the chosen dictation or meeting task through relevant setup; optional calendar access, speaker enrollment and summary setup appear when needed. Existing settings, providers and unfinished setup remain recoverable after upgrades. Existing users enter their ordinary windows directly; incomplete access produces a recovery action rather than a forced first-run reset.

The main dictation route explains speaking in another app; the editable, locally persistent pad is optional. Its external-paste preference defaults off and applies only to pad recordings. Global shortcut dictation freezes the active external destination when recording stops. A small non-activating pill shows real provisional text where the selected recognition provider supports it. Practice always targets its own field.

Untrusted dictated, selected and meeting text cannot authorize actions. CLI language providers run without tools in an empty folder. Preserve the existing prompt, clipboard and session protections.

## Brand Commitments

Pennel retains its checked-in marks, circular orange dot, Figtree UI and Instrument Serif large titles and wordmark. Warm paper and ink, a barely darker sidebar, restrained orange and familiar controls follow the original Swift app at `91775c1`, with matching light/dark structure across platforms. Reference products inform hierarchy, alignment and continuity; they do not replace Pennel's identity or justify product claims.

English and Dutch are equal interface languages, selected independently of speech and content. Copy is concrete, calm and direct; Dutch uses "je". Name the person's task before exposing implementation terminology.

## Evidence on Hand

- The Rust core, shared desktop app and SwiftUI reference are the implementation sources.
- `assets/brand/tokens.json` defines the shared design system, with generated CSS and Swift values.
- `assets/ui/icons/manifest.json` defines identical bundled interface glyphs for both renderers.
- `docs/shared-app-migration.md` separates implementation, automated checks, packaged builds and outstanding device acceptance.
- `DESIGN.md` records the approved visual and interaction contract. Design approval does not certify shipped behavior or hardware acceptance.

## Product Principles

- Reach first value with real speech recognition and a working input path.
- Keep local data, user configuration and pending work intact.
- Reveal advanced setup when it matters; unavailable controls remain visible with a reason.
- Make output destinations explicit and preserve ownership through asynchronous work.
- Claim only behavior supported by the relevant build and acceptance evidence.
- Optimize first for useful open-source adoption. Keep the local app free, account-free and without telemetry; optional Cloud convenience has separate availability and acceptance evidence.
- Evaluate visual impressions separately from task completion. The [usability and adoption protocol](docs/ux-adoption.md) contains the uncompleted human study work and initial targets.

## Accessibility & Inclusion

Support keyboard navigation, clear focus, readable secondary text, labeled controls, reduced motion and resizable layouts. Manual pad and practice controls must remain usable when global shortcuts or external insertion are unavailable. Light and dark modes share the same hierarchy and task paths.
