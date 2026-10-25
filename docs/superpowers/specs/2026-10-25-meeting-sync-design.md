# Pennel Cloud v1 — accounts, meeting-sync en gehoste modellen (ontwerp)

Stand: 2026-10-25. Repo's: `musli` (app + kern) en `pennel-cloud` (server + website).

## 1. Wat en waarom

Wie bij Pennel Cloud is ingelogd, ziet zijn meetingnotities op elk toestel waarop hij inlogt: Mac, Windows en Linux nu, de iPhone later (fase 4). Een meeting die op de laptop is opgenomen staat even later op de andere computer, met notities, samenvatting en transcript.

Het moet "super proper" blijven, dus in lijn met de productregels: privé als standaard, meetings blijven lokale Markdown, en niets wordt verstuurd behalve wat nodig is. Daarom is de sync **end-to-end versleuteld**. De server bewaart alleen versleutelde blobs en kan titels, notities en transcripten niet lezen.

**Waarom niet iCloud of een gedeelde map?** iCloud bestaat niet op Linux, werkt omslachtig op Windows en hangt niet aan het Pennel-account. Een map die met Dropbox of OneDrive synct, kan de gebruiker vandaag al zelf kiezen. Dat blijft werken, maar het is geen Pennel-functie en het wordt geen basis voor de iPhone.

**Klaar als:** iemand logt op twee computers in met hetzelfde account, neemt op de ene een meeting op, en ziet die binnen een minuut na afronden op de andere. Daar kan hij de meeting bewerken, en die bewerking komt ook terug op de eerste computer.

## 2. Doelen, niet-doelen, anti-doelen

**Doelen (v1)**
- Pennel-account met login vanuit de app (geen wachtwoord).
- Meetings automatisch synchroniseren tussen alle toestellen van één account: `meeting.md`, `notes.md`, `transcript.md` en alle andere tekstbestanden in een meetingmap. Ook de samenvattingssjablonen (`.templates/*.md`) gaan mee.
- Nieuwe, gewijzigde en verwijderde meetings, ook als ze buiten Pennel bewerkt zijn (bijvoorbeeld in Obsidian).
- Werkt offline: wijzigingen gaan mee zodra er weer verbinding is.
- Toestellen bekijken en afmelden; account en clouddata volledig verwijderen.

**Niet-doelen (v1, wel later met hetzelfde mechanisme)**
- Audio synchroniseren (`*.m4a`): te groot voor v1, komt later als aparte opt-in.
- Instellingen, woordenlijst en dictaatgeschiedenis synchroniseren.
- Een webviewer op app.pennel.app (die zou in de browser moeten ontsleutelen).
- Meetings delen met anderen of in een team.
- De iPhone-app. Het protocol moet die wel zonder wijzigingen kunnen bedienen.
- Betalen. Sync en gehoste modellen zijn in de beta gratis voor elk account, binnen de limieten van §7 (beslist door Ignace, 2026-10-25).

**Anti-doelen (ook als het technisch werkt, is het dan mislukt)**
- De server of de beheerder kan meetinginhoud of meetingtitels lezen.
- Data gaat verloren door een conflict of een verwijdering. Bij twijfel worden beide versies bewaard.
- Sync wordt verplicht of gaat vanzelf aan. Lokaal gebruik zonder account blijft volledig.
- Meetinginhoud of bestandsnamen komen in logs terecht, in de app of op de server.

## 3. Hoe het werkt (beslist)

### 3.1 Account en inloggen
- **Eén accountsysteem: de Rust-server** (`pennel-server` + Postgres) is eigenaar van gebruikers, sessies, toestellen en verwijderen. De webpagina's (`/device`, later het dashboard met Stripe) praten met de API van die server. De geplande port van better-auth uit kbo-api vervalt, zodat er geen tweede gebruikerstabel komt. Account verwijderen ruimt later ook het Stripe-klantrecord op.
- Inloggen gebeurt in de browser. In **Instellingen → Account** klik je **Inloggen**. De app toont een korte code en opent `https://app.pennel.app/device`, een pagina van de website in `pennel-cloud/apps/web`. Daar vul je je e-mailadres in, je klikt op de magic link in je mail en je bevestigt de code. Ondertussen vraagt de app aan de server of de login al rond is (device-code flow).
- Een geslaagde login levert per toestel een **refresh token** op, dat in de OS-sleutelopslag komt (Keychain, Credential Manager, Secret Service), net zoals API-sleutels vandaag. Daarnaast is er een kortlevend access token van 15 minuten.
- Elk toestel heeft een naam (standaard de computernaam), die je kan aanpassen. Een toestel afmelden trekt zijn refresh token in.

