# Pennel commercialiseren — open source + freemium

Stand: 2026-10-24. Concept om samen te beslissen; prijzen en bronnen uit marktonderzoek van dezelfde dag (zie onderaan).

## 0. Eerst: drie blokkers vóór je iets publiek zet

1. **Naam — beslist: Pennel.** De werknaam "Musli" botste met de open-source Mac-app **Muesli** (`Muesli-HQ/muesli`) die bijna hetzelfde doet, en alle musli-domeinen waren bezet. Onderzoek in `branding/naming-research.md`. Nog te doen: pennel.app (+ .ai, getpennel.com) registreren en een merkenonderzoek in klasse 9 en 42.
2. **Werkgever.** Check je arbeidscontract bij AE op IP-clausules (eigendom van software gemaakt tijdens of naast je job, concurrentiebeding, nevenactiviteiten). Laat bij twijfel schriftelijk bevestigen dat Pennel jouw project is. Dit bepaalt of je het überhaupt mag verkopen.
3. **Juridische vorm.** Om te factureren via een Merchant of Record heb je een ondernemingsnummer nodig (eenmanszaak in bijberoep volstaat om te starten; BV later voor aansprakelijkheid).

## 1. Positionering

> **Eén lichte Mac-app voor alles wat je zegt: dicteren in elke app én meetingnotities — privé, op je eigen toestel, open source.**

Waarom iemand voor ons kiest i.p.v. Wispr Flow ($12–15/m) + Granola ($14/m):
- **Twee producten in één** — dicteren en meetings, één abonnement.
- **Privé eerst** — lokale modellen (Parakeet/Whisper) of je eigen provider; meetings als Markdown in je eigen map.
- **Licht** — 13 MB, ~16 MB geheugen (Electron-concurrenten: honderden MB).
- **Open source** — vertrouwen, controleerbaar, geen lock-in.
- **Beste Nederlands/Vlaams** — opkuis en modelkeuze afgestemd op NL (concurrenten zijn Engels-eerst). Sterke niche: Benelux-consultants, juristen, zorg, overheid.

## 2. Model: open core

| Laag | Licentie | Waarom |
|------|----------|--------|
| App + Rust-kern | **Apache-2.0** (aanbevolen) of MIT | Maximale adoptie en bijdragen (Handy: MIT, ~23k sterren). Apache geeft patentbescherming. Merknaam blijft van jou. |
| Pennel Cloud (backend) | **Gesloten** (of FSL-1.1 als je hem toch wil tonen) | De waarde zit in de gehoste dienst (sleutels, kwaliteit, privacygaranties, facturatie), niet in proxycode. |
| Merk + logo | Geregistreerd merk (BOIP/EUIPO) | Forks mogen de code, niet de naam. |

Alternatief: GPL-3.0 + CLA (zoals VoiceInk, die betaalde ondertekende builds verkoopt). Nadeel: minder bijdragers, App Store-complicaties. Niet aangeraden: AGPL/BSL/ELv2 op de app — botst met "echt open source" en levert hier weinig op.

**Principe:** alles wat lokaal draait is gratis en volledig. Betalen = gemak (geen sleutels nodig), betere hosted modellen, sync en teamfuncties.

## 3. Pakketten

| | **Gratis** (open source) | **Cloud Free** | **Pro** | **Team** |
|---|---|---|---|---|
| Prijs | €0 | €0 (account) | **€9/m of €90/j** | **€15/gebruiker/m** (€150/j) |
| Lokaal dicteren + meetings | onbeperkt | onbeperkt | onbeperkt | onbeperkt |
| Eigen API-sleutels (BYOK) | ✓ | ✓ | ✓ | ✓ |
| Pennel Cloud (geen sleutels nodig) | — | 2.000 woorden/week + 2 meetings/m | fair use: ~40 u dicteren + 20 u meetings/m | idem per gebruiker |
| Beste hosted modellen, sprekers in de cloud, vragen over meetings | — | basis | ✓ | ✓ |
| Sync (instellingen, woordenlijst, sjablonen; later iPhone) | — | — | ✓ | ✓ |
| Gedeelde sjablonen, teamwoordenlijst, beheer, facturatie op factuur | — | — | — | ✓ |
| EU-only verwerking + DPA | — | — | optie | ✓ |

