# Directe transcriptie — tekst terwijl je spreekt (ontwerp)

Stand: 2026-10-25. Repo's: `musli` (kern, gedeelde app, native Mac-referentieapp) en `pennel-cloud` (server, alleen §6).

## 1. Wat en waarom

Vandaag zie je tijdens het dicteren alleen "Luistert…" en een golfje. Pas na het loslaten komt de hele opname in één keer bij het model, daarna volgen opkuisen en plakken. Bij een lange zin weet je dus pas op het einde of Pennel je goed verstond. Bij meetings komt het live transcript per fragment van 15 tot 30 seconden binnen (`core/src/chunker.rs`).

"Directe transcriptie" betekent dat de tekst verschijnt terwijl je spreekt:

- **Dicteren:** de pill toont live wat Pennel hoort. Na het loslaten wordt nog altijd **één keer** opgekuiste tekst geplakt, en dat gebeurt sneller omdat het meeste werk al tijdens het spreken gedaan is.
- **Meetings:** het live-venster toont onderaan per spreker een voorlopige regel, die verdwijnt zodra het definitieve fragment binnen is.

Niet elk model kan streamen. Elk model krijgt daarom de beste methode die technisch kan:

1. echte streaming;
2. als het model dat niet kan: snel opnieuw transcriberen van de lopende frase, met spraakdetectie;
3. als ook dat niet zinnig is: per frase;
4. anders staat de bediening **uitgeschakeld met een korte Nederlandse reden**, volgens de pariteitsregel in `CLAUDE.md`.

**Klaar als:** met het standaardmodel van elke providersoort verschijnt de eerste tekst in de pill binnen de latentiedoelen van §5.3, het geplakte resultaat is niet slechter dan vandaag, en geen gedeeltelijke tekst komt ooit in een log, de geschiedenis of een foutmelding terecht.

## 2. Doelen, niet-doelen, anti-doelen

**Doelen**
- Live tekst in de pill tijdens dicteren en opdrachten (Shift), voor elke provider en elk model waarvoor het kan.
- Kortere wachttijd tussen loslaten en plakken. Bij streaming en frasen is het grootste deel van de audio al getranscribeerd bij het loslaten.
- Een voorlopige regel in het live meetingvenster voor lokale modellen en Apple-dicteren.
- Eén capaciteitsfunctie in de kern die per gekozen model zegt welke methode geldt en waarom. Beide UI's tonen hetzelfde.
- Pennel Cloud krijgt een realtime-endpoint, zodat de toekomstige standaardprovider ook echt live is (§6).

**Niet-doelen (v1)**
- Tekst live in de doel-app typen, terwijl je spreekt. Zie het anti-doel hieronder en §5.1.
- Opkuisen tijdens het spreken (speculatieve cleanup). Dat zou dubbele LLM-kosten vragen en kan tekst herschrijven die al op het scherm staat.
- Meetings via realtime cloud-sockets als bron van het transcript. Dat komt als fase 4, alleen als Ignace ja zegt (§9, vraag 4).
- Een realtime-verbinding naar "Eigen server", Ollama of LM Studio. Daar bestaat geen gedeeld protocol voor.
- Live sprekerherkenning. Diarisatie blijft nabewerking na de meeting.

**Anti-doelen (ook als het technisch werkt, is het dan mislukt)**
- Gedeeltelijke tekst komt in logs, diagnostiek, de dictaatgeschiedenis, crashrapporten of foutteksten terecht.
- Er wordt meer dan één keer geplakt per opname, of de doel-app krijgt tekst die later nog verandert.
- Het resultaat is slechter omdat er live getoond wordt. Faalt de live-weg ergens, dan transcribeert Pennel de volledige opname zoals vandaag. Er gaat geen audio verloren.
- De laptop wordt merkbaar heet of traag door een voorbeeld dat niemand vroeg. De rekenbudgetten in §5.4 zijn harde grenzen.
- Stilletjes dubbel betalen: één opname wordt niet zowel live als in batch naar dezelfde betaalde provider gestuurd.