### 3.2 Versleuteling
- Er is één willekeurige **kluissleutel** van 256 bit per account. Het eerste toestel dat sync aanzet, maakt hem aan. Hij verlaat een toestel alleen ingepakt.
- Bij het aanmaken toont de app één keer een **herstelcode** (bijvoorbeeld 6×4 tekens) en vraagt de gebruiker die te bewaren. De server krijgt alleen de kluissleutel, ingepakt met een sleutel die met Argon2id uit de herstelcode is afgeleid, samen met de KDF-parameters.
- Een nieuw toestel vraagt na het inloggen om de herstelcode, haalt de ingepakte sleutel op en pakt hem lokaal uit. De sleutel wordt op het toestel in de OS-sleutelopslag bewaard.
- De herstelcode wordt nergens bewaard, ook niet op het toestel. **Nieuwe herstelcode maken** pakt de kluissleutel opnieuw in en vervangt de kopie op de server. Daarmee wordt de oude code ongeldig.
- Ben je de herstelcode kwijt en heb je geen toestel meer met de sleutel, dan is de clouddata onleesbaar. De app zegt dat eerlijk en biedt **Kluis opnieuw beginnen** aan:
  - De server wist alle objecten en verhoogt de **kluisgeneratie**.
  - Dit toestel maakt een nieuwe sleutel, toont een nieuwe herstelcode en uploadt zijn lokale bestanden.
  - Andere toestellen zien de nieuwe generatie, stoppen met syncen en vragen: "De kluis is opnieuw begonnen op een ander toestel — voer de nieuwe herstelcode in." Daarna doen ze een eerste sync, waarbij verschillen conflictkopieën worden.

  Lokale bestanden gaan nooit verloren.
- Een toestel afmelden trekt alleen zijn toegang in. Het behoudt wat het al had, en de sleutel wordt in v1 niet geroteerd. Wie een gestolen toestel vreest, gebruikt **Kluis opnieuw beginnen**. Dat staat zo in de app.
- Elk bestand is een **object**:
  - Uit de kluissleutel worden twee subsleutels afgeleid (domeinscheiding): `id-sleutel = HMAC-SHA256(kluissleutel, "pennel/object-id/v1")` en `inhoudssleutel = HMAC-SHA256(kluissleutel, "pennel/object-enc/v1")`.
  - Het object-id is `HMAC-SHA256(id-sleutel, sleutel)`, zodat de server geen namen ziet. De sleutel is `meeting/<meeting-uid>/<bestandsnaam>` voor meetingbestanden en `templates/<bestandsnaam>` voor sjablonen.
  - Elke meeting krijgt daarvoor een **vaste uid** (UUIDv7), die als `pennel_id:` in de frontmatter van `meeting.md` staat. Bestaande meetings krijgen die bij de eerste sync. Zo is een hernoemde meeting geen "alles wissen en opnieuw aanmaken".
  - De inhoud is `XChaCha20-Poly1305(inhoudssleutel, {sleutel, mapnaam, inhoud, gewijzigd-op})`, met het object-id en de kluisgeneratie als associated data. De mapnaam reist mee in de ciphertext. Een ontvanger zoekt de map op via de uid, en als de mapnaam verschilt, hernoemt hij de map.
- Wat de server wel ziet: aantal objecten, grootte, tijdstippen en welk toestel uploadt. Dat staat ook in de privacytekst.

