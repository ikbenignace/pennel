# Pennel brand

Pennel (PEN-el, the same in Dutch and English): the little pen that writes down what you say. Push-to-talk dictation anywhere you type, and meeting notes that stay on your Mac. Open source, with a paid Pennel Cloud.

## Recommendation

**Finalize 01 Full Stop for the identity, and use 03 Ink Pulse for the menubar and the listening state.**

- **Wordmark, app icon, website, docs:** 01 Full Stop. "pennel." in Instrument Serif with the orange full stop; the "p." icon on ink. It keeps everything the Musli round got right and fits the new name better: a pen's name ending in the mark a pen makes. "Zeg het. Punt uit." still works in both languages.
- **Menubar icon:** `03-ink-pulse/menubar.svg`. Five 2 pt bars on a 4 pt pitch, hanging from one line and tapering into a nib, with the dot at the tip. It is the old Pulse glyph the owner liked, now with a pen in it.
- **Listening pill:** the same five bars, animated while you talk. When you let go they settle into the nib shape and the dot appears: "written, full stop". That small animation is the brand moment.
- **Runner-up:** 02 Nib, if a pictorial symbol is preferred over a typographic one. It is the most ownable mark of the six and the strongest Dock icon.

Before final artwork: register `pennel.app` (and `pennel.ai`, `getpennel.com`) and run a EUIPO/BOIP and USPTO search for classes 9 and 42 (see `../naming-research.md`).

## Palette

| Token | Hex | Use |
|---|---|---|
| Paper | `#FCFBF8` | Cards, sheets, light icons, text on ink |
| Background | `#F7F4EE` | Page and window background ("oat") |
| Ink | `#2A2723` | Text, marks, dark icons, the pill |
| Muted text | `#7A7268` | Secondary text that must be readable (4.6:1 on paper, AA) |
| Line | `#ECE7DE` | Hairlines, borders, the outline of light icons |
| Accent: burnt orange | `#C2410C` | The full stop, the one thing that matters on a surface (5.0:1 on paper) |
| Soft wave | `#F2B48E` | The accent on dark surfaces (8.3:1 on ink) |

The old Muted `#9B948A` is retired for text. Keep it, if at all, for decorative strokes only.

Rules
- **One accent per surface.** Orange marks one thing: the dot, the active state, the primary highlight. Never large text blocks.
- **Orange never sits on ink** (2.9:1). On dark surfaces use Soft wave. Every dark icon and the pill follow this.
- **No gradients, no shadows in marks, no glows.** Icons are flat.
- The dot is always a true circle, never a font period.

## Typography

- **Instrument Serif** (Regular): headlines, the wordmark, marketing, pull quotes. Use italic for the second-language line or an aside. Never bold, never all caps.
- **Figtree**: interface, body, buttons, labels. Regular for body, Medium for UI labels, SemiBold for buttons and small headings.
- Sentence case everywhere. Small tracked caps only for labels (11 px, +8% tracking, SemiBold, Muted text).
- Headlines tight (line-height 1.0 to 1.1, tracking -1%); body 15 to 17 px at 1.5.
- Shared interface and artwork fonts live in `assets/brand/fonts/`; outline artwork text so SVGs render without installed fonts. The native reference keeps its required bundled resource copies.

## Writing the name

- In running text it is always **Pennel**: capital P, one word, double n, single l. Never "PENNEL", "PenNel", "Pennell" or "Pennel app".
- The lowercase **pennel.** (with the full stop) exists only as the wordmark artwork. Don't type it in sentences.
- Verb use is fine in casual copy: "Just pennel it." / "Even pennellen." Don't overdo it; once per page at most.
- Repo, CLI, bundle and URLs: `pennel` (lowercase). Bundle id: `app.pennel.mac`.

## Naming system

| Name | What it is |
|---|---|
| **Pennel** | The free, open-source Mac app (Apache-2.0). Local models or your own provider. |
| **Pennel Cloud** | The paid subscription: hosted models with no API keys, sync, and backups. One personal plan. |
| **Pennel for Teams** | Pennel Cloud for organisations: shared meeting libraries, admin, billing, data-residency options. |

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
| Privacy | Your recordings stay on your Mac. Nothing leaves unless you choose a provider. | Je opnames blijven op je Mac. Er gaat niets weg, tenzij je zelf een provider kiest. |
| Error | Couldn't reach your provider. Your recording is saved; try again when you're back online. | Je provider is niet bereikbaar. Je opname is bewaard; probeer opnieuw als je weer online bent. |
| Upgrade | Pennel Cloud: no API keys, nothing to set up. | Pennel Cloud: geen API-sleutels, niets in te stellen. |

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

- `index.html` and `preview-board.png`: the board with all six directions.
- `NN-name/`: `mark.svg`, `wordmark.svg`, `app-icon.svg` (1024 master), `menubar.svg` (18 pt template), `notes.md`.
- `tools/make_assets.py` generates every SVG from code (shared stroke weights, outlined text). `tools/build_board.py` rebuilds the board; add `--png` to render the preview with headless Chrome.
