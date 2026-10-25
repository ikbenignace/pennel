# Pennel brand

Pennel (PEN-el, the same in Dutch and English): the little pen that writes down what you say. Dictation where you type, and meeting notes kept on your computer. One Rust core serves macOS, Windows and Linux.

## Identity

Use the checked-in `wordmark.svg`, `mark.svg`, `app-icon.svg` and `menubar.svg`. The wordmark uses Instrument Serif and a true circular orange dot. The listening indicator uses five bars; movement conveys actual audio level or processing state.

The desktop interface follows the neutral native-app direction approved on 25 October 2026: compact sans-serif UI, restrained controls, a clear sidebar and an uninterrupted onboarding window. Raycast and FluidVoice are craft and interaction references. Pennel retains its own marks, copy and color roles.

## Palette and source of truth

[`tokens.json`](tokens.json) owns semantic colors, spacing, radii and type sizes for both interfaces. Run `python3 scripts/generate-theme.py` from the repository root after editing it. The generated CSS and Swift constants are checked in; `--check` verifies their contents and text contrast, and `--self-test` exercises the generator.

| Role | Light | Dark |
|---|---|---|
| Surface | `#FFFFFF` | `#202022` |
| Window chrome | `#F6F6F8` | `#27272A` |
| Sidebar | `#F1F1F3` | `#252527` |
| Foreground | `#1D1D1F` | `#F3F3F5` |
| Secondary text | `#64646A` | `#A6A6AE` |
| Border | `#E3E3E8` | `#3B3B40` |
| Selected surface | `#E3E3E8` | `#3B3B40` |
| Field | `#FFFFFF` | `#2B2B2F` |
| Primary button | `#252527` | `#EAEAEC` |
| Primary button text | `#FFFFFF` | `#202022` |
| Accent | `#C2410C` | `#F2B48E` |

The artwork's Paper `#FCFBF8`, Ink `#2A2723` and Soft wave `#F2B48E` remain fixed brand colors. The dictation pill uses its own fixed Ink background, Paper text and Soft wave indicator in both appearances. Secondary text, including selected-row metadata, must retain at least 4.5:1 contrast.

Legacy UI names map to semantic roles: `paper` means Surface, `background` means Subtle, `ink` means Foreground and `line` means Border. They adapt with appearance; fixed artwork and pill colors use their dedicated roles.

Rules
- **One accent per surface.** Orange identifies an active state, focus or an error that needs attention. Buttons use the neutral primary-control pairing.
- **Orange never sits on ink.** Dark surfaces use Soft wave. Every dark icon and the pill follow this.
- **No gradients, no shadows in marks, no glows.** Icons are flat.
- The dot is always a true circle, never a font period.

## Typography and geometry

- **Figtree** carries all product headings, body text, buttons and labels. Use Regular for prose, Medium for controls and SemiBold for short headings.
- **Instrument Serif** Regular is for the wordmark and marketing. It is not the product's heading font. Never fake bold or use it in controls.
- Use sentence case. Typical UI sizes are 11 pt captions, 12 pt labels, 13 pt body, 14 pt reading text, 16 pt section headings, 21 pt workspace headings and 26–28 pt onboarding headings.
- Use a 4 pt spacing grid. Controls have 6 pt radii, segmented controls 8 pt, panels and windows 12 pt. Rounded pills are reserved for the recording overlay and compact indicators.
- Keep component state families consistent: default, hover, focus, selected, disabled, loading and error. A disabled primary button keeps a readable label; hover does not replace its background with a pale secondary fill.
- Fonts live in `assets/brand/fonts/` and are bundled locally. Outline artwork text so SVG marks render without installed fonts. The native reference keeps its required bundled resource copies.

## Writing the name

