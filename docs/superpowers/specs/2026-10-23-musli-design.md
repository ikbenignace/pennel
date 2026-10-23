# Musli — ontwerp

*Werknaam: Musli (naam van de repo). Datum: 23 oktober 2026.*

## 1. Wat en waarom

Musli is één ultralichte app die twee dingen doet die vandaag twee aparte (zware, cloud-only) apps vragen:

1. **Dicteren overal** — sneltoets ingedrukt houden, praten, loslaten → opgekuiste tekst verschijnt in de app waar je cursor staat (Slack, mail, AI-prompts, …). Vervangt Wispr Flow / Handy.
2. **Meetings opnemen en samenvatten** — zonder bot in de call. Je typt eventueel wat steekwoorden; na de call worden die aangevuld tot nette notities, of krijg je een samenvatting als je niets typte. Volledig transcript en audio altijd beschikbaar. Vervangt Granola.

**Waarom:** geen bestaand product combineert beide. Granola en Wispr Flow sturen alles naar hún cloud en providers; Wispr gebruikt ~800 MB RAM in rust. Handy is licht maar kan niet opkuisen en heeft geen meetings. De gebruiker wil **zelf kiezen** waar audio en tekst naartoe gaan, en een app die je niet voelt draaien.

**Succes:** de gebruiker zegt Granola én Wispr Flow op omdat Musli beide beter doet, op zijn Mac, met een app die nauwelijks geheugen gebruikt.

## 2. Doelen, niet-doelen, anti-doelen

**Doelen**
- Mac eerst (prio 1), daarna iPhone, Windows, Linux.
- Provider zelf kiezen voor elke AI-taak (spraak→tekst en LLM), inclusief self-hosted. Lokaal draaien is een optie, geen standaard.
- Super clean, minimal UI, warm en rustig.
- Ultralicht: kleine download, bijna geen RAM/CPU als er niets gebeurt.
- Later open source (eerst privé).

**Niet-doelen (voor nu)**
- Eigen cloud/accountsysteem of betaalmodel.
- Bot die meetings joint.
- Mac App Store-distributie (kan niet: tekst invoegen vereist toegankelijkheid/toetssimulatie buiten de sandbox).

**Anti-doelen — dit maakt het een mislukking, ook als het technisch werkt**
- Data gaat ergens heen behalve naar de provider die de gebruiker koos. Geen telemetrie, geen analytics, geen "even naar onze server".
- Schermcontext of screenshots doorsturen zonder dat de gebruiker dat expliciet per app aanzette (Wispr Flow-incident).
- Opnemen zonder dat de gebruiker het weet of bevestigde.
- Zware app: Electron-achtige footprint, merkbare CPU in rust, honderden MB RAM.
- Druk, generiek "AI-app"-uiterlijk.

## 3. Platformen en fasering

| Fase | Inhoud | Vervangt |
|---|---|---|
| 1 | Mac — dicteren | Handy / Wispr Flow |
| 2 | Mac — meetings | Granola |
| 3 | Extra's: chat met meetings, sprekerherkenning met namen, commandomodus, export/delen, lokale modellen | — |
| 4 | iPhone — dicteertoetsenbord, fysieke meetings opnemen, sync | — |
| 5 | Windows + Linux | — |

Elke fase levert iets op dat de gebruiker dagelijks gebruikt. Fase 1 legt de gedeelde basis (audio, providers, instellingen, opslag) die fase 2 hergebruikt.

## 4. Architectuur (beslist)

**Rust-kern + dunne native schil per platform** ("Ghostty-patroon", met Rust i.p.v. Zig omdat het Rust-ecosysteem voor audio/HTTP/modellen veel rijper is en Zig nog pre-1.0 is).

- **`musli-core` (Rust)**: alles wat niet platform-UI is — provider-clients, audio-chunking en VAD, transcript-samenvoeging, samenvatten/opkuisen-pipeline, opslag en zoekindex, instellingen, optionele lokale modellen (whisper.cpp / Parakeet). Ontsloten via **UniFFI** (genereert Swift en later Kotlin bindings).
- **macOS-schil (Swift, SwiftUI + AppKit waar nodig)**: menubalk-item, vensters, zwevend pilletje (NSPanel), globale sneltoets (event tap), microfoon en systeemaudio opnemen (Core Audio), tekst invoegen, call-detectie, agenda (EventKit), Keychain.
- Later: iOS-schil (SwiftUI + keyboard extension), Windows-schil (WinUI/Win32), Linux-schil (GTK4), allen boven dezelfde kern.

