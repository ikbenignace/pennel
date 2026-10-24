<img src="assets/brand/wordmark.svg" height="72" alt="pennel.">

# Pennel

Ultralichte Mac-app om overal te dicteren en (binnenkort) meetings op te nemen en samen te vatten. Je kiest zelf naar welke provider je audio en tekst gaan — OpenAI, OpenRouter, Groq, Mistral, een lokale server of je eigen endpoint. Geen account, geen telemetrie.

Ontwerp: [`docs/superpowers/specs/2026-10-23-pennel-design.md`](docs/superpowers/specs/2026-10-23-pennel-design.md)

## Vereisten

- macOS 14.4 of nieuwer op Apple Silicon
- Xcode (Swift 6)
- Rust (`rustup`, target `aarch64-apple-darwin`)
- CMake (`brew install cmake`) voor de ingebouwde whisper.cpp

## Bouwen

```sh
make core   # Rust-kern + Swift-bindings + XCFramework
make test   # Rust- en Swift-tests
make app    # build/Pennel.app
make run    # bouwen en openen
```

`apps/macos/Frameworks/` en `apps/macos/Sources/PennelCore/` worden door `make core` gegenereerd en staan niet in git.

### Ondertekenen

Zonder instelling wordt de app ad-hoc ondertekend. macOS vergeet dan na elke build de toestemmingen (microfoon, toegankelijkheid). Maak eenmalig een lokaal certificaat aan (vraagt je wachtwoord):

```sh
scripts/create-dev-cert.sh
PENNEL_SIGN_IDENTITY="Pennel Local Dev" make app
```

## Eerste start

1. Geef toegang tot de **microfoon** en **toegankelijkheid** (nodig voor de sneltoets en om tekst te plakken).
2. Kies bij **Instellingen → Providers** een provider en plak je API-sleutel. Groq met `whisper-large-v3-turbo` is snel en goedkoop.
3. Zet in **Systeeminstellingen → Toetsenbord** de 🌐/fn-toets op *Doe niets*, anders opent macOS de emoji-kiezer.

Houd **fn** ingedrukt om te praten, laat los en de tekst verschijnt waar je cursor staat. Twee keer tikken = handenvrij.

**Opdrachten:** druk tijdens het inspreken ook op **⇧ Shift**. Met tekst geselecteerd herschrijft Pennel die selectie ("maak formeler", "vertaal naar Engels", "maak er een lijstje van"); zonder selectie schrijft het wat je vraagt ("schrijf een korte mail dat ik later ben") en plakt het waar je cursor staat.

## Lokale modellen

Lokale spraakherkenning draait via [transcribe.cpp](https://github.com/handy-computer/transcribe.cpp) met Metal: Whisper, Parakeet, Canary, Cohere, Qwen3-ASR en meer. De lijst in **Providers → Lokaal** komt uit de meegebakken catalogus van [`@handy-computer/models`](https://github.com/handy-computer/models) (`core/src/catalog.json`), met per model snelheid, nauwkeurigheid per taal en ondersteunde talen. Bijwerken:

```sh
scripts/update-catalog.sh 0.2.2
```

Op een M2 Pro: Parakeet TDT 0.6B v3 transcribeert een korte zin in ~0,7 s (inclusief laden), Whisper small in ~0,6 s. App: 13 MB, ~16 MB geheugen in rust.

## Waar staat wat

- Instellingen en geschiedenis: `~/Library/Application Support/Pennel/`
- API-sleutels: in de Keychain (service `dev.pennel.Pennel`), nooit in bestanden

## Structuur

- `core/` — Rust-kern (`pennel-core`): providers, opkuisen, geschiedenis, dicteer-pipeline; via UniFFI naar Swift
- `apps/macos/` — Swift Package: `PennelKit` (pure logica, getest) en `Pennel` (de app)
- `scripts/` — build- en bundelscripts
