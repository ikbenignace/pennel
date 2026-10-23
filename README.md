<img src="assets/icon.svg" width="64" height="64" alt="">

# Musli

Ultralichte Mac-app om overal te dicteren en (binnenkort) meetings op te nemen en samen te vatten. Je kiest zelf naar welke provider je audio en tekst gaan — OpenAI, OpenRouter, Groq, Mistral, een lokale server of je eigen endpoint. Geen account, geen telemetrie.

Ontwerp: [`docs/superpowers/specs/2026-10-23-musli-design.md`](docs/superpowers/specs/2026-10-23-musli-design.md)

## Vereisten

- macOS 14.4 of nieuwer op Apple Silicon
- Xcode (Swift 6)
- Rust (`rustup`, target `aarch64-apple-darwin`)

## Bouwen

```sh
make core   # Rust-kern + Swift-bindings + XCFramework
make test   # Rust- en Swift-tests
make app    # build/Musli.app
make run    # bouwen en openen
```

`apps/macos/Frameworks/` en `apps/macos/Sources/MusliCore/` worden door `make core` gegenereerd en staan niet in git.

### Ondertekenen

Zonder instelling wordt de app ad-hoc ondertekend. macOS vergeet dan na elke build de toestemmingen (microfoon, toegankelijkheid). Maak eenmalig een lokaal certificaat aan (vraagt je wachtwoord):

```sh
scripts/create-dev-cert.sh
MUSLI_SIGN_IDENTITY="Musli Local Dev" make app
```

## Eerste start

1. Geef toegang tot de **microfoon** en **toegankelijkheid** (nodig voor de sneltoets en om tekst te plakken).
2. Kies bij **Instellingen → Providers** een provider en plak je API-sleutel. Groq met `whisper-large-v3-turbo` is snel en goedkoop.
3. Zet in **Systeeminstellingen → Toetsenbord** de 🌐/fn-toets op *Doe niets*, anders opent macOS de emoji-kiezer.

Houd **fn** ingedrukt om te praten, laat los en de tekst verschijnt waar je cursor staat. Twee keer tikken = handenvrij.

## Waar staat wat

- Instellingen en geschiedenis: `~/Library/Application Support/Musli/`
- API-sleutels: in de Keychain (service `dev.musli.Musli`), nooit in bestanden

## Structuur

- `core/` — Rust-kern (`musli-core`): providers, opkuisen, geschiedenis, dicteer-pipeline; via UniFFI naar Swift
- `apps/macos/` — Swift Package: `MusliKit` (pure logica, getest) en `Musli` (de app)
- `scripts/` — build- en bundelscripts
