# Fase 3g — Microsoft 365-agenda (ontwerp)

Stand: 2026-10-25. Repo: `musli` (kern, gedeelde app, Mac-platformbibliotheek, native referentie).

## 1. Wat en waarom

Pennel geeft een meeting de titel en deelnemers van het agenda-item dat nu loopt, en laat je per agenda kiezen of een meeting altijd, nooit of op vraag wordt opgenomen (`core/src/rules.rs`, `decide_recording`). Vandaag komt die agenda alleen uit EventKit, de Mac-agenda (`apps/macos-platform/Sources/PennelMacPlatform/CalendarProvider.swift`). Op Windows en Linux staat de functie zichtbaar maar uitgeschakeld, met de reden "Agendagegevens zijn voorlopig alleen beschikbaar via de macOS-agenda." (`apps/desktop/src-tauri/src/capabilities.rs`).

Dat is het grootste pariteitsgat dat overblijft. De doelgroep (Benelux-consultants, AE als eerste pilot) werkt in Outlook en Microsoft Teams. Op de Mac lukt dat alleen als het werkaccount in Systeeminstellingen → Internetaccounts staat, en veel bedrijfstenants blokkeren dat.

Een rechtstreekse koppeling met Microsoft 365 via Microsoft Graph lost beide op: dezelfde functie op Mac, Windows en Linux, en ook op Macs waar de Internetaccounts-route dicht zit.

**Klaar als:** op Windows en Linux kan iemand in **Instellingen → Opnemen → Agenda** inloggen met zijn Microsoft 365-account via een code in de browser. Daarna krijgt een herkende Teams-call de titel en deelnemers van het lopende Outlook-item, en werken de regels per agenda zoals op de Mac. Op de Mac kan je kiezen tussen "Mac-agenda" en "Microsoft 365".

## 2. Doelen, niet-doelen, anti-doelen

**Doelen**
- Inloggen bij Microsoft 365 met de device-code flow (geen ingebouwde browser, geen redirect-server op localhost), voor werk- en schoolaccounts en persoonlijke Outlook.com-accounts.
- Agenda's oplijsten en het lopende item bepalen, met exact dezelfde regels op elk platform en voor elke bron.
- Eén bron per toestel: Mac-agenda (alleen op macOS) of Microsoft 365 (overal).
- Een duidelijke Nederlandse uitleg als de organisatie de app nog niet heeft goedgekeurd, met een link die de IT-beheerder kan gebruiken.
- De keuzelogica voor "het lopende item" verhuist naar de kern, zodat EventKit en Graph dezelfde regels volgen. Dat lost meteen een bekende bug op: bij overlappende items werd het eerste met een link gekozen.

**Niet-doelen**
- Google Calendar en CalDAV. Het kernmodel laat dat later toe, maar het zit niet in deze fase.
- Schrijven naar de agenda: geen notities of samenvattingen terug in het Outlook-item.
- Een opname laten stoppen bij het einde van het agenda-item (bekende beperking, apart onderwerp).
- Meerdere Microsoft-accounts tegelijk.
- De agenda via Pennel Cloud laten lopen. De app praat rechtstreeks met Microsoft.

**Anti-doelen**
- Pennel leest of bewaart meer dan nodig: geen mailbody, geen bijlagen, geen agendabeschrijving (`body`).
- Agendagegevens of tokens komen in logs, in de meetingmap (behalve titel en deelnemers, zoals vandaag) of op een Pennel-server.
- Een scope met schrijfrechten (`Calendars.ReadWrite`, `Mail.*`), of `User.Read.All`.
- Een kapotte of trage Graph-verbinding houdt de opnameprompt tegen. Zonder agenda gaat de prompt gewoon door, zoals vandaag zonder toegang.

## 3. Randvoorwaarden

- `AGENTS.md`: logica in de kern, tokens alleen in de OS-sleutelopslag, identieke UI en Nederlandse tekst op elk platform, een functie die een platform niet heeft tonen we uitgeschakeld met reden, en het gat komt in `docs/desktop.md`.
- Microsoft vereist een **Azure-app-registratie** met:
  - multi-tenant plus persoonlijke accounts;
  - "Allow public client flows" aan;
  - gedelegeerde rechten `Calendars.Read`, `offline_access`, `openid` en `profile`.

  De client-ID is publiek en mag in de app; er is geen client secret. Bedrijfstenants die gebruikers geen toestemming laten geven, hebben **admin consent** nodig (open-items §3: "mogelijk AE-admin-goedkeuring").
- HTTP gaat via de bestaande `ureq` in de kern. Er komt geen MSAL en geen Microsoft-SDK.
- De refresh token gaat in de OS-sleutelopslag via de `SecretStore`-trait. Die trait komt uit Pennel Cloud v1, kerntaak 1 (`core/src/cloud/mod.rs` op `feat/pennel-cloud-v1`, commit `d52e219`).

## 4. Beslissingen

