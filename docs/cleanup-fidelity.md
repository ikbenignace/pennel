# Cleanup fidelity evidence

Issue [#60](https://github.com/pennel-app/pennel/issues/60) replaces output-word overlap with conservative ordered-content safeguards. The optional semantic verifier remains disabled by default, with an empty production qualification registry. Configuring a decision connection or passing its synthetic sample does not qualify automatic cleanup.

The guard permits punctuation, formatting, unambiguous hesitation removal, exact dictionary case/accent/spacing correspondence, English/Dutch cardinal integers 0–999 becoming equal digits, and bounded explicit corrections. Signs, numeric separators, dates, units, negation, questions and word order remain protected. Dictionary spacing cannot erase grammatical, numeric, temporal or location-word boundaries: a saved `nowhere` cannot turn `now here` into a different statement. Literal also protects code punctuation and identifier spelling. Ambiguous normalization, guessed spellings, larger spoken numbers and broader paraphrases retain raw text; these limits favor preservation over polish.

## Reproduce without inference spending

From the repository root:

```sh
python3 scripts/cleanup-fidelity.py --output build/cleanup-fidelity --freeze
python3 scripts/cleanup-fidelity.py --output build/cleanup-fidelity
```

The first command records fixture identity, development/held-out families, denominators, timing IDs and criteria without assessing proposals. The second refuses a changed manifest, runs the actual Rust guard and offline mock controls, then writes `quality.json`, `timing.json` and `report.md` under ignored `build/`. Select a new output directory for a deliberately revised panel; preserve earlier evidence. No credentials, external provider access or model downloads are used. The v1 fixture SHA-256 frozen before the first assessment is `58472711682e991a9fe6094f033b242aae7255a5dfade60f63e38907a2a77e03`.

The wrapper invokes a test-only Rust module with the existing toolchain and `--no-default-features`. Ordinary core tests check split isolation, counts, known regressions and harmful proposals. The ignored report test runs only through an explicit command. No benchmark library or production inference feature is added.

## Frozen panel and interpretation

Eight synthetic scenario families have English, Dutch and mixed-language variants. Harbor/orchard are development families; observatory/theater/clinic/museum/workshop/greenhouse are held out. All translations and perturbations of a family share its split. Distinct subjects, people, amounts and application terms keep adjacent variants together rather than distributing them across development and held-out sets.

Each family/language has eight harmful proposals (amount, date, recipient, negation, question conversion, unsupported addition, translation and role reversal) and six benign proposals (punctuation/fillers, formatting, dictionary correspondence, number formatting, explicit correction and a preserved question). The panel has 336 proposals, including 252 held-out proposals: **48 harmful and 36 benign per language**. Five previously known issue/#53 examples run separately and never count as unseen held-out evidence.

Admission criteria are fixed before the final run: mandatory regressions pass, held-out harmful acceptance is zero per language, and every benign rejection remains reported. No model confidence threshold is fitted or imported. The panel uses Neutral style with controlled saved dictionary terms; Literal and saved application-style behavior have separate targeted regressions. This small synthetic regression panel repeats category structures; it does not establish generalization to natural dictation, broader paraphrases or untested numeral forms, and does not qualify a model.

The comparison records accepted harmful / all harmful, raw fallback on benign / all benign, and abstentions / scheduled cases, plus reason counts. Deterministic mismatches are rejections. Semantic unavailable/timeout/cancellation outcomes are reported separately through actual mock controls. Quality records contain case IDs, lengths, hashes and reason codes, without raw or cleaned fixture content.

## Timing and provider-call evidence

The fixed timing panel selects one harmful and one benign proposal from every held-out family in each language: twelve cases per language. It excludes one warmup and measures thirty paired repetitions per case, alternating baseline/new order. Added p50/p95 use linear type-7 percentiles. These unoptimized Rust test timings run under uncontrolled concurrent machine load. CPU microbenchmarks are descriptive rather than pass/fail thresholds; raw paired samples remain inspectable.

Offline mock scenarios run separately with annotated English, Dutch and mixed-language inputs, recording generation/verifier dispatch counts, raw fallback and elapsed time. They distinguish deterministic rejection (`1 + 0`), qualified mock acceptance (`1 + 1`), dispatched failures/timeouts (`1 + 1`), unavailable qualification and cancellation before dispatch (zero verifier calls). These records test the implementation's bounded control flow. Programmed mock responses do not measure semantic accuracy. Live semantic latency, physical receiving-app behavior and provider/server cancellation remain unmeasured.

A separate mock overhead panel pairs generation-only cleanup with generation plus one typed verifier request, using thirty repetitions per annotated English/Dutch/mixed language and alternating order. It excludes one warmup pair per language from p50/p95 while preserving the warmup's actual call records. Every timed invocation uses a fresh loopback mock server/client. The panel measures the additional mock cleanup path rather than actual recognition or hosted-model stop-to-result latency.

## Qualification boundary

[The #53 evaluation](https://github.com/pennel-app/pennel/issues/53) found bounded research candidates and an actual mixed-language generation that translated `on Thursday` into Dutch. Its calibrated acceptance depended on the exact model, returned identity, route, prompt, ordered labels, envelope and language. This implementation changes that pipeline and copies no fitted cutoff.

Future semantic activation needs a separately reviewed qualification for the exact production pipeline and a trustworthy content-language policy. Recognition currently returns text without detected-language evidence; the interface language and recognition hint cannot supply that evidence. English, Dutch and mixed-language qualifications remain distinct, and no production default is selected here.
