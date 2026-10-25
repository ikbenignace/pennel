# Fase C — Betalen voor Pennel Cloud (ontwerp)

Stand: 2026-10-25. Repo's: `pennel-cloud` (server, website, Stripe-script) en `musli` (kern en gedeelde app). Bouwt verder op Pennel Cloud v1 (`docs/superpowers/specs/2026-10-25-meeting-sync-design.md`).

## 1. Wat en waarom

Pennel Cloud v1 brengt accounts, versleutelde meeting-sync en gehoste modellen. Tijdens de beta is dat gratis binnen één set limieten, die de server berekent in één functie: `entitlements(user) -> Limits` (`crates/pennel-server/src/entitlements.rs`). Fase C maakt van die bèta een product dat je kan kopen. Het businessplan (§3, §10 fase C) zegt het zo: "Iemand kan Pro kopen en het werkt meteen".

Stripe is al gekozen. `scripts/stripe_prices.sh` houdt de Stripe-prijzen gelijk met `apps/web/lib/plans.ts`. De omgevingsvariabelen `STRIPE_PRICE_{PRO,TEAM}_{MONTH,YEAR}` bestaan al, en `deploy-production` synchroniseert de prijzen bij elke promotie. Wat ontbreekt, is alles wat een betaling koppelt aan een account: checkout, webhooks, het abonnement in de database, limieten per plan, en een knop in de app.

**Klaar als:** iemand met een Cloud Free-account klikt in **Instellingen → Account** op **Upgraden naar Cloud Pro**. Hij betaalt in Stripe Checkout in de browser, en binnen een minuut toont de app "Cloud Pro" met de Pro-limieten. Opzeggen, een betaalmethode wijzigen en facturen bekijken gaat via **Abonnement beheren** (het Stripe-klantportaal). Wie het abonnement laat aflopen, valt terug naar Cloud Free zonder dat er lokaal iets verloren gaat.

## 2. Doelen, niet-doelen, anti-doelen

**Doelen**
- Plannen Cloud Free en Cloud Pro, met limieten per plan vanuit `entitlements()`. Cloud Team krijgt in deze fase alleen het datamodel; het product volgt in fase E.
- Upgraden, opzeggen en beheren vanuit de app, met Stripe Checkout en het Stripe-klantportaal.
- Stripe-webhooks op de Rust-server, als enige bron van waarheid voor de abonnementsstatus.
- De btw correct: Stripe Tax, prijzen voor consumenten inclusief btw (D21) en een btw-nummer optioneel bij het afrekenen.
- Een nette overgang uit de beta: een vaste einddatum, en daarna gelden de limieten per plan.
- Account verwijderen zegt ook het abonnement op en wist de Stripe-klant.

**Niet-doelen**
- Een webdashboard op `app.pennel.app`. Kopen en beheren gebeurt vanuit de app; het Stripe-portaal doet de rest.
- Kopen op de website zonder app. De prijspagina verwijst naar downloaden en upgraden in de app.
- Proefperiodes, kortingscodes, verwijzingen en lifetime-licenties. Promotiecodes van Stripe mogen wel aan staan in Checkout.
- Instellingen en de woordenlijst synchroniseren. Het businessplan noemt dat Pro, maar het is een aparte uitbreiding van sync.
- Facturatie per factuur of overschrijving. Dat komt met Team (fase E).
- Een iOS-aankoop via de App Store (fase 4 beslist dat).

**Anti-doelen**
- Een betaalfout of een webhookstoring neemt iemand lokaal iets af. Lokaal dicteren, meetings en eigen providers blijven altijd volledig, zonder account.
- Clouddata wordt gewist omdat een abonnement afloopt. Wat al op de server staat, blijft leesbaar en downloadbaar.
- Kaartgegevens, betaalgegevens of Stripe-payloads komen in onze database of logs. We bewaren alleen Stripe-ID's, plan, status en data.
- De app of de website toont prijzen die niet overeenkomen met wat Stripe aanrekent.
- Webhooks die zonder geldige handtekening of twee keer verwerkt worden.

## 3. Randvoorwaarden

