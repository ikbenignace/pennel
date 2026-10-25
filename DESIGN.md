# Pennel design system

This is the approved desktop redesign contract. Runtime and device acceptance are recorded separately from design approval.

## Visual language

Pennel is a compact native-feeling tool. Neutral surfaces, familiar controls and a stable sidebar let the user's text lead. Figtree carries the entire interface, including headings; Instrument Serif belongs to the wordmark and marketing.

The main workspace has Dicteren, Vergaderingen, Geschiedenis and Instellingen. Settings retain Providers, Modellen, Dicteren, Meetings, Opnemen, Woordenlijst and Stijlen in that order, followed by Weergave. Existing product functions stay reachable as content moves into this structure.

Use spacing to group related controls before adding a container. Prefer unframed reading content, compact source lists, aligned setting rows and progressive detail. Orange identifies selected state, focus or an actionable problem; primary controls use a neutral foreground/background pair. Product surfaces are opaque. Motion conveys an actual state change and respects reduced motion.

## Shared tokens

`assets/brand/tokens.json` is the source of truth. `python3 scripts/generate-theme.py` emits:

- `apps/desktop/src/theme.generated.css` for shared webview content.
- `apps/macos/Sources/PennelKit/ThemeTokens.generated.swift` for the native reference.

`assets/ui/icons/manifest.json` owns the compact shared 24 pt icon paths and 1.7 pt rounded strokes. The same generator emits local SVGs and `Icons.generated.swift`; the native `PennelIcon` loader uses template images and carries the identical SVG as a development fallback. Both renderers consume these shapes rather than substituting different platform symbols.

Edit the token source, then regenerate. `--check` validates output reproducibility and text contrast; `--self-test` verifies failure cases and platform equivalence. The generator has no third-party runtime dependency.

Core color roles are Surface, Chrome, Sidebar, Foreground, Muted, Border, Subtle, Hover, Field, Selection, Button, Button text, Accent and Focus ring. Components use these roles rather than adding private hex values. The approved light secondary text is slightly darkened to retain AA contrast on selected-row backgrounds.

The 4 pt spacing grid, 6 pt controls, 8 pt segmented controls and 12 pt panels/windows are shared. Captions start at 11 pt, common controls at 12–13 pt, reading content at 14 pt, workspace headings at 21 pt and onboarding headings at 26–28 pt. Use Regular, Medium and SemiBold Figtree purposefully, with sentence case and restrained heading tracking.

Compatibility names remain semantic: Paper maps to Surface, Background to Subtle, Ink to Foreground and Line to Border. These values adapt to appearance. Fixed artwork colors and dictation-pil colors have separate roles.

## Appearance

Systeem follows the operating system. Licht and Donker override it for every open product window, menu, dialog, picker and editor. Apply the preference before the window is shown and update it live without replacing focused inputs.

Both appearances have the same geometry, content and control states. The dark palette is charcoal rather than warm brown; light uses white content and neutral-gray secondary surfaces. Readable text contrast is at least 4.5:1, including metadata and placeholders.

The dictation pill stays Ink with Paper text and a Soft wave audio/processing indicator in both appearances. Do not invert it by reusing ordinary window foreground and background roles. Speaker identity uses the shared four-color set with appearance-specific readable variants; labels also identify the speaker.

## Onboarding

Use a separate headerless window without a product titlebar or traffic-light buttons. A small Pennel corner label, accessible drag area and stable bottom actions provide orientation without adding window chrome. Keep the action reachable when content must scroll.

The flow is Welkom → spraakherkenning en taal → toegang → eerste dictaat. The engine step has a compact adjacent language label/control, one native segmented selector for Lokaal / Apple-dicteren / Eigen provider, and one unframed detail area. The footer owns the primary action: Download model → Downloaden… → Verder, Apple-dicteren's Verder, or Gebruik provider.

Show real model metadata, progress and errors. Permission controls describe their task and provide the actual recovery action. Calendar and system audio are introduced when meetings need them. Denied or unknown access must not become an unexplained disabled dead end.

Practice uses the actual configured speech engine and a visible editable result field. The configured shortcut and manual recording action share the same session. Practice never falls back to another app or the clipboard. Only a real successful result completes the wizard; Later instellen and Later proberen preserve a resumable state.

## Workspace and data entry

Dicteren leads with using the shortcut in another app. The persistent editable pad is an explicit secondary surface under Open dicteerpad. Its optional external-paste setting applies only to pad sessions and shows the captured target app. Text remains in the pad if external delivery is unavailable or cancelled. Global dictation freezes its current external destination when recording stops and never retargets after asynchronous processing.

The floating pill shows at most two recent lines of real recognition text above a small listening/working indicator. It does not activate Pennel or capture input focus. Provisional text replaces earlier revisions rather than accumulating duplicates; working retains the current caption, and cancel, hide or a new session clears it. Batch providers show their final recognition text after recording rather than implying streaming. Shared pill metrics define the same bounded geometry in both apps.

Meeting notes dominate the detail surface. Notities, Transcript and Vragen stay compact. Infrequent template, regeneration, export and file actions belong in consistent local controls or a contextual menu. Search and cited answers must preserve selection and timestamp navigation.

Keep draft text, selection, scroll position and focused controls stable during model progress, theme updates, permission refresh and navigation. Preserve recording status when users visit another section. A generic success message must not occupy every otherwise empty surface; show acknowledgement after a relevant action.

## Components and acceptance

Every control has clear default, hover, keyboard focus, selected, disabled, loading and error states. A disabled primary button keeps its label readable and does not inherit a secondary hover background. Empty states explain what belongs there and provide a useful first action. Unavailable platform features keep the same control and a short Dutch reason.

Inspect both implementations side by side with deterministic fixtures in light and dark. Use app windows or offscreen rendering, not the user's screen. Cover the workspace routes, all wizard steps and meaningful error/loading states at normal and small window sizes. Hardware capture, focus, clipboard and shortcut checks remain distinct acceptance evidence.