## 3. Hoe het vandaag werkt (vertrekpunt)

- **Dicteren:**
  - De app neemt alles op in `Recorder` (`apps/desktop/src-tauri/src/audio.rs`; Mac: `AudioRecorder.swift`).
  - Bij het loslaten roept de app `PennelEngine::dictate` aan (`core/src/engine.rs`).
  - `dictate` doet eerst `has_speech`. Daarna transcribeert `stt()` via een van drie wegen:
    - lokaal via `LocalEngine::transcribe` (`core/src/local_engine.rs`, transcribe-cpp);
    - Apple via de host-trait `SystemTranscriber` (`core/src/system_stt.rs`, in Swift `DictationTranscriber` met preset `.longDictation`, alleen eindresultaten);
    - cloud via multipart `POST /audio/transcriptions` (`core/src/stt.rs`).
  - Daarna filtert `is_hallucination` het resultaat, volgt cleanup in de modus Clean (`core/src/cleanup.rs`) en komt het dictaat in de geschiedenis.
  - Eén plakopdracht gaat via de geordende plakwachtrij (`dictation.rs`, `paste_worker`).
- **Meetings:**
  - `MeetingSession` (`core/src/session.rs`) gebruikt per spreker (Ik / Anderen) een `Chunker` van 15–30 s die bij een pauze knipt.
  - Eén worker-thread transcribeert de fragmenten na elkaar.
  - De live-weergave pollt `session.transcript()`: `meeting_status` in de desktop-app, `liveTranscript` in `MeetingController.swift`.
- **Lokale modellen:**
  - transcribe-cpp 0.4.1 heeft al een streaming-API (`Session::stream` → `Stream::feed/finalize/text()`), met `StreamText { committed, tentative }`.
  - Die API weigert met `NOT_IMPLEMENTED` als `capabilities.supports_streaming` false is.
  - Per model kan er maar één stream tegelijk actief zijn, en zolang die loopt geeft `run()` `Busy` terug.
  - `catalog.json` bevat per model `capabilities.streaming`, maar `catalog.rs` leest dat veld nog niet in.
- **Standaardinstellingen** (`default_settings`): Groq `whisper-large-v3-turbo` voor dicteren en meetings. De standaardgebruiker heeft dus vandaag geen streamend model.
- **Pill:** venster van 420×72, toont alleen een label en het niveau (`pill.rs`, `pill.ts`; Mac: `PillView.swift`).

## 4. Capaciteitsmatrix

Methodes:
- **Stream**: echte streaming met vaste en voorlopige tekst.
- **Venster**: lokaal opnieuw decoderen van de lopende frase, ongeveer elke seconde, binnen het budget van §5.4.
- **Frasen**: elke frase die door spraakdetectie wordt afgesloten, gaat één keer als batchverzoek weg.
- **Fragment**: zoals vandaag, zonder voorbeeld.
- **Uit**: de bediening is zichtbaar maar uitgeschakeld, met een reden.

