# Distributie — publiceren van de gedeelde app en publieke repo (ontwerp)

Stand: 2026-10-25. Repo's: `musli` (app, scripts, workflows) en `pennel-cloud` (website, alleen lezen voor dit ontwerp). Businessplan: fase A ("Een vreemde kan Pennel downloaden en installeren zonder waarschuwing").

## 1. Wat en waarom

Een groot deel van fase A is al gebouwd. De gedeelde app heeft een ondertekende kandidaat-workflow (`.github/workflows/desktop-candidate.yml`), een releasehulp die Developer ID, notarisatie, Authenticode en updater-handtekeningen afdwingt (`scripts/desktop-release.py`), een update-manifest dat alleen geaccepteerde artefacten aanvaardt (`scripts/desktop-update-manifest.py`) en een legacy Sparkle-appcast voor de overstap van de native app. De website kent al een releasemodel per platform (`apps/web/lib/site.ts`: `Release.artifacts`, `getReleaseArtifact`).

Wat nog ontbreekt, is **de laatste stap**: een geaccepteerde kandidaat echt publiceren. Vandaag eindigt alles bij "publication is a separate reviewed action", maar die actie bestaat niet. Iemand moet nu met de hand artefacten uploaden, `latest.json` en `appcast.xml` naar de website kopiëren en `SITE.latestRelease` invullen. Dat is foutgevoelig, en een fout in `latest.json` breekt de updates voor iedereen.

Daarnaast is de repo bijna klaar om publiek te gaan, maar `CONTRIBUTING.md` beschrijft nog de oude Swift-app ("no web views", `PennelKit`, alleen macOS). Er is ook geen `SECURITY.md`, en de geschiedenis is nooit op gelekte geheimen gescand.

**Klaar als:** na een geslaagde acceptatie (T8–T10 in `docs/open-items.md`) publiceert één handmatige workflow-run de release op GitHub, zet `latest.json` en `appcast.xml` klaar voor pennel.app, en toont de downloadpagina Mac, Windows en Linux, telkens alleen voor de platformen die hun acceptatie haalden. De repo kan publiek zonder gênante of gevaarlijke inhoud.

## 2. Doelen, niet-doelen, anti-doelen

**Doelen**
- Eén handmatige publicatieworkflow die een bestaande kandidaat-run plus een acceptatiebewijs omzet in: een GitHub-release (eerst als concept), `SHA256SUMS`, `latest.json`, `appcast.xml` en een `release.json` voor de website.
- De website leest `release.json` en toont per platform alleen artefacten die `published` en `validated` zijn.
- Windows-ondertekening krijgt een concreet, gedocumenteerd signeercommando.
- Publieke repo: `CONTRIBUTING.md` herschreven voor de gedeelde app, `SECURITY.md`, en een geheimenscan over de volledige geschiedenis (eenmalig en in CI).

**Niet-doelen**
- Homebrew cask, winget, Flathub of Snap. Die kunnen later, met dezelfde `release.json` als bron.
- De Mac App Store. De sandbox verbiedt plakken via een gesimuleerde ⌘V en Toegankelijkheid (businessplan §7).
- Nieuwe acceptatietests of het automatiseren van toesteltests. Die blijven werk voor Ignace (T8–T10).
- De native Mac-app opnieuw uitbrengen. Die krijgt alleen nog de legacy-appcast die naar de gedeelde app overstapt.

**Anti-doelen**
- Een platform verschijnt als "beschikbaar" op de website of in `latest.json` zonder geslaagde acceptatie. De bestaande poort in `desktop-update-manifest.py` blijft de enige weg.
- Een workflow die zelf iets publiek zet zonder menselijke klik. De GitHub-release blijft een concept tot Ignace hem publiceert.
- Signeersleutels of updatersleutels worden gegenereerd, vervangen of gelogd.
- Telemetrie in het updatekanaal: geen download-ID's of toestel-ID's in update-URL's.

## 3. Randvoorwaarden

- `AGENTS.md`: alleen claimen wat uitkomt. Website-claims volgen `release.json`, niet een losse vlag.
- De updater-endpoint ligt vast in `scripts/desktop-release.py`: `https://pennel.app/updates/latest.json`. De native Sparkle-feed is `https://pennel.app/appcast.xml` (`SITE.appcastUrl`).
- Downloads via GitHub Releases werken pas als de repo publiek is (`docs/releasing.md`). Repo publiek maken en de eerste release horen dus samen.
- Niet-technische blokkers die vóór publicatie opgelost moeten zijn (businessplan §0): bevestiging van de werkgever, een ondernemingsnummer, Apple Developer ID (€99/jaar) en een Windows-certificaat.