Waarom zo:
- **Gratis blijft echt bruikbaar** (zoals Handy/Superwhisper) → groei via open source.
- **Cloud Free** haalt niet-technische gebruikers binnen zonder sleutel-gedoe; kost ons ~€0,10/gebruiker/maand.
- **Pro €9** ligt onder Wispr ($12–15) en Granola ($14) samen, met marge (zie §5).
- **Team** is waar het geld zit (B2B, consultancy, AE als eerste pilot).
- Jaarplannen pushen: vaste transactiekost (~$0,50) weegt zwaar op €9.

## 4. Pennel Cloud — architectuur

Doel: weinig ops, lage kost, privacy aantoonbaar.

```
Pennel app ──(token)──► api.<merk>.app  (Cloudflare Workers)
                         ├─ /v1/audio/transcriptions   → Groq whisper-large-v3-turbo  (EU-optie: Mistral Voxtral)
                         ├─ /v1/chat/completions       → snel goedkoop LLM (Gemini Flash-Lite / gpt-5-nano / Mistral Small)
                         ├─ /auth (magic link / device code), /account, /usage
                         └─ webhooks van betaalprovider (Polar/Lemon Squeezy/Paddle)
                       D1 (accounts, abonnementen, verbruik)  ·  Durable Object per gebruiker (quota, rate limits)
```

- **OpenAI-compatibel**: de app kent dit al. Pennel Cloud wordt gewoon een nieuwe providersoort ("Pennel Cloud") met vaste `base_url` en een token in plaats van een sleutel. Minimale app-wijziging.
- **Geen opslag** van audio of tekst: enkel tellers (seconden audio, tokens). Providers met zero-retention (Groq-console, OpenAI ZDR aanvragen). Betaalde Gemini-tier (de gratis tier traint op data).
- **Misbruik**: kortlevende tokens per toestel, quota per gebruiker, max clipduur, max gelijktijdige streams, dagelijkse kostenplafond per provider, wegwerpmail blokkeren voor de gratis tier.
- **Auth**: account via magic link of device-code login vanuit de app; abonnement gekoppeld aan de klant bij de betaalprovider.
- **Kost infra**: Cloudflare Workers Paid $5/m + verbruik; verwaarloosbaar tot duizenden gebruikers.

## 5. Unit economics (per actieve gebruiker per maand)

| Profiel | Kost (budget-stack: Groq turbo + goedkoop LLM) | Kost (premium) |
|---------|------|------|
| Zware dicteerder (1 u spraak/dag ≈ 30 u/m) | ≈ $1,5–2,3 | ≈ $6–10 |
| Meetinggebruiker (10 u/m, met samenvatting + vragen) | ≈ $0,55 | ≈ $2,5–4 (met cloud-diarization) |

Bij €9/m: na ~10 % fee van de Merchant of Record en ~€2 providerkost → **±70–80 % brutomarge**. Fair-use plafond beschermt tegen uitschieters; premium-modellen enkel in Pro/Team.

## 6. Betalen, btw, licenties

- **Merchant of Record** (zij zijn de verkoper, regelen EU-btw/OSS wereldwijd; jij factureert alleen hen):
  - **Polar.sh** of **Lemon Squeezy**: ingebouwde licentiesleutels, goede dev-ervaring, ~5 % + $0,50.
  - **Paddle**: meest gevestigd, ~5 % + $0,50.
  - Stripe Managed Payments (+3,5 %): check of Belgische verkopers ondersteund zijn.
- Advies: **Polar** of **Lemon Squeezy** om te starten.

## 7. Distributie

- **Rechtstreekse download** (geen Mac App Store: sandbox verbiedt plakken via gesimuleerde ⌘V en Toegankelijkheid; Wispr Flow doet het ook zo).
- **Apple Developer ID** (€99/j): ondertekenen + notariseren → geen Gatekeeper-waarschuwing, geen sleutelhangervragen.
- **Sparkle** voor automatische updates (appcast op R2/Pages).
- **Homebrew cask** (vereist notarisatie).
- Website: landing + prijzen + docs + changelog (Cloudflare Pages).

## 8. Privacy & juridisch