| Provider | Model(len) | Dicteren | Meeting (live-venster) | Streamingmethode en bron | Terugval |
|---|---|---|---|---|---|
| Lokaal | Nemotron Streaming 3.5 (`nemotron-3.5-asr-streaming-0.6b`, o.a. nl-NL) | **Stream** | Venster op het open fragment | transcribe-cpp `Session::stream`, `supports_streaming` (catalogus: `streaming: true`) | Venster, en bij een fout de hele opname in één keer |
| Lokaal | Parakeet Unified EN, Nemotron Speech Streaming EN, Moonshine Streaming (tiny/small/medium, niet gecureerd) | **Stream** (alleen Engels) | Venster | idem | idem |
| Lokaal | Voxtral Mini 4B Realtime (nl) | **Stream** als de vorige take sneller dan realtime liep; anders Venster | Venster | idem. 4,4B parameters: zwaar op een CPU | Venster of Fragment |
| Lokaal | Multitalker Parakeet (meetingmodel) | Stream (technisch kan het; het model is niet bedoeld voor solo-dictaat) | Venster | idem | — |
| Lokaal | Parakeet TDT v3/v2, Canary, Cohere, Qwen3-ASR, Granite, GigaAM, SenseVoice, Fun-ASR, Voxtral 3B, Moonshine | **Venster** | Venster | geen streaming in transcribe-cpp → `run()` op de open frase (≤ 15 s) | Fragment |
| Lokaal | Whisper (alle maten, incl. Breeze) | **Venster**, adaptief; zonder GPU bij large-v3 meestal enkel bij pauzes | Venster, adaptief | whisper vult elk venster aan tot 30 s, dus elke decodering kost evenveel; zoals het `stream`-voorbeeld van whisper.cpp | Frasen bij pauzes, dan Fragment |
| Apple-dicteren (macOS 26+) | systeemmodel | **Stream** | Venster (via `SystemTranscriber`), alleen als één aanroep < 1 s duurt | SpeechAnalyzer + `DictationTranscriber` met voorlopige resultaten, audio via `AsyncStream<AnalyzerInput>` | Hele opname via de huidige `transcribe` |
| Apple-dicteren op Windows/Linux | — | **Uit** (bestaande reden: vereist macOS 26) | Uit | — | — |
| OpenAI | live: `gpt-live-transcribe` | **Stream** (realtime WebSocket) | Fragment (batchmodel zoals vandaag) | `wss://api.openai.com/v1/realtime?intent=transcription`, sessie `type: "transcription"`, events `…input_audio_transcription.delta/.completed`, PCM 24 kHz; geen server-VAD → de client commit per frase | Frasen met het gekozen batchmodel |
| OpenAI | batch: `gpt-transcribe`, `gpt-4o(-mini)-transcribe`, `whisper-1` | Frasen (als live uit staat of als het realtime-model geweigerd wordt) | Fragment | `stream=true` streamt alleen de uitvoer van een *afgerond* bestand; `whisper-1` streamt niet | Fragment |
| Mistral | live: `voxtral-mini-transcribe-realtime-*` | **Stream** (realtime WebSocket) | Fragment | `wss://api.mistral.ai/v1/audio/transcriptions/realtime?model=…`, `session.update` → `input_audio.append` (base64 pcm_s16le 16 kHz) → `input_audio.end`; events `transcription.text.delta`, `transcription.done`; `target_streaming_delay_ms` 80–1200 of 2400, standaard 480 | Frasen met `voxtral-mini-latest` |
| Groq | `whisper-large-v3-turbo`, `whisper-large-v3` | **Frasen** (≥ 4 s per frase) | Fragment | geen streaming of WebSocket; Groq rekent minstens 10 s per verzoek | Fragment (bij 429: geen voorbeeld meer voor de rest van de opname) |
| OpenRouter | bv. `openai/whisper-large-v3-turbo` | **Frasen** | Fragment | alleen batch `POST /api/v1/audio/transcriptions` | Fragment |
| Eigen server (OpenAI-compatibel) | wat de server biedt | **Frasen** | Fragment | batch `/audio/transcriptions` | Fragment |
| Ollama, LM Studio | — | Frasen *als* de server `/audio/transcriptions` aanbiedt; standaard heeft die geen STT | Fragment | geen STT-endpoint in de standaardinstallatie | — |
| Pennel Cloud (in aanbouw) | `pennel-auto` | **Stream** via het nieuwe `wss://…/v1/audio/transcriptions/realtime` (§6); tot dan Frasen | Fragment | de server proxyt naar Mistral realtime (EU) | Frasen via `POST /v1/audio/transcriptions` |