### 3.3 Synchronisatie
- De eenheid is **een bestand**, niet een meeting. De meetingmap blijft de bron van waarheid. De sync-status (per pad: lokale hash, serverversie) bewaart Pennel in een SQLite-bestand in de app-supportmap, niet in de meetingmap.
- Pennel scant de meetingmap bij het opstarten, na elke eigen schrijfactie (afronden, notities bewerken, samenvatten, hernoemen) en elke paar minuten. Een bestandswatcher mag als versnelling, maar is niet nodig.
- **Wat gaat mee:** tekstbestanden in meetingmappen en `.templates/`.
  **Wat niet:** audio, `*.tmp`, de zoekindex, verborgen systeembestanden (`.DS_Store`, `desktop.ini`) en een meeting **die nog opneemt** (die volgt na afronden).
- **Uploaden** gebeurt met optimistische concurrency: elke upload stuurt de serverversie mee waarop hij gebaseerd is. Bestaat er intussen een nieuwere versie, dan is dat een conflict.
- **Downloaden** gaat via een oplopende cursor per account: "geef alle wijzigingen sinds X". Na ontsleutelen schrijft Pennel het bestand atomisch weg (eerst `.tmp`, dan hernoemen, zoals vandaag). Daarna wordt de zoekindex bijgewerkt en ververst de UI.
- **Conflict** (beide kanten gewijzigd sinds de laatste sync): de lokale versie blijft staan, en de serverversie wordt ernaast gezet als `notes (conflict <toestel> <datum>).md`. Zo'n kopie synchroniseert gewoon mee. De meeting toont de melding "Er is een conflictversie" met **Bekijken** en **Verwijderen**. Er gaat niets verloren en er gebeurt geen automatische merge.
- **Verwijderen:** een gewiste meeting levert grafstenen (tombstones) op voor zijn bestanden. Op de andere toestellen gaat de map naar een lokale prullenbak in de app-supportmap, die 30 dagen bewaard blijft. Verwijderen kan dus nooit stil data vernietigen.
- **Mapnamen** (`2026-10-25 1400 Titel`) zitten in het pad en reizen dus mee. Een andere meetingmap op een ander toestel is geen probleem, want paden zijn relatief aan de meetingmap.
- Staat de meetingmap al in iCloud, Dropbox of OneDrive, dan waarschuwt Pennel bij het aanzetten van sync: "Deze map synct al via … — dubbel synchroniseren kan conflicten geven." Sync blijft wel toegestaan.

### 3.4 Server (`pennel-cloud/crates/pennel-server`)
De bestaande Axum-server met Postgres 18 uit de Dokploy-stack. Voor deze schaal is geen extra opslagdienst nodig: blobs gaan als `bytea` in Postgres, met een maximum van 5 MB per object. S3 komt pas met audio.

Endpoints (JSON, onder `/v1`):

| Methode en pad | Doel |
|---|---|
| `POST /auth/device` | start de login: `device_code`, `user_code`, `verification_uri` |
| `POST /auth/token` | de app vraagt na of de login rond is; daarna access- en refresh-token |
| `POST /auth/refresh` | nieuw access token |
| `POST /auth/email`, `POST /auth/email/verify`, `POST /auth/device/lookup` | voor de webpagina: magic link sturen en verifiëren, en de toestelnaam bij een code opzoeken. Het toestel keurt de server goed bij `email/verify`; een aparte `approve` bestaat niet |
| `GET` / `PUT /vault` | ingepakte kluissleutel, KDF-parameters en generatie; `DELETE /vault` = opnieuw beginnen (wist alle objecten, generatie +1). Een kluis aanmaken kan alleen met `If-None-Match: *`: 409 `vault_exists` als er al een is; zonder die header op een ontbrekende kluis 409 `vault_missing` |
| `GET /sync/changes?since=&limit=` | wijzigingen sinds de cursor: `[{id, version, size, deleted}]`, een nieuwe cursor en de huidige kluisgeneratie |
| `GET /objects/{id}` | versleutelde blob en versie |
| `PUT /objects/{id}` | blob met `If-Match: <versie>` (0 = nieuw); 409 bij conflict |
| `DELETE /objects/{id}` | grafsteen, met `If-Match` |
| `GET /devices`, `PATCH`/`DELETE /devices/{id}` | toestellen bekijken, hernoemen, afmelden |
| `GET /account`, `DELETE /account` | e-mail, verbruik, alles wissen |