Afgewezen: Electron (te zwaar), Tauri als hoofdpad (webview, minder native gevoel; blijft wel fallback als native schillen per platform te veel werk blijken), Rust-UI-frameworks (GPUI/Iced/Slint: geen native look), Flutter/Compose/Avalonia (zwaarder).

**Doelwaarden (Mac)**: app-download < 25 MB zonder modellen; < 50 MB RAM en ~0% CPU in rust; geen achtergrondprocessen behalve de app zelf.

## 5. Providers (beslist door gebruiker: zelf kiezen)

Drie taken, elk met een eigen provider + model instelbaar:
1. **Dicteren — spraak→tekst**
2. **Meetings — spraak→tekst**
3. **LLM** — opkuisen van dictaten, notities aanvullen, samenvatten (later: chat)

Eén **OpenAI-compatibele client** (configurable base URL + API-sleutel + model) dekt: OpenAI, OpenRouter, Groq, Mistral, Ollama, LM Studio, speaches, whisper.cpp-server. Voorkeuzes (presets) voor deze providers zodat de gebruiker enkel een sleutel plakt. Sleutels in de Keychain.

Bekende eigenschappen om rekening mee te houden:
- **OpenRouter** ondersteunt `/api/v1/audio/transcriptions`, maar ~60 s upstream-timeout en 25 MB per verzoek; `prompt` wordt genegeerd.
- **OpenAI** transcriptie: max 25 MB / ~25 min per verzoek.
- **Groq whisper-large-v3-turbo**: zeer snel en goedkoop (~$0,04/u, minimum 10 s per verzoek) — goede standaard-suggestie voor dicteren.
- Taal: Nederlands en Engels, ook door elkaar. Standaard taal automatisch laten detecteren; gebruiker kan taal vastzetten.

Aparte adapters (Deepgram, ElevenLabs, AssemblyAI) alleen later, als live-streaming of diarization dat vraagt. Lokale modellen (whisper.cpp / Parakeet v3 via de Rust-kern) in fase 3; modellen worden nooit meegebundeld maar bij keuze gedownload.

## 6. Dicteren (fase 1)

**Flow**
1. Sneltoets **ingedrukt houden** = opnemen; loslaten = stoppen. **Dubbeltik** = handenvrij tot je opnieuw tikt. Sneltoets instelbaar.
2. Tijdens opnemen: klein donker **pilletje** onderaan het scherm met bewegend oranje golfje.
3. Na loslaten: audio → spraak→tekst-provider → (in opgekuiste stand) LLM → tekst invoegen in de app met focus.
4. Klembord van de gebruiker wordt na invoegen hersteld.

**Twee standen**
- **Snel**: alleen spraak→tekst, met woordenlijst als hint (`prompt`, waar de provider dat ondersteunt).
- **Opgekuist** (standaard): daarna een snel LLM dat "euh"s, herhalingen en zelfcorrecties ("dinsdag, nee wacht, donderdag" → "donderdag") verwerkt, leestekens/alinea's/opsommingen maakt, en de **stijl van de app** volgt.

