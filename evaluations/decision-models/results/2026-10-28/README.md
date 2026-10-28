# Decision-model results — 28 October 2026

**Recommendation: retain current behavior and adopt no new model from this run.** Every task has an unmet quality or evidence gate. Fast CPU inference did not imply adequate Dutch/English/mixed-language task quality. These findings apply to the frozen zero-shot prompts and artifacts, not every possible use of these models.

## Evidence and reproduction

Generation commit: [`4a3135b`](https://github.com/pennel-app/pennel/commit/4a3135b99ede8363493eb13e599b19b796a88603). The protocol, corpus, candidates/prompts, model artifacts, hardware and latency panel were frozen before held-out inference. The manifest SHA-256 is `f89a727418bc7b85676f5a2367b92bd3a6e96757814e27b479d5e25a431b692c`. Subsequent portability changes correct a Windows-only test assertion and normalize protocol checkout line endings; neither changes recorded inference or its numerical results. Live reproduction of the recorded experiment uses that generation commit; a current-checkout live experiment must prepare and commit a fresh manifest.

Run from the repository root, with normal Rust dependencies cached:

```sh
python3 evaluations/decision-models/run.py --offline \
  --replay evaluations/decision-models/results/2026-10-28
```

This regenerates deterministic core metrics and replays the actual measured predictions without model downloads, credentials or inference. `report.json.gz`, `resources.json`, `core-resources.json`, `calibration.json`, and compressed calibration/held-out prediction streams contain the full metrics and identities. `artifact-receipts.json` records verified downloads. No model weights are committed. See [the protocol and live commands](../../../../docs/decision-model-evaluation.md).

The corpus has 90 fictional meetings: 10 development, 10 calibration and 10 held-out groups per language. Held-out classification contains 1,620 cases per classifier; each rejected 96 over-context cases instead of truncating them. GLiClass and GTE each evaluated 48 frozen-candidate ranking cases. Total held-out results: 3,144 successful measured cases and 192 unavailable cases. Calibration used different meeting groups and yielded 36 fitted contexts; 12 contexts lacked sufficient samples. GLiNER returned selected labels/confidence without complete distributions, so its probability calibration, Brier and ECE remain unmeasured.

All 90 meetings use controlled authored grammar with split-disjoint topic domains. This is a stress suite, not a representative natural-meeting dataset. Label-order variants remain in the same meeting group and do not create additional independent meetings.

## Retrieval and ranking

Question means below cover the 15 answerable ranking queries per language; the 16th query is unanswerable and has undefined nDCG/recall. Whole-meeting context is reported separately, with no ranking nDCG. The public grouped-search control reuses Ask-style questions and is not a search-UI usability measurement.

| Model | Language | Baseline nDCG@5 | Reranked nDCG@5 | Baseline recall@5 | Reranked recall@5 | Fixed candidate recall |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| gliclass-multilingual | en | 0.589 | 0.733 | 0.933 | 0.733 | 0.933 |
| gliclass-multilingual | mixed | 0.571 | 0.829 | 0.933 | 0.867 | 1.000 |
| gliclass-multilingual | nl | 0.572 | 0.132 | 0.933 | 0.267 | 0.933 |
| gte-reranker | en | 0.589 | 0.629 | 0.933 | 0.933 | 0.933 |
| gte-reranker | mixed | 0.571 | 0.574 | 0.933 | 1.000 | 1.000 |
| gte-reranker | nl | 0.572 | 0.638 | 0.933 | 0.933 | 0.933 |

The preregistered comparison gives equal weight to each of 10 meeting groups, rather than each query. Its point estimate and 95% paired-bootstrap interval therefore differ from the descriptive question means above. Selection requires ≥0.03 gain, a strictly positive lower bound and no evidence-recall regression.

| Model | Language | Meeting-weighted nDCG gain | 95% CI | Result |
| --- | --- | ---: | --- | --- |
| gliclass-multilingual | en | 0.313 | [0.201, 0.369] | Fail: recall@5 regresses |
| gliclass-multilingual | mixed | 0.341 | [0.285, 0.369] | Fail: recall@5 regresses |
| gliclass-multilingual | nl | -0.583 | [-0.631, -0.488] | Fail: recall@5 regresses |
| gte-reranker | en | -0.001 | [-0.039, 0.037] | Fail: gain/confidence interval |
| gte-reranker | mixed | -0.054 | [-0.105, -0.003] | Fail: gain/confidence interval |
| gte-reranker | nl | 0.016 | [0.000, 0.049] | Fail: gain/confidence interval |

GLiClass improves English/mixed ranking nDCG but lowers recall@5 and strongly degrades Dutch. GTE does not meet the frozen meeting-weighted gain/confidence requirements. Candidate recall is unchanged by reranking; it is only 0.933 for English/Dutch in this panel, so #43 still owns candidate-recall improvements. Retain current retrieval in all three languages.

## Task quality and calibration

Raw coverage includes explicitly unavailable cases as abstentions. Precision/recall/F1 are macro values. Brier is the unnormalised categorical definition (range 0–2); binary Brier, per-class counts, ECE bins and selective curves are in the compressed `report.json.gz` (and regenerated `report.json`). These are actual CPU predictions, not mock replies.

| Runner | Language | Task | Precision | Recall | F1 | Raw coverage | Brier | ECE |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| gliclass-multilingual | en | answerability | 0.263 | 0.250 | 0.256 | 93.3% | 0.717 | 0.354 |
| gliclass-multilingual | en | claim_support | 0.333 | 0.450 | 0.383 | 90.0% | 0.532 | 0.231 |
| gliclass-multilingual | en | cleanup | 0.333 | 0.500 | 0.400 | 100.0% | 0.493 | 0.150 |
| gliclass-multilingual | en | events/kind | 0.035 | 0.163 | 0.057 | 90.0% | 0.918 | 0.327 |
| gliclass-multilingual | en | events/owner | 1.000 | 0.900 | 0.947 | 90.0% | 0.018 | 0.083 |
| gliclass-multilingual | en | speaker_names | 0.483 | 0.533 | 0.475 | 90.0% | 0.668 | 0.217 |
| gliclass-multilingual | en | templates | 0.618 | 0.633 | 0.591 | 95.0% | 0.691 | 0.394 |
| gliclass-multilingual | mixed | answerability | 0.250 | 0.225 | 0.237 | 93.3% | 0.779 | 0.428 |
| gliclass-multilingual | mixed | claim_support | 0.333 | 0.450 | 0.383 | 90.0% | 0.563 | 0.235 |
| gliclass-multilingual | mixed | cleanup | 0.333 | 0.500 | 0.400 | 100.0% | 0.494 | 0.163 |
| gliclass-multilingual | mixed | events/kind | 0.175 | 0.121 | 0.089 | 90.0% | 0.860 | 0.256 |
| gliclass-multilingual | mixed | events/owner | 0.500 | 0.250 | 0.333 | 90.0% | 0.459 | 0.156 |
| gliclass-multilingual | mixed | speaker_names | 0.399 | 0.517 | 0.440 | 90.0% | 0.650 | 0.153 |
| gliclass-multilingual | mixed | templates | 0.618 | 0.667 | 0.629 | 95.0% | 0.716 | 0.346 |
| gliclass-multilingual | nl | answerability | 0.414 | 0.375 | 0.360 | 93.3% | 0.848 | 0.377 |
| gliclass-multilingual | nl | claim_support | 0.433 | 0.412 | 0.410 | 90.0% | 0.487 | 0.012 |
| gliclass-multilingual | nl | cleanup | 0.167 | 0.500 | 0.250 | 100.0% | 0.719 | 0.371 |
| gliclass-multilingual | nl | events/kind | 0.042 | 0.225 | 0.070 | 90.0% | 0.972 | 0.490 |
| gliclass-multilingual | nl | events/owner | 0.500 | 0.125 | 0.200 | 90.0% | 0.747 | 0.412 |
| gliclass-multilingual | nl | speaker_names | 0.413 | 0.400 | 0.327 | 90.0% | 0.766 | 0.304 |
| gliclass-multilingual | nl | templates | 0.646 | 0.733 | 0.635 | 95.0% | 0.540 | 0.271 |
| gliner-multilingual | en | answerability | 0.263 | 0.250 | 0.256 | 93.3% | — | — |
| gliner-multilingual | en | claim_support | 0.333 | 0.450 | 0.383 | 90.0% | — | — |
| gliner-multilingual | en | cleanup | 0.688 | 0.583 | 0.416 | 100.0% | — | — |
| gliner-multilingual | en | events/kind | 0.042 | 0.225 | 0.070 | 90.0% | — | — |
| gliner-multilingual | en | events/owner | 0.000 | 0.000 | 0.000 | 90.0% | — | — |
| gliner-multilingual | en | speaker_names | 0.185 | 0.200 | 0.192 | 90.0% | — | — |
| gliner-multilingual | en | templates | 0.631 | 0.633 | 0.583 | 95.0% | — | — |
| gliner-multilingual | mixed | answerability | 0.269 | 0.263 | 0.266 | 93.3% | — | — |
| gliner-multilingual | mixed | claim_support | 0.333 | 0.450 | 0.383 | 90.0% | — | — |
| gliner-multilingual | mixed | cleanup | 0.688 | 0.583 | 0.416 | 100.0% | — | — |
| gliner-multilingual | mixed | events/kind | 0.042 | 0.225 | 0.070 | 90.0% | — | — |
| gliner-multilingual | mixed | events/owner | 0.000 | 0.000 | 0.000 | 90.0% | — | — |
| gliner-multilingual | mixed | speaker_names | 0.129 | 0.200 | 0.157 | 90.0% | — | — |
| gliner-multilingual | mixed | templates | 0.429 | 0.600 | 0.450 | 95.0% | — | — |
| gliner-multilingual | nl | answerability | 0.280 | 0.287 | 0.284 | 93.3% | — | — |
| gliner-multilingual | nl | claim_support | 0.333 | 0.450 | 0.383 | 90.0% | — | — |
| gliner-multilingual | nl | cleanup | 0.688 | 0.583 | 0.416 | 100.0% | — | — |
| gliner-multilingual | nl | events/kind | 0.292 | 0.233 | 0.088 | 90.0% | — | — |
| gliner-multilingual | nl | events/owner | 0.333 | 0.033 | 0.061 | 90.0% | — | — |
| gliner-multilingual | nl | speaker_names | 0.263 | 0.225 | 0.205 | 90.0% | — | — |
| gliner-multilingual | nl | templates | 0.427 | 0.533 | 0.407 | 95.0% | — | — |

Frozen GLiClass selective results follow. Name/template coverage counts actual accepted suggestions, excluding `unknown`/`abstain`; other rows describe accepted classifier decisions. Calibration fits only the calibration split, with distinct language/label-order contexts.

| Language | Task | Selective accuracy/precision | Coverage | Calibrated Brier | Calibrated ECE |
| --- | --- | ---: | ---: | ---: | ---: |
| en | answerability | 50.0% | 10.0% | 0.438 | 0.025 |
| en | claim_support | 100.0% | 5.0% | 0.448 | 0.007 |
| en | cleanup | 100.0% | 5.6% | 0.448 | 0.028 |
| en | events/kind | — | 0.0% | 0.755 | 0.139 |
| en | events/owner | — | 0.0% | — | — |
| en | speaker_names | 88.2% | 56.7% | 0.203 | 0.112 |
| en | templates | 100.0% | 17.5% | 0.634 | 0.399 |
| mixed | answerability | 66.7% | 15.0% | 0.440 | 0.040 |
| mixed | claim_support | 66.7% | 5.0% | 0.447 | 0.012 |
| mixed | cleanup | 100.0% | 7.2% | 0.440 | 0.087 |
| mixed | events/kind | — | 0.0% | 0.753 | 0.126 |
| mixed | events/owner | — | 0.0% | — | — |
| mixed | speaker_names | 96.0% | 41.7% | 0.196 | 0.087 |
| mixed | templates | 100.0% | 17.5% | 0.646 | 0.331 |
| nl | answerability | 90.9% | 18.3% | 0.463 | 0.091 |
| nl | claim_support | 80.0% | 8.3% | 0.445 | 0.010 |
| nl | cleanup | 100.0% | 13.3% | 0.442 | 0.017 |
| nl | events/kind | 66.7% | 5.0% | 0.744 | 0.157 |
| nl | events/owner | — | 0.0% | — | — |
| nl | speaker_names | 100.0% | 28.3% | 0.395 | 0.188 |
| nl | templates | 100.0% | 35.0% | 0.552 | 0.282 |

Answerability/support never reaches the required 50% coverage at adequate precision. Event-kind F1 remains below 0.1 in these configurations. Templates achieve calibrated precision of 1.0 only at 17.5–35% suggestion coverage, below the frozen 50% requirement. Speaker names fail precision and/or coverage; raw GLiClass also forces names in ambiguous cases. Retain manual naming/templates and current summaries; defer the new classifiers.

## Cleanup fidelity

Each deterministic core language set has 30 valid and 60 critical corrupt proposals. The current overlap guard accepts all 60 corrupt proposals and all 30 valid proposals. This measures a guard limitation on authored proposals; it does not establish that any generative provider produced those corruptions.

Decision-model sets contain 60 valid and 120 critical-unsafe cases per language because label orders are evaluated separately. A confident **reject** is a covered classification decision, not acceptance of a valid cleanup.

| Runner | Language | Raw valid proposals allowed | Raw corrupt proposals allowed | Calibrated valid allowed | Calibrated corrupt allowed |
| --- | --- | ---: | ---: | ---: | ---: |
| gliclass-multilingual | en | 0/60 | 0/120 | 0/60 | 0/120 |
| gliclass-multilingual | mixed | 0/60 | 0/120 | 0/60 | 0/120 |
| gliclass-multilingual | nl | 60/60 | 120/120 | 0/60 | 0/120 |
| gliner-multilingual | en | 60/60 | 100/120 | Unmeasured | Unmeasured |
| gliner-multilingual | mixed | 60/60 | 100/120 | Unmeasured | Unmeasured |
| gliner-multilingual | nl | 60/60 | 100/120 | Unmeasured | Unmeasured |

Rejecting every valid cleanup fails the ≥90% valid-acceptance requirement even when no corruption is permitted. No model qualifies. Retain the raw-text fallback and manual review; do not describe the existing overlap guard as semantic verification. Production changes belong to #60.

## Latency, memory, size and cost

Apple M4 Pro, 48 GiB RAM, macOS 27.0.1; isolated CPU workers use four inference threads and FP32 for the three measured models. Artifact/tokenizer/runtime identities are in the receipts. Five fresh starts per language provide 15 load samples per model. Thirty warm samples follow an excluded warmup for each fixed task/language panel case. Cold means a fresh owned process; OS file caches are uncontrolled. Load includes artifact verification and framework imports.

| Model | Artifact MiB | Load p50 / p95 ms (n=15) | Peak owned RSS GiB | Native app binary size |
| --- | ---: | --- | ---: | --- |
| gliclass-multilingual | 305.6 | 2113.491 / 2363.528 | 1.539 | Unmeasured; runtime not embedded |
| gliner-multilingual | 1111.5 | 6530.082 / 6650.846 | 3.331 | Unmeasured; runtime not embedded |
| gte-reranker | 599.9 | 1814.056 / 2078.010 | 2.048 | Unmeasured; runtime not embedded |

| Model | Language | Cold representative task | Cold first inference p50 / p95 ms (n=5) |
| --- | --- | --- | --- |
| gliclass-multilingual | en | cleanup | 30.272 / 30.996 |
| gliclass-multilingual | mixed | cleanup | 26.419 / 26.959 |
| gliclass-multilingual | nl | cleanup | 25.357 / 36.437 |
| gliner-multilingual | en | cleanup | 69.200 / 79.272 |
| gliner-multilingual | mixed | cleanup | 69.682 / 70.290 |
| gliner-multilingual | nl | cleanup | 70.565 / 74.892 |
| gte-reranker | en | ranking | 274.784 / 311.359 |
| gte-reranker | mixed | ranking | 287.664 / 305.092 |
| gte-reranker | nl | ranking | 286.582 / 299.429 |

| Model | Language | Task | Warm p50 / p95 ms (n=30) |
| --- | --- | --- | --- |
| gliclass-multilingual | en | answerability | 15.664 / 17.640 |
| gliclass-multilingual | en | claim_support | 47.464 / 50.606 |
| gliclass-multilingual | en | cleanup | 18.739 / 19.943 |
| gliclass-multilingual | en | events | 47.325 / 57.574 |
| gliclass-multilingual | en | ranking | 262.224 / 277.116 |
| gliclass-multilingual | en | speaker_names | 48.331 / 52.971 |
| gliclass-multilingual | en | templates | 16.131 / 17.400 |
| gliclass-multilingual | mixed | answerability | 21.659 / 25.601 |
| gliclass-multilingual | mixed | claim_support | 50.903 / 53.289 |
| gliclass-multilingual | mixed | cleanup | 22.599 / 24.536 |
| gliclass-multilingual | mixed | events | 51.231 / 64.858 |
| gliclass-multilingual | mixed | ranking | 248.965 / 270.769 |
| gliclass-multilingual | mixed | speaker_names | 49.695 / 50.564 |
| gliclass-multilingual | mixed | templates | 15.660 / 16.336 |
| gliclass-multilingual | nl | answerability | 20.524 / 21.960 |
| gliclass-multilingual | nl | claim_support | 60.689 / 62.796 |
| gliclass-multilingual | nl | cleanup | 20.500 / 22.440 |
| gliclass-multilingual | nl | events | 61.025 / 62.652 |
| gliclass-multilingual | nl | ranking | 255.258 / 277.737 |
| gliclass-multilingual | nl | speaker_names | 59.282 / 60.574 |
| gliclass-multilingual | nl | templates | 20.657 / 22.024 |
| gliner-multilingual | en | answerability | 40.401 / 44.561 |
| gliner-multilingual | en | claim_support | 145.301 / 164.688 |
| gliner-multilingual | en | cleanup | 44.419 / 49.947 |
| gliner-multilingual | en | events | 142.446 / 145.276 |
| gliner-multilingual | en | speaker_names | 150.187 / 152.232 |
| gliner-multilingual | en | templates | 43.614 / 45.081 |
| gliner-multilingual | mixed | answerability | 40.241 / 42.142 |
| gliner-multilingual | mixed | claim_support | 143.634 / 146.154 |
| gliner-multilingual | mixed | cleanup | 46.410 / 48.787 |
| gliner-multilingual | mixed | events | 145.378 / 152.668 |
| gliner-multilingual | mixed | speaker_names | 143.529 / 159.582 |
| gliner-multilingual | mixed | templates | 45.147 / 50.380 |
| gliner-multilingual | nl | answerability | 45.047 / 46.340 |
| gliner-multilingual | nl | claim_support | 156.569 / 160.248 |
| gliner-multilingual | nl | cleanup | 47.367 / 61.608 |
| gliner-multilingual | nl | events | 158.251 / 174.387 |
| gliner-multilingual | nl | speaker_names | 157.522 / 160.634 |
| gliner-multilingual | nl | templates | 44.482 / 45.605 |
| gte-reranker | en | ranking | 267.584 / 287.147 |
| gte-reranker | mixed | ranking | 272.083 / 296.363 |
| gte-reranker | nl | ranking | 273.584 / 283.764 |

Measured warm task budgets and the 24 GiB owned-worker memory ceiling pass on this panel; quality remains the limiting factor. Cold timing for other task cells, Windows/Linux model inference, server memory and an embedded/native product runtime remain unmeasured.

Total verified model acquisition is 2,770,266,421 bytes (2.58 GiB), including the Q8 artifact/tokenizers and separately pinned GTE custom code. The standalone release evaluation executable is 1,527,248 bytes; this is not an embedded product-runtime size. Core retrieval/guard cold/warm timings and owned RSS are in `core-resources.json`.

Hosted requests: **0**. Hosted billed spend for this evaluation: **US$0.00**; a hosted inference workload cost/latency comparison is unmeasured. Local inference has no token bill; electricity/hardware costs are excluded. The fixed quality workload is the frozen held-out split: 1,620 decision cases plus 48 ranking cases where each runner supports them. Latency-panel repeats are separate. No vendor latency, price estimate or mocked response is labelled measured inference.

## Per-task decision and remaining evidence

| Task | English | Dutch | Mixed |
| --- | --- | --- | --- |
| Ranking | Retain current; defer models | Retain current; defer models | Retain current; defer models |
| Answerability / claim support | Defer | Defer | Defer |
| Actions / decisions / open questions | Retain summaries/manual review | Retain summaries/manual review | Retain summaries/manual review |
| Templates | Retain manual choice | Retain manual choice | Retain manual choice |
| Speaker names | Retain explicit naming | Retain explicit naming | Retain explicit naming |
| Cleanup verification | Retain raw fallback; defer model | Retain raw fallback; defer model | Retain raw fallback; defer model |

- OpenRouter/direct inference needs explicitly configured credentials and verified selected-route retention/region. The runner tests preserve typed distributions, model/provider/route identity, usage and billed cost, but do not establish live service behavior.
- The configured Ollama generative baselines were unavailable. Free-text answer/summary grounding, citation support and generated-cleanup fidelity therefore remain unmeasured. Citation syntax checks are not a substitute.
- Q8 graph loading was verified, but its reference-parity comparison would exceed the remaining download ceiling. Laya and official local Mac Decider inference are unavailable/unmeasured.
- No decision runtime is embedded into Pennel. Native packaging/linkage, Windows/Linux inference, physical devices, pilot usefulness and deployed hosted policy remain separate gates.
- Keep #53 open. #43 owns vector/hybrid candidate recall; #54 owns production provider contracts. No production feature or default model is changed.
