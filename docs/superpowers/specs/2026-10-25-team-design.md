# Fase E — Cloud Team (ontwerp)

Stand: 2026-10-25. Repo's: `pennel-cloud` (server, mail, website) en `musli` (kern en gedeelde app). Bouwt verder op Pennel Cloud v1 en op fase C (betalen).

## 1. Wat en waarom

Volgens het businessplan zit het geld in Team (§3: "B2B, consultancy, AE als eerste pilot"). De website belooft voor "Pennel Cloud for teams" al: gedeelde woordenlijst en sjablonen, centraal beheer en facturatie, EU-only verwerking en een DPA (marketing-spec §5, nog als *coming soon*).

Een team van tien consultants wil drie dingen:
- **Dezelfde vaktermen** goed laten schrijven: klantnamen, productnamen, afkortingen.
- **Dezelfde meetingverslagen** volgens het huissjabloon.
- **Eén factuur** op naam van het bedrijf, met een btw-nummer en een DPA die zegt dat alles in de EU verwerkt wordt.

Meetings delen hoort daar uitdrukkelijk **niet** bij. Elk teamlid houdt zijn meetings privé en end-to-end versleuteld, zoals in v1.

**Klaar als:** een beheerder maakt in de app een team aan en rekent af voor 5 plaatsen. Hij nodigt 4 collega's uit per mail en deelt een sjabloon "Klantgesprek" en een teamwoordenlijst. Die collega's aanvaarden in hun app. Daarna gebruiken ze binnen enkele minuten het sjabloon en de woorden, en ze hebben de Team-limieten. De beheerder ziet de facturen in het Stripe-portaal. Iemand uit het team verwijderen geeft hem zijn eigen Free-account terug, met al zijn meetings.

## 2. Doelen, niet-doelen, anti-doelen

**Doelen**
- Een team (workspace) met rollen: eigenaar, beheerder en lid. Eén team per account.
- Uitnodigen per e-mail, uitnodigingen aanvaarden of weigeren, leden verwijderen, zelf vertrekken en het eigenaarschap overdragen.
- Facturatie per plaats via Stripe, met het aantal leden als hoeveelheid, een btw-nummer en een factuur op bedrijfsnaam.
- Gedeelde sjablonen (alleen-lezen voor leden) en een teamwoordenlijst die samen met de eigen woordenlijst gebruikt wordt.
- EU-only verwerking als harde garantie: de proxy weigert een niet-EU-upstream voor teamleden.
- Leden krijgen de Team-limieten (per gebruiker zoals Pro, fase C).

**Niet-doelen**
- Meetings, notities of transcripten delen binnen het team.
- Beheer van de app-instellingen van leden: providers vastleggen, BYOK verbieden of lokale modellen afdwingen. Pennel is open source; we beloven geen controle die we niet hebben.
- SSO (Entra ID, Google Workspace), SCIM en domeinclaims. Die kunnen later, als een klant ernaar vraagt.
- Een webdashboard. Beheer gebeurt in de app, facturen in het Stripe-portaal (zoals in fase C).
- Betalen op factuur of per overschrijving, en meerdere teams per account.
- Een audit log voor beheerders.

**Anti-doelen**
- Een beheerder of Pennel kan de meetings, dictaten of geschiedenis van een lid zien. Geen enkel teamendpoint geeft objecten van een ander account terug.
- Bij het verwijderen of vertrekken verliest een lid lokale data of clouddata.
- Teaminhoud (sjablonen, woordenlijst) komt in logs.
- Een gedeeld sjabloon kan acties starten. Het blijft promptdata, zoals een eigen sjabloon (`AGENTS.md`: onbetrouwbare tekst start nooit acties).
- "EU-only" in de UI of op de website terwijl een request buiten de EU verwerkt kan worden.

## 3. Randvoorwaarden

