# Openstaande punten

Stand: 2026-10-24, na fase 3a–3f (alles in `main`).

## 1. Te testen door Ignace (alleen jij kunt dit)

| # | Wat | Waarom |
|---|-----|--------|
| T1 | Meeting met Teams/Meet/Zoom: komen **Ik** en **Anderen** er allebei in? | Microfoonfix voor echo-onderdrukking is nog niet in een echte meeting bevestigd |
| T2 | Na een meeting meteen dicteren | Voice processing wordt nu uitgezet bij stoppen |
| T3 | Speakers zonder koptelefoon: hoort Pennel Teams/Zoom dubbel? | Of Apple's echo-onderdrukking het call-geluid echt wegfiltert |
| T4 | Sprekers herkennen + hernoemen + vraag stellen + kopiëren naar Notion/Slack/Mail | 3b/3e/3f nog niet met echte meeting getest; opgemaakt plakken niet geverifieerd |
| T5 | Commandomodus (fn + ⇧) met en zonder selectie, in Slack, Mail, browser, Terminal | Selectie lezen verschilt per app |
| T6 | AirPods wisselen tijdens meeting, USB-headset, FaceTime, Safari-call | Hardware- en procesvarianten |
| T7 | Lange dictaten opkuisen met je huidige model (`inception/mercury-3`) | Time-out is nu dynamisch (15 s + 1 s per 50 tekens, max 90 s) |

Tip: menu → **Logboek openen** toont wat er misloopt (`~/Library/Logs/Pennel/pennel.log`).

## 2. Bekende beperkingen en open bugs

**Meetings**
- "Anderen"-blokken van 15–30 s krijgen één stem (meerderheid); per beurt hertranscriberen kan later.
- Taal "Automatisch" raadt per blok; kan wisselen naar Engels. Overweeg standaard Nederlands of taal per meeting vastpinnen.
- Sporen "Ik" en "Anderen" lopen niet op één gedeelde klok (kleine verschuivingen mogelijk).
- Stereo-invoer: alleen linkerkanaal.
- Mislukte start laat een lege meeting achter; meetingmap wijzigen tijdens opname breekt afronden.
- Menubalk-icoon blijft rood tijdens samenvatten.
- Samenvatten van de lopende meeting is niet geblokkeerd.
- Opnieuw transcriberen ("Opnieuw proberen") geeft herstelde stukken geen stem.
- Agenda: geweigerde toegang zonder uitleg; overlappende items kiezen het eerste met link; einde van het agenda-item stopt de opname niet.
- Safari-calls worden waarschijnlijk niet herkend (WebKit-proces).

**Dicteren**
- Ontbrekende API-sleutel of opslagfout → opname verloren.
- AirPods die van profiel wisselen tijdens dicteren worden niet opgevangen.
- Zonder Toegankelijkheid faalt plakken stil.
- Grote afbeelding op klembord kan even bevriezen.
- Parakeet ondersteunt geen woordenlijst (alleen via opkuisen).

**Providers & modellen**
- Twee downloads van hetzelfde model tegelijk delen één `.part`.
- Ollama/LM Studio tonen een sleutelveld dat ze niet nodig hebben.
- Oude v1-instellingen kunnen eigen servers samenvoegen.

**App**
- Echte donkere modus (vensters staan nu vast op licht).
- Instellingen worden bij elke toetsaanslag bewaard.
- Sleutelhanger vraagt na elke nieuwe build opnieuw (self-signed); verdwijnt met Developer ID.

## 3. Nog te bouwen

- **3g Microsoft 365-agenda** (Graph, device-code login) — vereist Azure app-registratie, mogelijk AE-admin-goedkeuring.
- **Fase 4 – iPhone:** dicteertoetsenbord, Action Button/Shortcut, fysieke meetings opnemen, sync.
- **Fase 5 – Windows en Linux** (Rust-kern is herbruikbaar; nieuwe shells).
- **Commercialisatie** (zie `docs/business/plan.md`): licentie, Pennel Cloud-backend, accounts/licenties, betalingen, website, onboarding zonder eigen API-sleutels.

## 4. Distributie

- Apple Developer ID (€99/jaar): ondertekenen + notariseren, geen Gatekeeper-waarschuwing, geen sleutelhangervragen per build.
- Automatische updates (Sparkle) en Homebrew cask.
- Publieke repo: licentie, README in het Engels, CONTRIBUTING, screenshots, CI (build + tests).
- Branding: zie `branding/` (logo-varianten, kies er één).
