# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

Pennel is a desktop product. The shared Tauri interface targets macOS, Windows and Linux, with native services behind one Rust core. A native SwiftUI Mac application remains available for comparison and recovery until shared packaged-app acceptance passes.

## Users

People who prefer speaking while writing messages, notes and other text, and people who want transcripts and useful notes from meetings. Everyday use happens inside other applications as well as Pennel's own workspace.

## Product Purpose

Turn spoken words into text where the user is writing, and keep meetings as local transcripts and notes. The first-use experience should prove this with one successful dictation rather than require users to learn the full settings model.

## Positioning

Local speech models and user-chosen providers share the same product logic. No account is required. API keys remain in the OS credential store, meeting files stay readable as Markdown, and logs contain timings, lengths and errors rather than dictated or meeting content.

## Operating Context

- The app stays available through its menu/tray and shortcuts after the main window closes.
- Global shortcut dictation serves the external app where the user is working. On Wayland, the configured desktop shortcut and clipboard path remain explicit.
- Meetings use microphone and, where available, system audio. Calendar, speaker recognition, model availability and permissions depend on the platform and current setup.
- The shared UI and native reference use the same Dutch labels, fonts, colors, ordering and product capabilities. OS chrome, shortcut names and system permission prompts may differ.

## Capabilities and Constraints

Providers, processing, dictation history, meeting lifecycle, search, cited questions, templates, exports and model management belong in the Rust core. Tauri and SwiftUI render state and use native capture, clipboard, focus, permissions and credential services.

The approved redesign uses one workspace with Dicteren, Vergaderingen, Geschiedenis and Instellingen, plus a separate headerless onboarding window. The native Mac app must remain available throughout migration.

New profiles begin with local speech recognition and Snel. Existing settings and provider relationships survive upgrades. Existing users enter the workspace directly; incomplete access produces a recovery action rather than a forced first-run reset.

The main dictation route explains speaking in another app; the editable, locally persistent pad is optional. Its external-paste preference defaults off and applies only to pad recordings. Global shortcut dictation freezes the active external destination when recording stops. A small non-activating pill shows real provisional text where the selected recognition provider supports it. Practice always targets its own field.

Untrusted dictated, selected and meeting text cannot authorize actions. CLI language providers run without tools in an empty folder. Preserve the existing prompt, clipboard and session protections.

## Brand Commitments

Pennel retains its checked-in marks, circular orange dot, Figtree UI and Instrument Serif wordmark. The approved interface is a compact neutral desktop tool with matching light and dark appearances. Raycast and FluidVoice establish the expected level of native familiarity and guided first use; they do not supply Pennel's identity or product claims.

Dutch is the first product language. Copy is concrete, calm and direct; use "je" and the user's task vocabulary.

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

## Accessibility & Inclusion

Support keyboard navigation, clear focus, readable secondary text, labeled controls, reduced motion and resizable layouts. Manual pad and practice controls must remain usable when global shortcuts or external insertion are unavailable. Light and dark modes share the same hierarchy and task paths.