- Privacyverklaring, gebruiksvoorwaarden, **DPA** + lijst sub-verwerkers (Cloudflare, Groq/OpenAI/Google/Mistral, betaalprovider).
- Standaard: geen audio opslaan in de cloud; meetings blijven lokaal.
- **Toestemming bij meetings**: opnemen van anderen vraagt melding (zit al in de app als optie: toestemmingszin).
- EU-only verwerking als Team-feature (Mistral, EU-regio's) — sterk verkoopargument in de Benelux.

## 9. Go-to-market

1. **Pilot bij AE** (na OK werkgever): 5–10 collega's, Team-plan gratis in ruil voor feedback → eerste testimonials en een B2B-referentie.
2. **Open-source lancering**: publieke repo (Engelstalige README, GIF's), Show HN, r/macapps, Product Hunt, lijsten als openalternative.co ("open-source Wispr Flow + Granola").
3. **Benelux-niche**: content in het Nederlands ("beste dicteer-app voor Vlaams/Nederlands"), LinkedIn, consultancy-netwerken.
4. **Groei-lus**: gratis lokaal → Cloud Free (geen sleutels) → Pro bij limiet; iPhone-toetsenbord (fase 4) als extra reden om te betalen (heeft cloud nodig).

## 10. Stappenplan

| Fase | Inhoud | Klaar als |
|------|--------|-----------|
| **A. Fundament** (1–2 w) | Naam + branding kiezen, werkgever-check, Developer ID, notarisatie, Sparkle, licentie, Engelse README, publieke repo | Een vreemde kan Pennel downloaden en installeren zonder waarschuwing |
| **B. Pennel Cloud MVP** (2–3 w) | Workers-proxy (STT + LLM), accounts (magic link), quota/rate limits, "Pennel Cloud"-provider in de app + inloggen | Nieuwe gebruiker dicteert zonder eigen sleutel |
| **C. Betalen** (1 w) | Polar/Lemon Squeezy, webhooks → abonnement, verbruikspagina, upgrade-flow in de app | Iemand kan Pro kopen en het werkt meteen |
| **D. Website + lancering** (1 w) | Landing, prijzen, docs, privacy/DPA, lancering | Publieke launch |
| **E. Team** | Workspaces, gedeelde sjablonen/woordenlijst, facturatie, EU-only | Eerste betalend team |
| **F. iPhone** | Toetsenbord + opnemen, via Pennel Cloud | Pro-feature op iOS |

## 11. Beslissingen voor jou

1. ~~Naam~~ → Pennel (beslist). Logo-richting kiezen (`branding/pennel/`).
2. Licentie app: **Apache-2.0** (advies), MIT, of GPL-3.0 + CLA?
3. Prijzen: €9 Pro / €15 Team ok? Jaarkorting?
4. Betaalprovider: Polar, Lemon Squeezy of Paddle?
5. Eerst Cloud (fase B) of eerst iPhone? Advies: Cloud — het is de verdienmotor en iPhone hangt ervan af.

## Bronnen (marktonderzoek 2026-10-24)

Prijzen concurrenten: [Wispr Flow](https://wisprflow.ai/pricing), [Superwhisper](https://superwhisper.com/docs/get-started/sw-pro.md), [VoiceInk](https://tryvoiceink.com/pricing), [Handy](https://github.com/cjpais/Handy), [Granola](https://www.granola.ai/pricing), [Otter (3P)](https://sonix.ai/resources/otter-ai-pricing/), [Fireflies (3P)](https://www.castmagic.io/de/blog/fireflies-pricing).
Licenties: [Sentry FSL](https://blog.sentry.io/introducing-the-functional-source-license-freedom-without-free-riding), [Elastic License FAQ](https://www.elastic.co/licensing/elastic-license/faq).
API-prijzen: [Groq](https://groq.com/whisper-large-v3-turbo-now-available-on-groq-combining-speed-quality-for-speech-recognition/), [OpenAI](https://developers.openai.com/api/docs/pricing), [Deepgram](https://deepgram.com/pricing), [AssemblyAI](https://www.assemblyai.com/pricing), [Google Gemini](https://ai.google.dev/gemini-api/docs/pricing).
Betalen: [Paddle](https://www.paddle.com/pricing), [Lemon Squeezy](https://www.lemonsqueezy.com/pricing), [Stripe Managed Payments](https://stripe.com/managed-payments).
Hosting/privacy: [Cloudflare Workers pricing](https://developers.cloudflare.com/workers/platform/pricing/), [OpenAI data](https://developers.openai.com/api/docs/guides/your-data), [Groq data](https://console.groq.com/docs/your-data).
Mac App Store-beperkingen: [Apple forums 2.4.5](https://developer.apple.com/forums/thread/820594).
Naamconflict: [Muesli-HQ/muesli](https://newreleases.io/project/github/Muesli-HQ/muesli/release/v0.8.5).