Waarom meetings met lokale streamingmodellen toch "Venster" krijgen:
- transcribe-cpp staat per geladen model één actieve stream toe en blokkeert `run()` zolang die loopt.
- Een meeting heeft twee sporen (Ik en Anderen), en Pennel moet tijdens de meeting ook kunnen dicteren.
- Twee streams vragen het model twee keer in het geheugen (bij Voxtral 4B onaanvaardbaar).
- Een voorbeeld per 3–5 s is voor een meeting ruim genoeg.

Waarom cloud-meetings v1 geen voorbeeld krijgen: elke voorbeeldaanvraag is een extra betaald verzoek bovenop het fragment. Bij Groq kost elk verzoek minstens 10 s.

Terloops gevonden: OpenAI markeert `gpt-4o-mini-transcribe` (Pennels standaard voor OpenAI in `provider.rs`) als verouderd, met uitschakeling op 2027-02-26. Dat staat los van dit ontwerp, maar `default_model` moet vóór die datum naar `gpt-transcribe`.

Bronnen:
- OpenAI realtime transcription: https://developers.openai.com/api/docs/guides/realtime-transcription
- OpenAI speech-to-text en streaming: https://developers.openai.com/api/docs/guides/speech-to-text
- Mistral realtime: https://docs.mistral.ai/studio/audio/speech-to-text/transcribe-in-realtime
- Groq speech-to-text (minimum 10 s, geen streaming): https://console.groq.com/docs/speech-to-text
- OpenRouter STT: https://openrouter.ai/docs/guides/overview/multimodal/stt
- Apple SpeechAnalyzer / DictationTranscriber: https://developer.apple.com/documentation/speech/dictationtranscriber
- whisper.cpp stream-voorbeeld: https://github.com/ggml-org/whisper.cpp/tree/master/examples/stream
- transcribe-cpp 0.4.1: `src/session.rs` (`Stream`), `include/transcribe.h` (`transcribe_stream_begin`: `NOT_IMPLEMENTED` zonder `supports_streaming`, één stream per model)

## 5. UX

### 5.1 Dicteren
- **De pill groeit tot een tekstballon:**
  - Links staat het golfje. Rechts staan maximaal twee regels: de laatste ±120 tekens van het transcript, beginnend aan een woordgrens met "…".
  - Breedte maximaal 560 px. De ballon groeit naar boven, zodat de onderrand vast blijft.
  - Vaste tekst staat in `--paper`, voorlopige tekst in `--paper` met 60 % dekking. Er komen geen nieuwe kleuren of maten bij: alle waarden komen uit `assets/brand/brand.md`.
  - Zolang er nog niets herkend is, blijft het label "Luistert…".
- **Na het loslaten:**
  - De laatste tekst blijft staan en het golfje wordt een spinner.
  - In de modus Clean staat het label "Opkuisen…". In de modus Fast wordt meteen geplakt.
  - Daarna verdwijnt de pill zoals vandaag.
  - De ballon toont nooit de opgekuiste tekst. Die komt alleen in de doel-app terecht.
- **Opdrachtmodus (Shift):** dezelfde ballon met de gesproken instructie. Het label "Opdracht…" blijft als voorvoegsel.
- **Plakken:**
  - Er wordt exact één keer geplakt, met de opgekuiste eindtekst, via de bestaande plakwachtrij.
  - Een voorbeeld dat na het afronden nog binnenkomt, wordt weggegooid (volgnummer per opname).
- **Niet live in de doel-app typen**, om drie redenen:
  - Het werkt niet betrouwbaar in elke app.
  - Het vervuilt de ongedaanmaakgeschiedenis.
  - Het botst met cleanup en met de geordende plakwachtrij.
- **Instelling** in Dicteren: schakelaar **"Toon tekst terwijl je spreekt"** (standaard aan), met eronder een regel uit `live_capability`, bijvoorbeeld:
  - "Live via Nemotron Streaming 3.5."
  - "Per zin via Groq."
  - "Niet beschikbaar: Apple-dicteren vraagt macOS 26 of nieuwer."

  Daaronder een vaste hint: "De tekst is zichtbaar voor wie je scherm ziet."