- `AGENTS.md`: logica in de kern, identieke UI op Mac, Windows en Linux, Nederlands eerst, en de native Mac-app toont wat ze niet kan uitgeschakeld, met een reden.
- De Rust-server is het enige accountsysteem. Stripe-code, `billing_customers`, `subscriptions` en de webhook komen uit fase C.
- Niet-technisch: een DPA en een lijst van subverwerkers, nagekeken door een jurist (marketing-spec D27: geen "GDPR compliant"-claim), en Stripe Tax voor intracommunautaire verlegging bij B2B.

## 4. Beslissingen

1. **Eén team per account, en het lidmaatschap hangt aan het bestaande Pennel-account.** Er komt geen tweede login, en een lid houdt zijn persoonlijke vault en meetings. *(mijn voorstel)*
2. **Gedeelde sjablonen en de teamwoordenlijst zijn niet end-to-end versleuteld.** Het zijn teaminstellingen, geen meetinginhoud. De server moet ze aan elk lid kunnen geven, ook aan een nieuw lid zonder sleutel. De app zegt dat bij het delen eerlijk: "Gedeelde sjablonen en de teamwoordenlijst zijn leesbaar voor Pennel Cloud, zodat elk teamlid ze kan ophalen. Zet er geen vertrouwelijke inhoud in." *(mijn voorstel; zie vraag 2)*
3. **Beheer in de app, in een nieuwe sectie "Team" in de Account-tab.** Sjablonen deel je vanuit **Instellingen → Meetings** (`Delen met team` bij een eigen sjabloon). De teamwoordenlijst bewerk je in **Instellingen → Woordenlijst**. Leden zien beide alleen-lezen, met het label "Team". *(mijn voorstel)*
4. **Team-sjablonen leven niet in de meetingmap.** De kern bewaart ze in een cache in de app-datamap (`team.json`), niet in `.templates/`. Zo synchroniseren ze niet mee in de persoonlijke vault, en kan een lid ze niet per ongeluk overschrijven. Hun id's zijn `team-<slug>`. Verdwijnt een team-sjabloon, dan valt de samenvatting terug op het standaardsjabloon. *(mijn voorstel)*
5. **De woordenlijst die gebruikt wordt, is de eigen woordenlijst plus die van het team:** hoofdletterongevoelig ontdubbeld, de eigen termen eerst, en daarna beperkt door de bestaande prompt-limieten (`MAX_PROMPT_CHARS`). *(mijn voorstel)*
6. **Plaatsen.**
   - De Stripe-hoeveelheid is het aantal leden, inclusief de eigenaar.
   - Een aanvaarde uitnodiging verhoogt de hoeveelheid met proratie. Een verwijderd lid verlaagt ze vanaf de volgende periode.
   - Uitnodigen kan alleen met een actief Team-abonnement.
   - Een minimum van 2 plaatsen bij het afrekenen.

   *(mijn voorstel)*
7. **EU-only.** Elke upstream krijgt een regio (`PENNEL_STT_REGION`, `PENNEL_LLM_REGION`, standaard `eu` voor Mistral). Een team staat altijd op `eu_only`. Is de upstream niet `eu` voor een teamlid, dan antwoordt de proxy `503 eu_only`: "Pennel Cloud kan dit nu niet binnen de EU verwerken. Probeer het later opnieuw." Vandaag is alles Mistral (EU), dus dit is een slot op de toekomst, geen omleiding. *(mijn voorstel)*
8. **De eigenaar kan zijn account niet verwijderen zolang er andere leden zijn.** De server geeft dan `409 workspace_owner`: "Draag eerst het eigenaarschap over of verwijder het team." Een team verwijderen kan pas zonder actief abonnement. *(mijn voorstel)*
9. **Uitnodigingsmail** via de bestaande `Mailer`, met de tekst: "{naam} nodigt je uit in het team {team} op Pennel. Download Pennel en log in met dit e-mailadres om de uitnodiging te aanvaarden." De link gaat naar `pennel.app/download`. Er zit geen token in de mail: aanvaarden gebeurt in de app, na inloggen met hetzelfde adres. *(mijn voorstel)*
10. **Native Mac-referentieapp:** de Account-tab is daar al uitgeschakeld. De sectie "Teamwoordenlijst" in Woordenlijst staat er ook, uitgeschakeld, met "Beschikbaar in de nieuwe Pennel-app." *(mijn voorstel)*