**Stijl per app**: op basis van de actieve app (bundle-ID), niet schermcontext. Voorbeelden: Slack/WhatsApp los; Mail/Outlook netjes; AI-tools (Claude, ChatGPT, terminals, IDE's) letterlijk maar opgekuist. Gebruiker kan regels aanpassen. Geen tekst of screenshots van het scherm meesturen.

**Woordenlijst**: namen, jargon, productnamen; gedeeld met meetings; gebruikt als hint voor spraak→tekst en als context voor het LLM.

**Geschiedenis**: recente dictaten blijven bewaard (opnieuw kopiëren als plakken mislukte). Bewaartermijn instelbaar.

**Tekst invoegen**: klembord + gesimuleerde Cmd-V (werkt in vrijwel alle apps), met herstel van het vorige klembord. Toegankelijkheids-API als eerste poging waar betrouwbaar mag de builder zelf beslissen.

**Doel**: van loslaten tot tekst < 1,5 s met Groq in de opgekuiste stand.

## 7. Meetings (fase 2)

**Opnemen**
- Microfoon en systeemaudio als **twee aparte sporen** (systeemaudio via Core Audio process taps, macOS 14.4+; toestemming "Alleen systeemaudio opnemen", geen schermopname-toestemming).
- Twee sporen geven gratis het onderscheid **"Ik" / "Anderen"** in het transcript.
- **Welke systeemaudio** *(mijn keuze)*: bij een gedetecteerde call enkel de audio van die call-app (proces-tap), niet muziek of meldingen. Bij handmatig opnemen alle systeemaudio, met duidelijke opname-indicator.
- **Echo-onderdrukking is vereist** (beslist door gebruiker): hij wisselt tussen AirPods en de Mac-speakers. Met speakers vangt de microfoon ook de anderen op; dat mag niet als dubbele tekst onder "Ik" eindigen. Echo-onderdrukking op het microfoonspoor (met het systeemaudiospoor als referentie) en/of ontdubbelen van overlappende tekst tussen beide sporen. Moet getest worden met beide opstellingen.
- **AirPods**: zodra de microfoon actief is, schakelt macOS AirPods naar een lagere audiokwaliteit. Dat is gedrag van het systeem; Musli moet er niet slechter door werken.
- **Dicteren tijdens een meeting** *(mijn keuze)*: mag. Eén gedeelde microfoonstroom voedt beide; gedicteerde stukken worden uit het meetingtranscript gehouden. Het audiolaagje uit fase 1 houdt hier rekening mee.

**Call-detectie**
- Hoofdsignaal: een andere app gebruikt de microfoon (macOS meldt welke app). Browsercalls (Meet, Teams web): tabtitel via toegankelijkheid.
- Optioneel agenda: titel en deelnemers van de meeting die nu loopt. De gebruiker werkt met **Outlook** als standaard-agenda. Fase 2 *(mijn keuze)*: via de Mac-agenda (EventKit); onboarding legt uit hoe je het werkaccount toevoegt via Systeeminstellingen → Internetaccounts (alleen agenda's) als het daar nog niet zichtbaar is. Rechtstreekse **Microsoft 365-koppeling** (Graph, OAuth) volgt in fase 3 als EventKit niet volstaat; risico: bedrijfs-tenants vragen vaak admin-goedkeuring voor externe apps.
- Call-detectie zelf werkt ook zonder agenda.
- **Einde**: de call-app laat de microfoon > ~30 s los, of agendaeinde + lange stilte, of handmatig stoppen.

**Regels — wat wel/niet opnemen** (beslist door gebruiker)
- Standaard: melding **"Opnemen?"** met *Ja / Nee / Nooit voor deze app*. Opname start pas na *Ja*.
- Regels per app (vragen / altijd / nooit), per agenda (bv. privé nooit), per (terugkerende) meeting.
- Handmatig starten/stoppen kan altijd (menubalk, sneltoets).
- Optie (standaard uit): bij start automatisch een meldingszin in de chat plakken ("Ik neem notities met transcript").

**Tijdens de call**
- Eenvoudig notitievenster; je typt vrije steekwoorden.
- Live transcript loopt mee (bijna-live: audio in stukjes naar de provider), standaard ingeklapt.

**Na de call**
- Eigen notities aanwezig → LLM **vult ze aan** met het transcript (jouw tekst zwart, aanvulling grijs).
- Geen notities → **samenvatting** uit het transcript.
- Altijd secties voor **beslissingen** en **actiepunten** (met persoon waar bekend).
- **Sjablonen** per soort meeting (1-op-1, klantgesprek, standup, …), aanpasbaar.
- Tabs **Notities | Transcript**. Klik op een zin in het transcript → dat stukje audio speelt af.

**Audio**: lokaal bewaard (gecomprimeerd), zodat citaten controleerbaar zijn. Optie om audio na X dagen automatisch te wissen.

**Lange opnames**: knippen op stiltes in stukken die binnen de limieten van de gekozen provider vallen (zie §5), transcripten op tijdstempel samenvoegen.

## 8. Opslag en sync

- **Markdown-bestanden zijn de bron van waarheid** *(mijn keuze)*: de app volgt de map en laadt wijzigingen van buitenaf (bv. Obsidian) opnieuw in. De markering "eigen tekst vs AI-aanvulling" wordt zo bewaard dat ze een bewerking buiten de app overleeft.
- Elke meeting = map met **Markdown** (notities + transcript met tijdstempels) + audio, in een map die de gebruiker kiest (standaard `~/Musli` *(mijn keuze)*, niet Documenten — die synct vaak automatisch naar iCloud). Kiest de gebruiker een map die naar iCloud of een andere cloud synct, dan waarschuwt de app. Leesbaar in Obsidian e.d.
- Dictaatgeschiedenis en instellingen in de app-supportmap.
- Lokale zoekindex (bv. SQLite FTS) is afgeleid en altijd herbouwbaar uit de bestanden.
- **Sync (fase 4)**: via de gekozen map (iCloud Drive, eigen server, S3, …). Geen eigen cloud vereist; een eigen sync-dienst kan later.

## 9. UI en stijl (beslist via mockups)

Referentie-mockup: `.superpowers/brainstorm/*/content/visual-style-v2.html`.

- **Richting**: "warm papier", maar extra clean en minimal.
- **Lettertype**: **Figtree** (UI en tekst); meetingtitels in een zachte serif (Instrument Serif in de mockup).
- **Kleuren**: crème/papier-achtergrond, warme donkere inkt, gedempt grijs voor AI-aanvulling en metadata, één accent (gebrand oranje). Donkere modus in dezelfde geest.
- **Menubalk-menu**: lopende opname bovenaan (met stop), nieuwe notitie, dicteren, volgende meeting, recente meetings, open app, instellingen.
- **Hoofdvenster**: rustige zijbalk (zoeken, meetings per dag/week), notitie groot en luchtig, tabs Notities/Transcript als subtiele segmentknop.
- **Pilletje**: donker, afgerond, oranje golfje, korte status ("Luistert…").
- **Instellingen**: providers per taak, sneltoets, standen, stijl per app, woordenlijst, opnameregels, opslagmap, bewaartermijnen.
- Onboarding: korte stappen voor toestemmingen (microfoon, toegankelijkheid, systeemaudio, optioneel agenda) en eerste provider.

## 10. iPhone (fase 4, richting)

- **Dicteertoetsenbord** (keyboard extension): iOS laat het toetsenbord niet zelf de microfoon gebruiken → zoals Wispr Flow kort naar de hoofdapp om een opnamesessie te starten die enkele minuten op de achtergrond actief blijft. Ook starten via Action Button, Back Tap, snelkoppeling of bedieningspaneel.
- **Meetings**: alleen fysieke meetings via de microfoon (iOS kan geen audio van andere apps opnemen).
- Zelfde Rust-kern, zelfde providers, sync via de gekozen map/cloud.

## 11. Randvoorwaarden

- macOS 14.4+ op Apple Silicon (voor Core Audio process taps).
- **Doelgroep** (beslist door gebruiker): eerst voor hemzelf, uiteindelijk ook voor collega's (en later open source). Onboarding en provider-presets moeten dus ook werken voor iemand anders dan de maker, maar fase 1–2 worden getest op zijn eigen opstelling.
- **Geen betaald Apple Developer-account** (nu): fase 1–2 draaien als lokaal ad-hoc gesigneerde build op zijn eigen Mac. Delen met collega's vraagt een Developer ID (€99/jaar) voor signeren, notariseren en automatische updates — dat wordt een eigen stap zodra hij wil delen. Zonder notarisatie moeten collega's de app handmatig toestaan; geen goede ervaring.
- Distributie buiten de App Store.
- Toestemmingen: microfoon, toegankelijkheid (invoegen + sneltoets), systeemaudio-opname, optioneel agenda.
- Repo eerst privé; later open source (licentie dan kiezen).

## 12. Gekende risico's

- **Echo bij luidsprekers**: vereist (zie §7); grootste technische risico van fase 2 — vroeg prototypen.
- **Agenda in bedrijfsomgeving**: Outlook-agenda niet altijd zichtbaar in de Mac-agenda; Microsoft 365-koppeling kan admin-goedkeuring vragen.
- **Toestemming systeemaudio** kan niet vooraf gecontroleerd worden via een publieke API → onboarding moet dat opvangen.
- **Fn-toets** als sneltoets werkt alleen op Apple-toetsenborden.
- **Providerlimieten** (timeouts, grootte) verschillen → chunking per provider configureren.
- **Kwaliteit Nederlands** verschilt per model → testen met echte Nederlandse meetings.

## 13. Overgelaten aan de builder

Projectstructuur en modules, exacte crates/libraries, audioformaat en chunkgroottes, prompt-ontwerp voor opkuisen/samenvatten, standaard-sneltoets, update-mechanisme, testaanpak, details van het Markdown-formaat, naam/branding definitief.