- In running text it is always **Pennel**: capital P, one word, double n, single l. Never "PENNEL", "PenNel", "Pennell" or "Pennel app".
- The lowercase **pennel.** (with the full stop) exists only as the wordmark artwork. Don't type it in sentences.
- Verb use is fine in casual copy: "Just pennel it." / "Even pennellen." Don't overdo it; once per page at most.
- Repo, CLI and URLs: `pennel` (lowercase). Existing package configuration owns bundle identifiers and their preservation across upgrades.

## Naming system

| Name | What it is |
|---|---|
| **Pennel** | The open-source desktop product (Apache-2.0). Local models or your chosen provider. |
| **Pennel Cloud** | Reserved name for a hosted personal service. Availability and features require separate release evidence. |
| **Pennel for Teams** | Reserved name for organisation services. Availability and features require separate release evidence. |

- Avoid "Pennel Pro" or "Plus": it suggests the open-source app is crippled, which hurts trust in an open-core product.
- Feature names are plain nouns in sentence case, written natively per language:

| English | Dutch |
|---|---|
| Dictation | Dicteren |
| Meetings | Vergaderingen |
| Notes | Notities |
| Ask | Vraag |
| Speakers | Sprekers |
| Search | Zoeken |

## Voice and tone

Calm, direct, a bit dry: the way a good colleague writes a meeting recap. Confident about privacy without preaching. Belgian modesty: show, don't shout. Dutch and English are equals; write each one natively, never word for word.

**Do**
- Short sentences, verbs first. "Hold fn. Talk. Let go." / "Hou fn in. Praat. Laat los."
- True, concrete facts: "13 MB", "no account", "on your Mac", "Markdown files in a folder you choose".
- Say where data lives and who sees it.
- Use the reader's words: emails, notes, meetings. Not "AI workflows".
- Dutch: use "je", not "u". Keep English UI terms only where Dutch users really use them (Mac, fn, Markdown).

**Don't**
- No "AI-powered", "revolutionary", "supercharge", "10x", "magic", sparkles, robots, microphone emoji or gradients.
- No perfection claims. "Cleans up", not "flawless".
- No fear-selling about privacy. State the facts and let them land.
- Don't pun on "pen" more than once per page.

**Examples**

| | English | Dutch |
|---|---|---|
| Onboarding | Hold fn and say something. Pennel writes it where your cursor is. | Hou fn in en zeg iets. Pennel schrijft het waar je cursor staat. |
| Privacy | Your recordings stay on your computer. Nothing leaves unless you choose a provider. | Je opnames blijven op je computer. Er gaat niets weg, tenzij je zelf een provider kiest. |
| Error | Couldn't reach your provider. Your recording is saved; try again when you're back online. | Je provider is niet bereikbaar. Je opname is bewaard; probeer opnieuw als je weer online bent. |

## Taglines

| # | English | Dutch |
|---|---|---|
| 1 | *Say it. Full stop.* (lead line) | *Zeg het. Punt uit.* (lead line) |
| 2 | *Speak. It's written.* | *Spreek. Het staat er.* |
| 3 | *Talk in. Text out.* | *Praat erin. Tekst eruit.* |
| 4 | *Your voice, in ink.* | *Je stem, in inkt.* |
| 5 | *Your meetings, written down. On your Mac.* | *Je vergaderingen, uitgeschreven. Op je Mac.* |
| 6 | *Written the way you meant it.* | *Geschreven zoals je het bedoelde.* |

The Dutch lines are written natively, not translated; they don't have to match the English word for word.

## Files

- `tokens.json`: canonical UI roles and dimensions.
- `fonts/`: bundled Figtree and Instrument Serif files and licenses.
- `mark.svg`, `wordmark.svg`, `app-icon.svg`, `menubar.svg`: existing flat identity artwork.
- `scripts/generate-theme.py`: dependency-free CSS/Swift token generator and checks.
- `assets/ui/icons/manifest.json`: compact interface glyph source, emitted as local SVGs and native template-image data.
- `DESIGN.md` at repository root: product composition, interaction and appearance rules.