- De versie is een oplopend nummer per account. Dat nummer is meteen de cursor.
- Grafstenen worden na 90 dagen opgeruimd. Een toestel dat langer offline was, doet een volledige vergelijking.
- Mail wordt verstuurd via een EU-mailprovider (bijvoorbeeld Scaleway TEM of Postmark EU). Magic links zijn 15 minuten geldig en werken één keer.
- Rate limits per IP en per account op auth en uploads. Wegwerpmailadressen worden geweigerd.
- Logs bevatten alleen methode, status, duur, grootte en account-id. Nooit object-id's, e-mailadressen of bodies.

### 3.5 App (`musli`)
- **Kern (`core/`):** een nieuwe module `sync` met de versleuteling, de scan, de push/pull-lus en de conflict- en prullenbaklogica. Daarnaast een module `cloud_account` voor login en tokens. HTTP gaat via de bestaande `ureq`. Nieuwe crates: `chacha20poly1305`, `argon2`, `hmac` (pure Rust, licht). Alles heeft unit tests met een nep-server.
- **Gedeelde Tauri-app:** een nieuwe tab **Account** in Instellingen met de volgende onderdelen:
  - Inloggen en uitloggen.
  - Sync aan/uit.
  - De status: "Gesynchroniseerd · 2 min geleden", "Offline", "Fout: …".
  - De herstelcode (opnieuw tonen vraagt bevestiging).
  - De lijst van toestellen.
  - Account verwijderen.

  In de meetinglijst staat een klein sync-icoon per meeting (gesynchroniseerd, wacht of conflict). Alle tekst is in het Nederlands, in de stijl van `assets/brand/brand.md`.
- **Native Mac-referentieapp:** toont dezelfde Account-tab uitgeschakeld, met de reden "Beschikbaar in de nieuwe Pennel-app". Het gat wordt vastgelegd in `docs/desktop.md`.
- **Website en README** worden pas aangepast wanneer de functie uitkomt, en dan alleen met wat echt werkt:
  - De Cloud-pagina, de privacypagina en de FAQ krijgen "meetings synchroniseren end-to-end versleuteld als je sync aanzet". Dat vervangt "Je meetings blijven op je toestel" waar dat niet meer klopt.
  - Er komt bij wat de server ziet en wat niet.
  - Er komt bij dat de herstelcode de enige weg terug is.

### 3.6 Gehoste modellen (fase B uit het businessplan)
Doel: een nieuwe gebruiker kan dicteren, opkuisen en meetings samenvatten zonder eigen API-sleutel, met hetzelfde account als sync.

- **De server als OpenAI-compatibele proxy:**
  - `POST /v1/audio/transcriptions` (multipart, zoals OpenAI) en `POST /v1/chat/completions` (JSON, ook streaming via SSE).
  - Authenticatie met het access token van §3.1.
  - De app kent dit formaat al.
- **Upstream:**
  - Standaard Mistral, omdat dat in de EU verwerkt en de website "verwerkt in de EU" belooft: Voxtral voor spraak, Mistral Small voor tekst.
  - De upstream-URL, het model en de sleutel per taak komen uit omgevingsvariabelen (Dokploy-secrets), zodat we kunnen wisselen zonder release.
  - De app stuurt als model `pennel-auto`; de server kiest het echte model. Een model dat niet in de toegelaten lijst staat, krijgt 400.
- **Opslag:** geen audio en geen tekst. De server telt per account en per dag alleen seconden audio, inputtokens en outputtokens (uit de `usage` van upstream). Request- en response-bodies worden nooit gelogd.
- **Misbruikbeperking:**
  - Maximaal 25 MB per request. De grens van 30 minuten audio controleert de server alleen bij WAV; voor andere formaten bepaalt de grens van 25 MB de lengte.
  - Chat-requests mogen maximaal 4 MB zijn.
  - Maximaal 2 gelijktijdige requests per account.
  - Een maandlimiet per account (§7).
  - Een globaal dagplafond aan kosten per upstream (env). Daarboven antwoordt de server 503 met "Pennel Cloud is tijdelijk overbelast".
  - Wie een limiet bereikt, krijgt 429 met een Nederlandse foutmelding die de app toont.