1. **Device-code flow tegen `login.microsoftonline.com/common`.** De app toont de code en opent `https://microsoft.com/devicelogin`. Dat is hetzelfde patroon als het inloggen bij Pennel Cloud, dus het hergebruikt dezelfde poll-hulp en hetzelfde codescherm. Er komt geen localhost-redirect, die op Linux en achter bedrijfsfirewalls vaker faalt. *(mijn voorstel)*
2. **Minimale scopes: `openid profile offline_access Calendars.Read`.** De accountnaam die we tonen, komt uit de `preferred_username`-claim van het id-token. Dat token wordt alleen gedecodeerd om die naam te tonen, niet gevalideerd, en we vragen geen `User.Read`. *(mijn voorstel)*
3. **Eén agendabron per toestel**, in de nieuwe instelling `calendar_source`: `system` (de Mac-agenda) of `microsoft365`. De standaard is `system` op macOS en `microsoft365` elders. Twee bronnen samenvoegen zou bij Outlook-in-EventKit dubbele items geven. *(mijn voorstel)*
4. **Graph-velden:** `$select=subject,start,end,isAllDay,isCancelled,showAs,responseStatus,onlineMeeting,location,attendees`, met `Prefer: outlook.timezone="UTC"`. Ophalen per agenda via `/me/calendars/{id}/calendarView` voor nu −12 u tot +12 u, met maximaal 10 agenda's. Het resultaat blijft 5 minuten in het geheugen, niet op schijf. *(mijn voorstel)*
5. **Snel falen.** Het ophalen tijdens een detectie heeft een time-out van 5 s. Bij een fout of time-out gaat de prompt verder zonder agenda, en de fout komt in het log als status en duur. *(mijn voorstel)*
6. **De keuze van het lopende item gebeurt in de kern** (`calendar::pick_current`):
   - Overgeslagen worden items die de hele dag duren, geannuleerd zijn, geweigerd zijn of als "vrij" staan.
   - Een item telt mee vanaf 5 minuten vóór de start tot het einde.
   - Een item met een meetinglink krijgt voorrang.
   - Daarna wint het item waarvan de start het dichtst bij nu ligt.

   De Swift-laag levert voortaan ruwe items en kiest niet meer zelf. *(mijn voorstel)*
7. **Deelnemers zijn namen, zonder e-mailadressen.** De ingelogde gebruiker wordt weggelaten, zoals EventKit `isCurrentUser` weglaat. Een deelnemer zonder naam valt weg. Er gaan geen e-mailadressen naar de frontmatter. *(mijn voorstel)*
8. **Admin consent.** Komt er een `AADSTS65001`, `AADSTS90094` of een vergelijkbare toestemmingsfout terug, dan toont de app: "Je organisatie moet Pennel eerst goedkeuren. Stuur deze link naar je IT-beheerder." Daaronder staat de admin-consent-URL `https://login.microsoftonline.com/organizations/v2.0/adminconsent?client_id=…&scope=https://graph.microsoft.com/Calendars.Read`, met de knop **Link kopiëren**. *(mijn voorstel)*
9. **Native Mac-referentieapp:** de bronkeuze staat er, met "Microsoft 365" uitgeschakeld en de reden "Beschikbaar in de nieuwe Pennel-app." Zo doen we het ook met Pennel Cloud. *(mijn voorstel)*
10. **De client-ID staat in de kern als constante** en kan in tests en ontwikkeling overschreven worden met `PENNEL_M365_CLIENT_ID`. Een build zonder ingevulde client-ID toont de Microsoft 365-optie uitgeschakeld met de reden "Microsoft 365 is in deze build niet ingesteld." *(mijn voorstel)*

## 5. UI (gedeelde app, Nederlands)

In **Instellingen → Opnemen → Agenda**, in deze volgorde:

1. **Bron.** Alleen op macOS: een keuzelijst `Mac-agenda` / `Microsoft 365`. Op Windows en Linux staat er geen keuzelijst, en is de bron altijd Microsoft 365.
2. **Microsoft 365, niet ingelogd:**
   - De tekst `Geeft meetings de titel en deelnemers uit je Outlook-agenda. Pennel leest alleen je agenda, nooit je mail.`
   - De knop `Inloggen bij Microsoft 365`.
3. **Codescherm:**
   - De tekst `Je browser is geopend. Vul daar deze code in:`, gevolgd door de code in groot schrift.
   - De knoppen `Browser opnieuw openen` en `Annuleren`.
   - Na 15 minuten: `De code is verlopen.` met de knop `Opnieuw beginnen`.
4. **Ingelogd:** `Verbonden als {account}` met de knop `Afmelden`, en daaronder de schakelaar `Agenda gebruiken` en de lijst met agenda's en hun regels, zoals vandaag.
5. **Toestemming nodig:** de tekst uit beslissing 8, met de knop `Link kopiëren`.

De bestaande tekst voor de Mac-agenda blijft staan bij de bron `Mac-agenda`.

## 6. Aan de bouwer overgelaten

- Of de Graph-client een eigen `ureq`-agent heeft of `http::agent` hergebruikt.
- De exacte JSON-vorm tussen de Swift-bridge en Rust voor de ruwe EventKit-items, binnen het `CalendarEvent`-record van de kern.
- Of de cache per agenda of per bron leeft.

## 7. Volgorde en afhankelijkheden

- Hangt **niet** af van Pennel Cloud als dienst. Er is alleen een codeafhankelijkheid: de `SecretStore`-trait uit Pennel Cloud v1, kerntaak 1. Is die nog niet op `main` als 3g start, dan wordt alleen die commit eerst naar `main` gebracht.
- Kan **nu** starten, parallel aan distributie. Ignace moet wel eerst de Azure-app-registratie doen. Begin daarmee, want publisher verification en een eventuele AE-goedkeuring kosten doorlooptijd.

## 8. Vragen voor Ignace

1. **In welke tenant registreren we de app?** Dat bepaalt de uitgeversnaam die gebruikers in het Microsoft-toestemmingsscherm zien. *Standaard: een eigen "Pennel"-tenant, niet die van AE, met publisher verification zodra het ondernemingsnummer er is.*
2. **Ook persoonlijke Outlook.com-accounts?** Dat bepaalt of we `common` of `organizations` gebruiken. *Standaard: ja, `common`.*
3. **Mag Microsoft 365 op de Mac een alternatief zijn voor de Mac-agenda?** Of is het alleen voor Windows en Linux? *Standaard: ja, als keuze; de Mac-agenda blijft de standaard op macOS.*
