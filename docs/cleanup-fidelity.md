# Cleanup fidelity evidence

Historical commands that mention `make app`, the old iOS targets or retired SwiftUI shells refer to the dated candidates recorded below. Those targets were removed under [cleanup #75](https://github.com/pennel-app/pennel/issues/75); current builds use the [desktop](desktop.md#build-and-test) and [mobile](mobile.md#build-and-verification) workflows.

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

## Candidate evidence — 29 October 2026

Measured from code commit `b4347e4479dd5944591962aebef2eeb62ef5f6ad`. The [compact receipt](evidence/cleanup-fidelity-2026-10-29.json) records source/fixture identity, environment, counts, mock controls and complete local artifact hashes. The ordinary test profile is unoptimized; concurrent native builds contributed uncontrolled machine load. These numbers describe this synthetic panel.

| Language | Historical harmful acceptance | New harmful acceptance | Benign rejection | Deterministic abstention | Added CPU p50 / p95 | Added mock verifier p50 / p95 |
| --- | --- | --- | --- | --- | --- | --- |
| English | 48/48 | 0/48 | 0/36 | 0/84 | 0.128 / 0.211 ms | 1.674 / 1.871 ms |
| Dutch | 42/48 | 0/48 | 0/36 | 0/84 | 0.117 / 0.228 ms | 1.660 / 3.525 ms |
| Mixed | 42/48 | 0/48 | 0/36 | 0/84 | 0.111 / 0.184 ms | 1.646 / 2.990 ms |

CPU timing has 360 paired samples per language; mock verification has 30 paired samples per language. The mock timing panel dispatched 180 generation and 90 verification calls, plus six generation and three verification calls for excluded warmups. The 27 control scenarios dispatched 24 generation and 15 verification calls. Each language has five abstentions out of nine deliberately scheduled controls (unqualified, explicit abstention, timeout and two mutation cases); this is control-flow coverage rather than a field abstention estimate.

Qualified mock acceptance uses one generation and one verifier call; deterministic rejection uses one and zero. Unavailable qualification uses zero and zero. Dispatched rejection/abstention/timeout uses one and one and retains raw text. Mutation preserves raw text and never contacts the replacement server. The three-second timeout controls returned raw at approximately the shared deadline. Additional unit tests cover pre-dispatch cancellation, owner cancellation without History, service errors, malformed/oversized responses and Fast/practice zero-call behavior.

The test-only qualification cutoff is an arbitrary mock predicate. It is neither a transferred #53 threshold nor approval of any model. Live semantic latency and quality remain unmeasured, and all production qualification entries remain absent.

### Repository and packaged verification

The source candidate passes 681 default and 670 lean core tests (seven/five ignored, including two explicit report targets), formatting and Clippy with warnings denied. Desktop passes 217 frontend and 216 backend tests, type/build checks and backend formatting/Clippy. `make core && make test && make app` passes, including 53 Mac Swift tests. Shared theme checks, 39 packaging-script tests, two language-policy checks and 23 visual-fixture checks pass.

```sh
make core && make test && make app
npm --prefix apps/desktop ci
npm --prefix apps/desktop run build
npm --prefix apps/desktop test
scripts/with-toolchain.sh cargo test --manifest-path core/Cargo.toml --lib
scripts/with-toolchain.sh cargo test --manifest-path core/Cargo.toml --lib --no-default-features
scripts/with-toolchain.sh cargo fmt --manifest-path core/Cargo.toml -- --check
scripts/with-toolchain.sh cargo clippy --manifest-path core/Cargo.toml --lib --tests -- -D warnings
scripts/with-toolchain.sh cargo clippy --manifest-path core/Cargo.toml --lib --tests --no-default-features -- -D warnings
scripts/with-toolchain.sh cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
scripts/with-toolchain.sh cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check
scripts/with-toolchain.sh cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets -- -D warnings
scripts/build-desktop-preview.sh
```

The actual shared Tauri preview also builds. Its isolated preview identity, arm64 architecture, bundle metadata, strict development signature and disk-image integrity pass inspection. The DMG contains 14,908,235 bytes, SHA-256 `36b9a2f7616f5035b86b913a9f63898e3f4825ee9faec18fb4c80fcf3a1297b2`; this is local preview packaging, not production signing or installed-device acceptance.

Eight owned desktop captures cover English/Dutch and light/dark, with interaction checks for disabled qualification, disabling a saved preference, save-failure recovery and zero inference actions. Active Expo English/Dutch controls were also visually checked with the matching JavaScript bundle. The [mobile ledger](mobile.md#cleanup-fidelity--29-october-2026) records matched native builds, tests and product hashes. Real provider qualification, live latency, physical devices and installed Windows/Linux acceptance remain open. The fixture corpus was unchanged while additional numeric, dictionary-boundary and Literal regressions were repaired; earlier local reports remain in `build/cleanup-fidelity/5fb3746-published`.

## Merge integration — 30 October 2026

The combined candidate retains the evaluation, optional-provider, startup and Mac-material ancestry from PRs #64, #69, #66 and #71. Evaluation now calls the production cleanup assessment with each sample's style and dictionary; removed negation returns raw text. The generative runner forwards those same settings. Historical corpora, manifests, recorded predictions and reports remain unchanged; new offline output measures the current source separately.

Desktop integration preserves custom-provider key controls and credential-store errors. Settings/provider/key mutations release their inference guard before refreshing cleanup verification. Shared controls retain Figtree on Mac and other platforms.

The evaluation executable builds and eight Rust evaluation tests pass. All 253 offline Python tests, including historical replays, pass; the current offline baseline also completes. Reproduce with:

```sh
scripts/with-toolchain.sh cargo build --locked --manifest-path core/Cargo.toml --no-default-features --features evaluation --bin pennel-eval
scripts/with-toolchain.sh cargo test --locked --manifest-path core/Cargo.toml --no-default-features --features evaluation --lib evaluation
python3 -B -m unittest discover -s evaluations/decision-models/tests -v
python3 -B evaluations/decision-models/run.py --offline
```

The integrated frontend build and 222 tests, 23 visual-fixture tests, and 20 EN/NL light/dark interaction captures pass. Core/native packaging, backend checks and generated mobile builds remain required on the integrated head; current hosted results are attached to [PR #70](https://github.com/pennel-app/pennel/pull/70). These integration checks do not qualify a semantic model or satisfy physical-device/release acceptance.