- `AGENTS.md`: privé als standaard, en alleen claimen wat uitkomt. Prijzen en "Cloud Pro" verschijnen pas als beschikbaar wanneer kopen echt werkt (`PLANS[].available`).
- De Rust-server is het enige accountsysteem (sync-spec, beslissing 8). Er komt geen better-auth en geen tweede gebruikerstabel. `docs/web.md` in `pennel-cloud` noemt nog de port van het kbo-api-dashboard; dat vervalt.
- Er komt geen zware Stripe-SDK in de server. Stripe wordt aangesproken met `reqwest` (form-encoded), en webhooks worden met `hmac` + `sha2` geverifieerd.
- Niet-technische blokkers (businessplan §0 en §6): een ondernemingsnummer, toestemming van de werkgever, een OSS-registratie voor btw als Stripe niet de verkoper is, en gebruiksvoorwaarden en een privacyverklaring die door een jurist zijn nagekeken (`/legal/*` staat nog als concept).
- Afhankelijk van Pennel Cloud v1: server plan 1 (Task 4 entitlements, Task 6 proxy), kern plan 2 (Task 2 `CloudAccount`) en app plan 3 (Task 2 Account-tab).

## 4. Beslissingen

1. **Stripe Billing + Stripe Tax, met Pennel als verkoper.** Dat sluit aan bij wat al klaarstaat: de prijzen, `tax_behavior` en de deploy-sync. Stripe Managed Payments (Stripe als Merchant of Record) blijft een alternatief en is vraag 1. *(mijn voorstel)*
2. **Kopen alleen vanuit de app.**
   - `POST /v1/billing/checkout` geeft een Checkout-URL terug, en de app opent die in de browser.
   - `POST /v1/billing/portal` geeft een portaal-URL terug.
   - Na betalen landt de browser op een statische pagina `pennel.app/{lang}/billing/done`, met de tekst "Je abonnement is actief. Je kan dit venster sluiten en teruggaan naar Pennel."
   - Zo is er geen websessie en geen dashboard nodig.

   *(mijn voorstel)*
3. **Prijzen voor de app komen van de server** (`GET /v1/billing/plans`), en de server leest ze uit Stripe via de geconfigureerde price-ID's (gecachet). Zo kan de app nooit een ander bedrag tonen dan Stripe aanrekent. *(mijn voorstel)*
4. **De webhook is de enige bron van waarheid.** Bij elk relevant event haalt de server het abonnement opnieuw op bij Stripe (`GET /v1/subscriptions/{id}`) en slaat de actuele staat op. Zo maakt de volgorde van events niet uit. Event-ID's worden bewaard om dubbele verwerking te voorkomen. *(mijn voorstel)*
5. **De limieten per plan** (mijn voorstel; zie vraag 2):

   | | Cloud Free | Cloud Pro | Cloud Team (per gebruiker) |
   |---|---|---|---|
   | Gehoste modellen | 3 u audio + 200.000 tokens per maand | 60 u audio + 5 mln tokens per maand (40 u dicteren + 20 u meetings uit `FAIR_USE`) | zoals Pro |
   | Sync uploaden | nee (lezen en wissen wel) | ja | ja |
   | Opslag | wat er al staat | 5 GB | 5 GB |
   | Toestellen | 3 | 10 | 10 |

   Wie op Free sync aanzet of upload, krijgt `403 plan_required` met de melding "Sync is onderdeel van Cloud Pro. Wat al in Pennel Cloud staat, blijft leesbaar. Lokaal blijft alles werken." In de app wordt dat een nieuwe sync-status `PlanRequired`: pull blijft werken, push wacht.
