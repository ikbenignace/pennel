# Musli fase 3 — ontwerp

Ignace vroeg om fase 3 te starten zonder verdere vragen, plus een modellenlijst "zoals Handy": snelheid, nauwkeurigheid en ondersteunde talen per model. Alle keuzes hieronder zijn mijn eigen calls, tenzij anders vermeld.

## Waarom

Musli werkt (fase 1–2b). Fase 3 maakt het slimmer en lokaler: betere lokale modellen kiezen op basis van echte cijfers, meetings bevragen en hergebruiken, en sprekers uit elkaar houden.

## Onderdelen en volgorde

Elk onderdeel krijgt een eigen branch, gestapeld op de vorige, met een eigen plan en review.

| # | Onderdeel | Kern |
|---|-----------|------|
| 3a | Lokale engine + modellencatalogus | whisper-rs → `transcribe-cpp`; Handy-catalogus met benchmarks; Parakeet e.a.; betere cloud-modelkiezer |
| 3b | Zoeken + chatten met meetings | SQLite FTS5-index over transcripten/notities; vraag-antwoord over één meeting of alle meetings met bronverwijzingen |
| 3c | Eigen samenvattingssjablonen | Sjablonen als Markdown-bestanden in `~/Musli/.templates`, bewerkbaar in de app |
| 3d | Commandomodus | Tekst selecteren + sneltoets + gesproken opdracht ("maak formeler") → geselecteerde tekst vervangen |
| 3e | Exporteren | Kopieer als Markdown/rich text, mail via `mailto:`/Mail, delen via macOS share sheet (Slack, Notion, …) |
| 3f | Sprekers | Diarization met Sortformer via `transcribe-cpp` op het "Anderen"-spoor; sprekers hernoemen, namen uit agenda-deelnemers voorstellen |
| 3g | Microsoft 365-agenda | Graph API met device-code login; alleen als EventKit/Outlook-sync tekortschiet — vereist een Azure app-registratie, dus pas na overleg |

Niet in fase 3: een gebundelde lokale LLM (llama.cpp). Lokaal opkuisen/samenvatten kan al via Ollama of LM Studio als provider; een eigen LLM-runtime maakt de app zwaar en botst met "ultralicht". Wel: Ollama/LM Studio als duidelijk aanbevolen lokale optie tonen.

## 3a in detail

### Engine

- `whisper-rs` wordt vervangen door `transcribe-cpp` (MIT, Metal, statisch gelinkt). Eén engine voor Whisper (GGUF én oude whisper.cpp `.bin`), Parakeet, Canary, Qwen3-ASR, Moonshine, Voxtral, Cohere, … Later ook VAD, taalherkenning en diarization (3f).
- `LocalWhisper` wordt `LocalEngine`: zelfde gedrag (laden bij eerste gebruik, vrijgeven na inactiviteit), taal en woordenlijst doorgeven waar het model dat ondersteunt (`Feature::Vocabulary`, `ContextPrompt`), anders negeren.
- Na de wissel: app-grootte en idle-geheugen opnieuw meten. Doel: app < 15 MB, idle < 50 MB.

### Catalogus

- `catalog.json` uit npm-pakket `@handy-computer/models` (MIT, versie vastgezet) wordt in de Rust-kern meegebakken (`include_str!`). Geen netwerk nodig om de lijst te tonen. Een script `scripts/update-catalog.sh` haalt een nieuwe versie op.
- Alleen modellen met rol `asr` en een default-bestand in de root van de repo worden aangeboden. Download van `https://huggingface.co/{repository}/resolve/{revision}/{filename}`, sha256 uit de catalogus, bestaande hervatbare download-code.
- Per model berekent de kern:
  - **Snelheid** 0–100: `100·(1 − e^(−rtf/8))` met `performance.accelerated` (anders `cpu`; geen meting → 10). Zelfde formule als Handy, zodat cijfers vergelijkbaar zijn.
  - **Nauwkeurigheid** 0–100 voor een gevraagde taal: `100·e^(−error/15)` uit `results.accuracy[taal]`; niet gemeten → `None` (UI toont "niet gemeten").
  - **Talen**: lijst ISO-codes; UI toont aantal + uitklapbare lijst met Nederlandse taalnamen.
  - Grootte, parameters, licentie, mogelijkheden (taaldetectie, timestamps).
- Aanbevolen selectie (mijn call, afgestemd op Nederlands + Engels): Parakeet TDT 0.6B v3, Whisper large-v3 turbo, Cohere Transcribe, Canary 1B v2, Whisper small. Rest onder "Alle modellen".
- Bestaande downloads (`whisper-large-v3-turbo-q5_0`, `whisper-small-q5_1`, `.bin`) blijven werken en blijven in de lijst als "Al gedownload", met de cijfers van hun catalogus-tegenhanger. Instellingen die ernaar verwijzen blijven geldig.

### Modellenscherm (Providers → Lokaal)

- Bovenaan: taalkiezer "Nauwkeurigheid voor" (standaard de dicteertaal uit Instellingen, anders Nederlands) en schakelaar "Alleen modellen die deze taal ondersteunen" (standaard aan).
- Per model een kaart: naam, korte beschrijving, grootte, twee balkjes (snelheid, nauwkeurigheid in gekozen taal), "N talen" met uitklap, badge "Aanbevolen", knop Download/Gebruik/Verwijder zoals nu.
- Sorteren: aanbevolen eerst, daarna nauwkeurigheid in gekozen taal.
- Verwijderen van een model in gebruik vraagt bevestiging (deferred minor uit fase 1 meegenomen).

### Cloud-modelkiezer

- De bestaande "Kies"-knop wordt een doorzoekbare lijst. Voor OpenRouter (`/api/v1/models`) tonen we prijs per miljoen tokens in/uit en contextlengte, en filteren we op taak (audio-invoer voor spraak, tekst voor opkuisen).
- `openrouter/free` en andere routers krijgen een waarschuwing: "kiest per aanvraag een willekeurig model — opkuisen wordt onbetrouwbaar".

### Tests

- Rust: catalogus parsen, score-formules, taalfilter, legacy-ids, download-URL; engine laadt ontbrekend model → `ModelNotDownloaded`; genegeerde echte-model-test met Parakeet en de bestaande `hello.wav`.
- Swift: sortering/filtering in een pure `ModelCatalogFilter` in MusliKit.

## Anti-doelen

- Geen telemetrie, geen account, geen netwerk om de lijst te tonen.
- App mag niet zwaar worden: geen ONNX runtime, geen Python, geen gebundelde LLM.