## 4. Beslissingen

1. **Publiceren is een aparte, handmatige workflow `desktop-publish.yml`** in `musli`, die de artefacten van een bestaande `desktop-candidate`-run ophaalt. Er wordt niets opnieuw gebouwd, dus de hashes uit het acceptatiebewijs blijven kloppen. *(mijn voorstel)*
2. **Eén script `scripts/desktop-publish.py` doet al het rekenwerk** en hergebruikt `accepted()`, `verify_artifacts()` en `manifest()` uit `desktop-update-manifest.py`. De workflow is alleen lijm, en het script heeft unit tests in `scripts/tests/`. *(mijn voorstel)*
3. **Binaries op GitHub Releases, metadata op pennel.app.** `latest.json`, `appcast.xml` en `release.json` gaan via een pull request naar `pennel-cloud/apps/web/public/updates/` (en `public/appcast.xml`). Zo loopt elke release door staging en een handmatige promotie naar productie, en daarmee door een tweede menselijke controle. *(mijn voorstel)*
4. **Platformen apart vrijgeven.** Het acceptatiebewijs bevat al aparte poorten (`macos`, `windows`, `linux_x11`, `linux_wayland`). `release.json` neemt een platform alleen op als zijn poort `passed` is. Het updatemanifest blijft de drie updater-doelen vereisen, zoals vandaag. *(mijn voorstel)*
5. **Windows-ondertekening via Azure Artifact Signing** (vroeger Trusted Signing), aangeroepen via het bestaande `PENNEL_WINDOWS_SIGN_COMMAND`. Dat is goedkoper dan een EV-certificaat, vraagt geen hardwaretoken en geeft sneller SmartScreen-reputatie dan een OV-certificaat. *(mijn voorstel; zie vraag 1)*
6. **De geheimenscan gebruikt `gitleaks`**: eenmalig over de volledige geschiedenis van alle branches vóór de repo publiek gaat, daarna als CI-job op elke push. Een vondst wordt een rotatie van dat geheim, niet alleen een herschreven geschiedenis. *(mijn voorstel)*
7. **De Engelstalige README blijft zoals hij is**, maar krijgt een sectie "Download" die naar pennel.app/download linkt zodra er een release is. De app-UI blijft Nederlandstalig. *(mijn voorstel)*

## 5. Aan de bouwer overgelaten

- Hoe de workflow de kandidaat-artefacten ophaalt (`actions/download-artifact` met `run-id`, of `gh run download`).
- Of het PR naar `pennel-cloud` automatisch komt (met een fine-grained token met alleen `contents` en `pull_requests` op die repo) of als artefact dat Ignace zelf commit. Als dat token nog niet bestaat, valt de workflow terug op een artefact.
- De exacte inhoud van `SECURITY.md`, binnen: meldadres `security@pennel.app` (staat al in `SITE.securityEmail`), geen bug bounty, een antwoord binnen 7 dagen.

## 6. Volgorde en afhankelijkheden

- Hangt **niet** af van Pennel Cloud v1. Kan nu starten, parallel aan 3g.
- Hangt wel af van de toesteltests T8–T10 en van de niet-technische blokkers in §3. De tooling mag eerder klaar zijn. Publiceren gebeurt pas na de acceptatie.
- Moet klaar zijn **vóór** fase C (betalen). Je kan geen Cloud Pro verkopen aan iemand die de app niet zonder waarschuwing kan installeren.

## 7. Vragen voor Ignace

1. **Windows-certificaat: Azure Artifact Signing of een klassiek OV-certificaat?** Dit bepaalt het signeercommando en de secrets. *Standaard: Azure Artifact Signing, als je onderneming de identiteitsvalidatie haalt; anders een OV-certificaat in een cloud-HSM.*
2. **Mogen de binaries op GitHub Releases staan?** Dat vereist een publieke repo op de dag van de eerste release. *Standaard: ja, de repo gaat publiek samen met de eerste release.*
3. **Mag de workflow automatisch een PR openen in `pennel-cloud`?** Dat vraagt een fine-grained token. *Standaard: ja, met een token dat alleen die repo mag aanraken.*