- **Schermdeling:** waar het OS dat toelaat, wordt de pill uitgesloten van schermopnames:
  - Windows: `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)`.
  - macOS: `sharingType = .none`. Dat wordt niet door elke opnamemethode gerespecteerd.

  We beloven dit nergens, daarom staat de hint er.

### 5.2 Meetings
- Onder het live transcript staat per actieve spreker één grijze cursieve regel, bijvoorbeeld "Anderen: …". Dat is de voorlopige tekst van het fragment dat nog open is.
- Komt het definitieve fragment binnen, dan verdwijnt de regel. Alleen definitieve fragmenten gaan naar `transcript.md`.
- Bij providers zonder voorbeeld staat op dezelfde plek één regel: **"Tekst verschijnt per fragment van 15 tot 30 seconden."**
- De schakelaar uit §5.1 geldt ook voor meetings. Er is geen tweede instelling.

### 5.3 Latentiedoelen

Gemeten op de referentietoestellen uit `catalog.json` (Ryzen 4750U, CPU en Vulkan) en op een M1.

| Methode | Eerste tekst na begin van spraak | Bijwerken | Loslaten → plakken (Fast) |
|---|---|---|---|
| Stream lokaal of Apple | ≤ 700 ms | 200–500 ms | ≤ 400 ms |
| Stream cloud (OpenAI, Mistral, Pennel Cloud) | ≤ 1 s | volgens de provider | ≤ 800 ms |
| Venster lokaal | ≤ 1,5 s | ~1 s (adaptief) | decodeertijd van de laatste frase (≤ 15 s audio) |
| Frasen cloud | eerste pauze + roundtrip (~2–5 s) | per frase | roundtrip van de laatste frase |

In de modus Clean komt de duur van de cleanup daar onveranderd bij.

### 5.4 Reken- en batterijbudget
- Nooit meer dan één decodering tegelijk per dictaat.
- Venster:
  - De volgende decodering start pas na `max(1 s, 2 × de vorige decodeertijd)`. Zo blijft de belasting onder ±50 % van wat het model kan.
  - Duurt één decodering langer dan de audio die ze dekt, dan gaat het voorbeeld voor de rest van de opname over naar "alleen bij pauzes".
- Meetingvoorbeeld:
  - Alleen als de meeting-worker niets in de wachtrij heeft.
  - Hoogstens elke 3 s per spreker.
  - Nooit als er tegelijk gedicteerd wordt.
- Modellen blijven geladen volgens de bestaande ontlaadregel (`local_unload_minutes`).
- Een lokale stream houdt het model vast zolang de opname duurt. Meetingfragmenten wachten zolang in de wachtrij. Ze gaan niet verloren.

### 5.5 Privacy
- Gedeeltelijke tekst bestaat alleen in het geheugen van de kern, in het Tauri-event naar het pillvenster (`emit_to("pill", …)`, nooit een broadcast) en in het geheugen van de webview.
- Logs bevatten alleen:
  - de tijd tot de eerste tekst;
  - het aantal updates;
  - de gekozen methode;
  - de reden van een terugval (als foutcategorie, nooit als providertekst).
- Live audio gaat alleen naar de provider die de gebruiker voor dicteren koos. Er is geen extra provider voor het voorbeeld.
- API-sleutels gaan in de `Authorization`-header van de WebSocket-handshake, nooit in de URL.

## 6. Pennel Cloud: realtime-endpoint (aparte servertaak)

Ja, de proxy uit de meeting-sync-spec (§3.6) heeft een streaming-endpoint nodig: `POST /v1/audio/transcriptions` is batch-only. Pennel Cloud wordt de standaard voor nieuwe gebruikers en verwerkt in de EU via Mistral. Zonder dit endpoint heeft de standaardervaring alleen "Frasen".

