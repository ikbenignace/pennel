# Decision-model evaluation

Measured results: [28 October 2026 report](../evaluations/decision-models/results/2026-10-28/README.md).
The recommendation is to retain current behavior and adopt no new model from this run.

Issue [#53](https://github.com/pennel-app/pennel/issues/53) owns this suite. Its corpus and
retrieval receipts are shared with [#43](https://github.com/pennel-app/pennel/issues/43).
Production decision/rerank provider contracts remain in
[#54](https://github.com/pennel-app/pennel/issues/54). This suite does not enable a product
feature, change a default model or enroll a speaker.

## Reproduce offline

From the repository root, with Python 3.11+ and the existing Rust dependencies cached:

```sh
python3 evaluations/decision-models/run.py --offline
python3 -B -m unittest discover -s evaluations/decision-models/tests -v
```

The first command builds the evaluation-only Rust executable with `--no-default-features
--features evaluation`. Cargo runs offline; if crate dependencies are not cached, prepare
them explicitly with the normal project build first. No speech or speaker model is loaded.
CI prepares ordinary build dependencies, then runs the same fixture and mocked-transport
checks without an API key, model download or inference request.

Replay the published actual inference scores with no optional model dependencies:

```sh
python3 evaluations/decision-models/run.py --offline \
  --replay evaluations/decision-models/results/2026-10-28
```

The compressed predictions are measured local inference, not canned mock replies. Replay
recomputes quality/calibration/ranking metrics; its resource observations retain the
original hardware and sample counts. A regression test compares the replay with the
recorded report, allowing only floating-point roundoff.

Reports are written under `build/decision-model-evaluation/offline/`. The semantic digest
covers deterministic scores rather than timing noise. Model inputs and predictions are
synthetic artifacts; diagnostics contain identifiers and sanitized errors, never secrets.
The Rust bridge creates and removes its own temporary library. It never opens the installed
Pennel profile, settings, meeting directory or credential store.

## Corpus and comparisons

The corpus generator and its provenance are versioned with the suite. There are 90 meeting
groups, with 10 development, 10 calibration and 10 held-out groups for each of English,
Dutch and mixed language. Related meetings and label-order variants stay in the same group.
Exact evidence offsets are Unicode code-point offsets within a transcript segment, notes
or summary. The core indexes the actual Markdown mirror; source spans are mapped to its
packed passages without assuming the first timestamp is a claim's exact position.

The deterministic baselines call the real Rust implementation:

- Public search uses its existing grouped full-text behavior.
- Ask uses its own whole-meeting/retrieved/recent selection. Candidate recall is measured
  before reranking; an omitted source cannot be recovered by a reranker.
- Cleanup measures the existing overlap/length guard, separately from generated-output
  fidelity. Accepting a corrupt synthetic proposal is a guard failure, not evidence that
  a provider produced that proposal.
- Citation parsing verifies syntax and source membership. Factual support needs the
  separate evidence rubric; a valid citation is insufficient.
- Template selection and text-based speaker names currently have no inference baseline.
  Their current automatic-suggestion coverage is zero. Acoustic recognition is outside
  this text benchmark.

The generative controls reuse Pennel's actual Q&A, summary/map-reduce and cleanup messages.
They require an explicitly configured available chat model. Mock replies only verify
transport/scoring. No other generative model search or diffusion cleanup comparison is
part of this issue.

## Freeze, calibrate and evaluate

`evaluations/decision-models/protocol.json` contains the task criteria, resource budgets,
comparison method, calibration rules and grounding rubric. Do not tune these after viewing
held-out results. Pinning includes source/corpus/protocol hashes, candidates, prompts,
ordered labels, question grouping, model/tokenizer artifacts, precision and hardware.

Prepare an experiment without scoring held-out model outputs:

```sh
python3 evaluations/decision-models/run.py --prepare \
  --config evaluations/decision-models/configs/experiment.local.json \
  --frozen evaluations/decision-models/locks/experiment.json
```

Commit the frozen manifest before held-out inference. Development runs may be used to
repair adapters and prompts. Fit calibration only on the calibration split; persist the
calibration context and threshold with the model/route/precision/prompt/label/grouping
identity. A change invalidates that calibration. A new experiment needs a new freeze.
The prompt-family hash binds all authored questions through the frozen corpus hash, task,
subtask and primitive; ordered labels remain separate contexts. Per-case wire requests
retain their own identities. Correct `unknown`/`abstain` decisions remain classification
outcomes, but never count toward actual name/template suggestion coverage.

```sh
python3 evaluations/decision-models/run.py --live --split development \
  --config evaluations/decision-models/configs/experiment.local.json \
  --frozen evaluations/decision-models/locks/experiment.json \
  --output build/decision-model-evaluation/development
```

Use the same configuration and committed freeze for calibration and held-out runs.
Incomplete judgments, identity drift and missing measurements are explicit acceptance gaps.
No aggregate score across languages selects a universal winner.

```sh
python3 evaluations/decision-models/run.py --live --split calibration \
  --config evaluations/decision-models/configs/measured-2026-10-28.json \
  --frozen evaluations/decision-models/locks/2026-10-28.json \
  --binary build/evaluation-core-target/release/pennel-eval \
  --output build/decision-model-evaluation/calibration
python3 evaluations/decision-models/run.py \
  --calibrate build/decision-model-evaluation/calibration/predictions.json \
  --frozen evaluations/decision-models/locks/2026-10-28.json \
  --output build/decision-model-evaluation/calibration
python3 evaluations/decision-models/run.py --live --split heldout --measure \
  --config evaluations/decision-models/configs/measured-2026-10-28.json \
  --frozen evaluations/decision-models/locks/2026-10-28.json \
  --calibration build/decision-model-evaluation/calibration/calibration.json \
  --binary build/evaluation-core-target/release/pennel-eval \
  --output build/decision-model-evaluation/heldout
```

Those historical live commands use the recorded generation commit `4a3135b`. For later
checkouts or hardware, use the preparation step with a new manifest path, commit it, and
substitute that path in each command. Do not overwrite the original measurement record.

The checked-in measured configuration describes this experiment, including unavailable
routes. Its OpenRouter account/retention gate is deliberately unverified. Supplying a key
does not bypass that gate. Changing the configuration requires a new freeze. The small
generative example's `max_cases` is a global smoke-test cap; omit it for full task/language
coverage, as the measured configuration does.

## Hosted routes and billing

OpenRouter is preferred when the requested version and required typed decision primitive
are available. The optional runner uses one explicitly selected `OPENROUTER_API_KEY`.
An environment key alone never enables inference. Copy an example into an ignored local
configuration, explicitly enable it, choose its retention/region policy, and name its
fixed model/upstream route. Keep the key out of JSON and Git.

The default API is `POST https://openrouter.ai/api/alpha/decisions`; explicit System One
mode uses `/api/v1/systemone`. Neither is chat completions. Preserve binary probabilities,
choice/ordinal distributions and confidence metadata. Discovery requests include
`output_modalities=decisions`; catalog presence is followed by capability/identity checks.

Pin `provider.only` to the exact endpoint tag and set `allow_fallbacks:false`. Do not use
latest aliases, automatic routers or model fallback arrays. Keep direct-provider runners
for explicitly configured missing capabilities or route comparisons; runtime failures do
not trigger a destination change. A regional rejection is not permission to use the global
domain. Provider/model identity and public checkpoint identity are separate: `liquid/d1`,
`liquid/d1.1` and `LiquidAI/d1-2B` are not interchangeable without exact evidence.

Record requested/actual model and provider, routing configuration, request/response ID,
usage and `usage.cost` for every attempt. Missing billed cost remains unknown. Account for
repeated state, grouped questions and any explicitly configured retry. The experiment
spend ceiling is US$2 total, not per provider or per split. Never present a catalog price
calculation as a returned billed charge.

OpenRouter end-to-end timing is its own series, separate from direct and local results.
Hosted client startup does not measure provider model load time or server cold start.
Provider-side memory and hardware remain unmeasured unless separately established.

Primary API references: [Decisions](https://openrouter.ai/docs/api/api-reference/alphadecisions/submit-a-decisions-request),
[System One](https://openrouter.ai/docs/guides/community/typesafe-sdk),
[catalog](https://openrouter.ai/docs/api/api-reference/models/list-all-models-and-their-properties),
[routing](https://openrouter.ai/docs/guides/routing/provider-selection),
[regional routing](https://openrouter.ai/docs/guides/features/in-region-routing).

## Evidence and remaining gates

Local artifacts use pinned revisions and isolated worker environments. Explicit downloads
are bounded by 3 GiB in total; no ordinary test downloads them. Runtime/worker size and
measured owned-process RSS are distinct from weight metadata and hypothetical native app
size. The optional runtime has not been embedded into a product package.

The suite publishes per-language recommendations to adopt, retain or defer for ranking,
answerability/support, meeting events, templates, names and cleanup. Adoption requires the
frozen task and privacy/resource criteria. Synthetic benchmark results do not complete
packaged-app, physical-device or Cloud deployment acceptance. Keep #53 open while its
required evidence is incomplete.

The first run verified 2,770,266,421 downloaded bytes and measured 3,144 successful held-out
cases, with 192 over-context cases retained as unavailable in the denominators. No hosted
requests were made. Full selected-route retention/region verification, hosted usage/cost
measurements, generative answer/citation support, Q8 parity, other local candidates and
packaged/platform acceptance remain explicit gaps. The small synthetic corpus is not
evidence of broad production accuracy.
