# Openstaande punten

Historische native Mac-baseline: 2026-10-24, na fase 3a–3f. De gedeelde Tauri-app voor Mac, Windows en Linux is daarnaast geïmplementeerd; zie [migratie en bewijs](shared-app-migration.md). Onderstaande toestelvragen blijven acceptatiepunten totdat ze met de kandidaatpakketten zijn uitgevoerd.

Update 2026-10-25: de gedeelde app heeft tijdelijke dicteerherstelitems, duurzame meetingchunks, vastgezette opname-instellingen, zichtbare opnamefouten, gedeelde modeldownloads, expliciete providertests en verbeterde bron-/toetsenbordnavigatie. Zie [desktopgedrag](desktop.md) en de [kleine Mac-pilot](improvement-pilot.md). De historische beperkingen hieronder zijn geen actuele buglijst voor de gedeelde app; fysieke toestel-, pakket- en updateacceptatie blijven afzonderlijk nodig.

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
| T8 | Windows 11: echte calls, klembord/plakken, sneltoetsen, echoverwerking en installatie | Cross-checks en unit tests bewijzen geen uitvoering op een Windows-toestel |
| T9 | Linux X11 én Wayland: call-audio, codecs, compositor-sneltoets en klembordfallback | Codec- en synthetische audioservertests bewijzen geen desktop-/toestelgedrag |
| T10 | Bestaande installatie upgraden/herstellen, ondertekende update en een opname van zestig minuten | Vervanging van de native app en publieke uitrol wachten op deze pakketacceptatie |
| T11 | Twee computers, zelfde account: meeting opnemen op de ene, verschijnt op de andere | Synchronisatie is alleen met tests en een gesimuleerde server geverifieerd |
| T12 | Kluis opnieuw beginnen en herstelcode op een tweede toestel | Herstelflow is nog niet op twee echte toestellen doorlopen |
| T13 | Dicteren en samenvatten via Pennel Cloud zonder eigen sleutel | Gehoste modellen zijn nog niet end-to-end op een toestel bevestigd |

Tip: menu → **Logboek openen** toont het log van de actieve build. Canoniek Mac: `~/Library/Logs/Pennel/pennel.log`; preview/Windows/Linux: `logs/pennel.log` in de eigen gegevensmap.

## 2. Bekende beperkingen en open bugs

Deze lijst beschrijft de native referentie en blijft bewaard als regressie-/acceptatielijst. De gedeelde implementatie heeft onder meer tijdgestuurde audio, opname-/mapvergrendeling, providercontrole en toestemmingsmeldingen met gerichte tests. Dat sluit de hieronder vermelde vragen over echte apparaten en apps niet automatisch af.

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

**Pennel Cloud**
- Kluizen die op staging vóór de controlewaarde (key_check) zijn aangemaakt, moeten vóór de release opnieuw beginnen.
- Linux-paden van het kopiëren van de herstelcode zijn nog niet op een Linux-toestel getest.

**App**
- Echte donkere modus (vensters staan nu vast op licht).
- Instellingen worden bij elke toetsaanslag bewaard.
- Sleutelhanger vraagt na elke nieuwe build opnieuw (self-signed); verdwijnt met Developer ID.

## 3. Nog te bouwen

- **3g Microsoft 365-agenda** (Graph, device-code login) — vereist Azure app-registratie, mogelijk AE-admin-goedkeuring.
- **Fase 4 – iPhone:** dicteertoetsenbord, Action Button/Shortcut, fysieke meetings opnemen, sync.
- **Gedeelde desktopapp – Mac, Windows en Linux:** één Tauri-interface en Rust-runtime met native services zijn geïmplementeerd; echte pakket-/toestelacceptatie en publieke uitrol staan nog open. Zie [desktopgedrag](desktop.md) en [Linux-pakketten](linux-packaging.md).
- **Pennel Cloud, fase B en synchronisatie:** inloggen, end-to-end versleutelde meetingsynchronisatie en gehoste modellen zijn gebouwd in de gedeelde app (native referentie: uitgeschakeld tabblad); wachten op toestelacceptatie T11–T13.
- **Commercialisatie** (zie het businessplan in de private repo pennel-cloud): licentie, accounts/licenties, betalingen, website, onboarding zonder eigen API-sleutels.

## 4. Distributie

- Apple Developer ID (€99/jaar): ondertekenen + notariseren, geen Gatekeeper-waarschuwing, geen sleutelhangervragen per build.
- Native referentie: Sparkle. Gedeelde app: Tauri-updater met een legacy-appcast voor de overgang; Linux-self-updates via AppImage, Debian via pakketbeheerder of geverifieerd pakket. Ondertekende installaties en updates moeten nog op kandidaatpakketten worden getest.
- Publieke repo: licentie, README in het Engels, CONTRIBUTING, screenshots, CI (build + tests).
- Branding en gedeelde fonts: zie `assets/brand/brand.md` en `assets/brand/fonts/`.