- **App:**
  - Er komt een nieuwe `ProviderKind::PennelCloud` ("Pennel Cloud") met een vaste `base_url`: `https://api.pennel.app/v1`, of de staging-URL in preview-builds.
  - De provider heeft geen sleutelveld; de kern zet het access token in de request en vernieuwt het zelf.
  - Pennel Cloud kan alle drie de modelslots vullen (dicteren, meetings, opkuisen).
  - Zonder login toont de kaart "Log in bij Pennel Cloud" met een knop naar de Account-tab.
  - Na de eerste login biedt de app aan om Pennel Cloud in te stellen voor slots die nog geen provider hebben. Bestaande keuzes worden nooit overschreven.
  - De Account-tab toont het verbruik van deze maand tegenover de limiet.

## 4. Randvoorwaarden
- De productregels in `CLAUDE.md`: geen telemetrie, sleutels en tokens alleen in de OS-sleutelopslag, onbetrouwbare tekst mag nooit acties starten (gesynchroniseerde Markdown is gewoon data).
- Gelijk gedrag op macOS, Windows en Linux; CI blijft groen op alle drie.
- Geen zware afhankelijkheden: geen sync-SDK, geen Firebase of Supabase.
- De iPhone (fase 4) moet hetzelfde protocol en dezelfde kernmodule kunnen gebruiken.

## 5. Aan de bouwer overgelaten
- De precieze tabelindeling, migraties (sqlx of refinery) en tokenformaat (opaak en gehasht opgeslagen, of PASETO/JWT).
- Het formaat van de herstelcode en de exacte Argon2id-parameters (minstens de OWASP-aanbeveling).
- Het scaninterval en de batchgrootte. Een bestandswatcher of niet.
- De keuze van de mailprovider binnen de EU.
- Of de webpagina `/device` in Next.js of in Axum rendert.
- De volgorde van werk: eerst server en kern met tests tegen elkaar, dan de UI.

## 6. Keuzes die ik maakte (te controleren)
1. End-to-end versleutelde Pennel Cloud-sync in plaats van iCloud of een gedeelde map.
2. Inloggen met magic link en device code, zonder wachtwoord.
3. Een herstelcode als enige sleutelback-up. Goedkeuren vanaf een bestaand toestel komt later.
4. Audio niet in v1.
5. Conflicten als kopie naast het origineel, zonder automatische merge.
6. Sync tijdens de beta gratis voor elk account, met een limiet.
7. UI alleen in de gedeelde app, uitgeschakeld in de native Mac-app.
8. De Rust-server is het enige accountsysteem; better-auth komt er niet.
9. Een vaste meeting-uid in de frontmatter, zodat hernoemen goed synct.
10. De herstelcode wordt nooit bewaard. "Nieuwe code" pakt de sleutel opnieuw in. De kluisgeneratie maakt "opnieuw beginnen" zichtbaar op alle toestellen. Geen sleutelrotatie bij afmelden in v1.
11. Gehoste modellen standaard via Mistral (EU), upstream configureerbaar; de app vraagt `pennel-auto`.
12. Een toestel wordt goedgekeurd bij `email/verify`; `device/lookup` toont alleen de toestelnaam. Er is geen aparte `approve`.
13. Een kluis aanmaken vraagt `If-None-Match: *`, zodat twee toestellen niet allebei een nieuwe kluis maken (409 `vault_exists` of `vault_missing`).
14. De limiet van 30 minuten audio geldt alleen voor WAV; andere formaten vertrouwen op de grens van 25 MB. Chat-requests zijn maximaal 4 MB.

## 7. Limieten in de beta
- **Sync:** 1 GB versleutelde opslag per account, 5 MB per bestand en 10 toestellen.
- **Gehoste modellen:** 20 uur audio en 2 miljoen tokens per kalendermaand per account. Dat is de fair use van Pro uit het businessplan.

Wie een limiet bereikt, krijgt een duidelijke melding, en lokaal blijft alles werken. De rechtencontrole zit in één functie (`entitlements(account) -> Limits`), zodat Free en Pro later alleen andere limieten worden. De website-claim "Sync is Pro" wordt bij de release aangepast naar wat dan echt geldt.