## 5. UI (gedeelde app, Nederlands)

**Account → Team** (na "Abonnement"):
- **Geen team, geen uitnodiging:**
  - De tekst `Werk je met collega's? Met Cloud Team delen jullie sjablonen en een woordenlijst, met één factuur.`
  - Een veld `Naam van het team`, het aantal `Plaatsen` en de knop `Team starten` (Checkout per maand of per jaar).
- **Uitnodiging:** `{team} nodigt je uit.` met de knoppen `Aanvaarden` en `Weigeren`. Aanvaarden met een eigen Pro-abonnement toont `Je Cloud Pro-abonnement loopt nog. Zeg het op in Abonnement beheren; het team betaalt voortaan.`
- **Lid:** `Lid van {team}` · `Verwerking: EU` · de knop `Team verlaten`.
- **Beheerder of eigenaar:**
  - De ledenlijst met `{e-mail}` · `{rol}`, de knop `Verwijderen` en een rolkeuze `Lid` / `Beheerder` (rollen wijzigen kan alleen de eigenaar).
  - De openstaande uitnodigingen met `Intrekken`.
  - Het veld `E-mailadres` met de knop `Uitnodigen`.
  - De regel `{n} van {plaatsen} plaatsen in gebruik`.
  - De knoppen `Facturen en betaling` (het portaal) en `Eigenaarschap overdragen` (alleen eigenaar).

**Meetings → Sjablonen:**
- Team-sjablonen staan bovenaan, met het label `Team` en zonder bewerkknop. Er is een knop `Kopie maken` die er een eigen sjabloon van maakt.
- Een beheerder ziet bij eigen sjablonen `Delen met team`, en bij team-sjablonen `Niet meer delen`.

**Woordenlijst:** een sectie `Teamwoordenlijst`. Voor leden is ze alleen-lezen, voor beheerders bewerkbaar. Ze krijgt de uitleg uit beslissing 2.

## 6. Aan de bouwer overgelaten

- De tabelindeling, en of uitnodigingen na 14 of 30 dagen verlopen (minstens 14).
- Hoe vaak de app de teamconfiguratie ophaalt, minstens bij het opstarten en bij elke sync-runnertik met ETag/304.
- De maximumgrootte van een gedeeld sjabloon (minstens 32 KB) en van de teamwoordenlijst (minstens 1.000 termen).

## 7. Volgorde en afhankelijkheden

- **Na fase C.** Team gebruikt `billing_customers`, `subscriptions`, Checkout, het portaal en de webhook. Die krijgen hier alleen een `workspace_id` en een hoeveelheid.
- **Na Pennel Cloud v1.** Team gebruikt de accounts, de `Mailer`, de proxy en de sync-runner van de app.
- Kan pas starten als de **AE-pilot** (businessplan §9) een go heeft. Gebruik die pilot als acceptatietest.

## 8. Vragen voor Ignace

1. **Blijven de meetings van een lid altijd van het lid, ook na vertrek?** Of moet een bedrijf ze kunnen houden of laten wissen? Dat bepaalt of er ooit een team-vault komt. *Standaard: altijd van het lid; geen teamtoegang tot meetings.*
2. **Mogen gedeelde sjablonen en de teamwoordenlijst leesbaar zijn voor de server?** Een E2E-versie vraagt een teamsleutel per lid en is een veel groter werk. *Standaard: leesbaar voor de server, met een duidelijke melding.*
3. **Is een minimum van 2 plaatsen goed?** Mag iemand Team kopen voor zichzelf? *Standaard: minimum 2.*
4. **Moeten teams kunnen betalen op factuur of per overschrijving (`collection_method=send_invoice`)?** Grote klanten vragen dat vaak. *Standaard: nog niet; kaart of SEPA via Checkout.*
5. **Krijgt de AE-pilot het Team-plan gratis?** Dat kan met een Stripe-coupon van 100 % en vraagt geen code. *Standaard: ja, 6 maanden, via een coupon.*