- **Route:** `GET /v1/audio/transcriptions/realtime` met een WebSocket-upgrade en `Authorization: Bearer <access token>` (§3.1 van de sync-spec).
- **Protocol:**
  - Het protocol is gelijk aan dat van Mistral, zodat de kern één adapter heeft voor Mistral en voor Pennel Cloud.
  - Toegelaten client-berichten: `session.update` (alleen `language` en `context_bias`), `input_audio.append` en `input_audio.end`.
  - De server forceert `pcm_s16le` 16 kHz, `target_streaming_delay_ms` 480 en geen diarisatie.
  - Het upstream-model komt uit de env (`PENNEL_STT_REALTIME_MODEL`). De app stuurt geen model.
- **Opname in het verbruik:**
  - Seconden = ontvangen audiobytes / 32 000, naar boven afgerond.
  - Ze tellen mee in de bestaande audiolimiet (`usage_daily.audio_seconds`).
  - Ze worden ook geboekt als de client wegvalt (zoals `detached`).
- **Grenzen:**
  - Een sessie neemt een van de 2 gelijktijdige verzoeken in, en een audio-permit.
  - Maximaal 15 minuten audio per sessie (env).
  - Na 30 s zonder audio sluit de server.
  - De quota en het dagplafond worden gecontroleerd bij de upgrade (`admit`) en opnieuw elke minuut.
  - Bij een overschrijding stuurt de server een `error`-event met de bestaande Nederlandse tekst en sluit hij.
- **Logs:** alleen account-id, duur, bytes en sluitreden. Nooit tekst of events.
- **Aanpassing in de app:** de providerkaart toont "Live" zodra de server `realtime: true` meldt in `GET /v1/account`, of via een capability-endpoint. Tot dan gebruikt de app "Frasen".

## 7. Beslissingen

1. **Eén abstractie in de kern** — *mijn voorstel*. `LiveDictation` (uniffi-object) krijgt audio in stukken van de host en meldt `LivePartial { committed, tentative, seq }` via een host-trait `LiveListener`. `finish()` levert het gewone `DictationResult`, via dezelfde hallucinatiefilter, cleanup en geschiedenis als `dictate`. De oude `dictate` blijft bestaan voor herproberen en als terugval.
2. **De volledige audio blijft altijd bewaard.** Faalt de live-weg (socket, model, 429), dan transcribeert `finish()` de hele opname via het bestaande `stt()`. De eindtekst is dus nooit slechter dan vandaag — *mijn voorstel*.
3. **Eindtekst per methode** — *mijn voorstel*:
   - Stream: de gefinaliseerde streamtekst; er volgt geen tweede pass.
   - Venster en Frasen: de definitieve decodering van elke afgesloten frase, aan elkaar gezet, plus de laatste frase bij het loslaten.
   - De vorige tekst gaat als `prompt` mee, voor de samenhang tussen frasen.

   Zo is "Frasen" ook voor batchproviders niet duurder (zelfde audio, behalve het Groq-minimum).