6. **Overgang uit de beta.** `PENNEL_BETA_UNTIL` (een datum, in de omgeving van de server) laat iedereen tot die datum de bètalimieten houden. Daarna geldt het plan. De app toont vanaf 30 dagen vóór die datum in het Account-tabblad "De beta loopt tot {datum}. Daarna is sync onderdeel van Cloud Pro." *(mijn voorstel)*
7. **Wanbetaling.** De Stripe-status `past_due` krijgt 7 dagen Pro-limieten met de melding "Je betaling is mislukt. Werk je betaalmethode bij in Abonnement beheren." Daarna en bij `unpaid` of `canceled` geldt Free. `cancel_at_period_end` houdt Pro tot het einde van de periode. *(mijn voorstel)*
8. **Btw-weergave.** Pro wordt getoond en aangerekend inclusief btw (`tax_behavior=inclusive`, D21), en Team exclusief btw. Nu maakt `scripts/stripe_prices.sh` alles `exclusive` aan, terwijl `plans.ts` Pro `inclusive` noemt. Het script krijgt `tax_behavior` per rij uit `PLANS[].vat`, en de bestaande pariteitstest controleert dat. *(mijn voorstel; volgt D21, wacht op Ignace)*
9. **Account verwijderen** zegt eerst het abonnement meteen op en wist daarna de Stripe-klant. Weigert Stripe, dan geeft de server `503 billing` ("Probeer het later opnieuw") en wist hij niets. Zo betaalt niemand verder voor een gewist account. Facturen blijven in Stripe bewaard, zoals de boekhoudwet vraagt. *(mijn voorstel)*
10. **Native Mac-referentieapp:** de Account-tab is daar al uitgeschakeld (Pennel Cloud v1, app plan Task 4), en daar verandert niets. *(mijn voorstel)*

## 5. UI (gedeelde app, Nederlands)

Het Account-tabblad krijgt een nieuwe sectie **Abonnement** tussen "Account" en "Gebruik deze maand".

- **Free:**
  - De tekst `Cloud Free`, en daaronder de limieten van Free.
  - De knop `Upgraden naar Cloud Pro`, met de keuze `Per maand · {prijs}` / `Per jaar · {prijs} (2 maanden gratis)`. De prijzen komen van `GET /v1/billing/plans`.
  - Na de klik toont de app `Rond de betaling af in je browser.` Ze ververst het account bij focus en elke 5 s gedurende 10 minuten.
- **Pro:**
  - `Cloud Pro · verlengt op {datum}`, of bij opzegging `Cloud Pro · loopt af op {datum}`.
  - De knop `Abonnement beheren`.
- **Betaling mislukt:** de melding uit beslissing 7, met de knop `Abonnement beheren`.
- **Beta:** de tekst uit beslissing 6, zolang de beta loopt.

Een limietfout (`quota`, `plan_required`) in de providerkaart of de sync-status krijgt de knop `Upgraden`, die naar het Account-tabblad gaat.

## 6. Aan de bouwer overgelaten

- De tabelindeling binnen de grenzen van het plan, en of `stripe_events` na 30 dagen opgeruimd wordt.
- Hoe lang de prijzen van `GET /v1/billing/plans` gecachet worden (minstens 10 minuten).
- De exacte Stripe Checkout-opties, binnen de vaste set: `automatic_tax`, `tax_id_collection`, `billing_address_collection=required`, `allow_promotion_codes` en `locale=auto`.

## 7. Volgorde en afhankelijkheden

- **Na Pennel Cloud v1.** Fase C heeft de account-, entitlements- en proxyroutes nodig, en de Account-tab. Begin pas als v1 op staging draait.
- **Na distributie.** Verkoop pas als de app zonder waarschuwing te installeren is.
- **Vóór fase E (Team).** Team gebruikt dezelfde Stripe-klant-, checkout- en webhookcode.

## 8. Vragen voor Ignace

1. **Stripe Billing met jou als verkoper (btw zelf via OSS) of Stripe Managed Payments (Stripe als Merchant of Record, +3,5 %)?** Dat bepaalt de Checkout-configuratie, de btw-registratie en de facturen. *Standaard: Stripe Billing + Stripe Tax; overschakelen kan later zonder wijziging in de app.*
2. **Is sync na de beta alleen voor Pro, zoals in het businessplan?** Of krijgt Free een kleine sync, bijvoorbeeld 250 MB en 2 toestellen? Dat bepaalt `plan_required` en de UI. *Standaard: alleen Pro; Free kan lezen en wissen wat er staat.*
3. **Klopt de btw-weergave D21: Pro inclusief, Team exclusief?** *Standaard: ja; het script volgt `plans.ts`.*
4. **Wanneer eindigt de beta (`PENNEL_BETA_UNTIL`)?** Krijgen bètagebruikers een korting? *Standaard: 60 dagen na de lancering van betalen, zonder korting; een Stripe-promotiecode kan altijd later.*
5. **Klopt de Free-limiet van 3 u audio en 200.000 tokens per maand?** Het businessplan zegt "2.000 woorden per week + 2 meetings per maand", maar de server telt seconden en tokens. *Standaard: 3 u en 200.000 tokens.*