4. **Frasegrenzen:** een pauze van ≥ 500 ms na ≥ 3 s spraak (bij Groq ≥ 4 s), en harde knip na 15 s. Voor de pauzes gebruikt Pennel de RMS-drempel van `chunker.rs`. Silero VAD uit de catalogus is een latere verbetering — *mijn voorstel*.
5. **OpenAI en Mistral:** staat live aan, dan gebruikt dicteren het realtime-model van die provider in plaats van het gekozen batchmodel. Dat geeft één kost en geen dubbele pass. De Modellen-tab toont "Live: gpt-live-transcribe". Meetings blijven batch — *mijn voorstel* (vraag 1).
6. **Meetings:** het voorbeeld is alleen voor weergave en alleen bij lokale modellen en Apple. Er zijn geen realtime-sockets in v1 — *mijn voorstel*.
7. **WebSocket-client:** `tungstenite` (synchroon, rustls, webpki-roots, zoals ureq al meebrengt), op een eigen thread per sessie. Er komt geen async-runtime in de kern — *mijn voorstel*.
8. **Apple:** een nieuwe host-trait `SystemLiveTranscriber`, los van `SystemTranscriber`, zodat de native Mac-app die niet hoeft te implementeren. Alleen de gedeelde app levert die via `apps/macos-platform` — *mijn voorstel*.
9. **Native Mac-referentieapp:** toont de schakelaar uitgeschakeld met "Beschikbaar in de nieuwe Pennel-app" (hetzelfde precedent als de Account-tab). Het gat wordt vastgelegd in `docs/desktop.md` — *mijn voorstel* (vraag 3).
10. **Standaard aan** voor iedereen, met de hint over schermdeling — *mijn voorstel* (vraag 2).
11. **Capaciteit uit de catalogus** (`capabilities.streaming`), bij het laden bevestigd door `Model::capabilities().supports_streaming`. Er komt geen hard-gecodeerde modellijst — *mijn voorstel*.

## 8. Aan de bouwer overgelaten
- De exacte tekstinkorting en het animeren van de pill (binnen de brandwaarden).
- De interval- en drempelconstanten binnen de grenzen van §5.3 en §5.4, gekozen op basis van metingen.
- Of Venster `run()` aanroept op de volledige open frase of op een overlappend staartvenster, als het resultaat maar dezelfde eindtekst geeft.
- Hergebruik van `MonoResampler` of een eenvoudige lineaire 16→24 kHz in de kern voor OpenAI.
- De exacte Swift-API (preset `.progressiveLongDictation` of `reportingOptions: [.volatileResults]`), na verificatie op macOS 26.
- Hoe de app de realtime-capaciteit van Pennel Cloud ontdekt (accountveld of HEAD op de route).
- De volgorde binnen een fase, zolang elke taak groen eindigt.

## 9. Vragen voor Ignace

Alleen vragen waarvan het antwoord verandert wat er gebouwd wordt.

1. **Mag bij OpenAI en Mistral het realtime-model het gekozen batchmodel vervangen voor dicteren** zolang live aanstaat (andere modelnaam, mogelijk een andere prijs per minuut)?
   - *Standaard:* ja, de kaart toont dat.
   - Bij nee: OpenAI en Mistral krijgen "Frasen" met het gekozen model.
2. **Staat "Toon tekst terwijl je spreekt" standaard aan?** Dicteertekst wordt dan zichtbaar op het scherm, ook bij schermdeling.
   - *Standaard:* aan, met de hint en met uitsluiting van schermopname waar het OS die biedt.
3. **Krijgt de native Mac-referentieapp de live pill ook**, of volstaat de uitgeschakelde schakelaar?
   - *Standaard:* uitgeschakeld, met de reden. Zelf bouwen kost een extra Swift-pad voor een app die verdwijnt.
4. **Fase 4 – meetings via realtime cloud** (OpenAI, Mistral of Pennel Cloud per spoor, waarbij de realtime-tekst het transcript wordt): bouwen of niet?
   - *Standaard:* nu niet. Eerst meten hoe vaak cloud-meetings gebruikt worden.
5. **Komt Nemotron Streaming 3.5 bij de aanbevolen lokale modellen** (`catalog::RECOMMENDED`), zodat er een aanbevolen Nederlandstalig streamingmodel is?
   - *Standaard:* ja, als tweede, maar alleen na een eigen nl-meting die niet slechter is dan Parakeet TDT v3. Anders blijft het bij een "Live"-label in de lijst.
6. **Pennel Cloud realtime:** maximaal 15 minuten per sessie, en telt mee in dezelfde 20 uur per maand?
   - *Standaard:* ja op beide. Een eventuele meerprijs van Mistral realtime vangt het dagplafond op.
