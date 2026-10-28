# OpenRouter follow-up — 28 October 2026

The supplied evaluation credential was stored in a dedicated macOS Keychain entry and injected only into benchmark subprocesses. No secret value is included in Git, model inputs, reports or application settings. The clipboard was preserved.

**Actual spend: US$0.313064388 across 30,162 requests**, within the shared US$2 ceiling. All charges are reconciled. This includes development probes, full separate calibration/held-out workloads, warmups, timing repeats and preserved service failures. No completed paid work was replayed during operational recovery.

## Result and scope

Hosted models provide useful **frozen-calibrated, bounded-input research candidates**. Standard Decider meets ranking-quality gates in all three languages; Decider, Flash and Mercury meet cleanup point gates in all three. This does not activate a product model. Sequential per-passage HTTP ranking takes seconds, name/owner calibration evidence is incomplete, and the template coverage target is mathematically infeasible in this corpus.

| Task | Recommendation for EN / NL / mixed |
| --- | --- |
| Passage ranking | Advance standard Decider to a bounded prototype; retain current production retrieval while HTTP fanout latency is redesigned and revalidated. |
| Answerability / claim support | All five meet frozen point-quality gates; prototype optional verification with actual routes and fresh calibration. Production activation remains deferred. |
| Cleanup verification | Advance frozen-calibrated Flash/standard Decider as optional verifier candidates; retain raw fallback. Do not infer safety of raw model decisions. |
| Structured actions / decisions / questions | Kind detection qualifies; defer complete workflow because owner calibration lacks diverse targets. |
| Templates | Defer selection judgment pending a feasible separately preregistered corpus; failure of this coverage gate does not establish poor model classification. |
| Speaker names | Retain explicit naming; positive suggestion scores do not establish robust ambiguity discrimination. |

The original [local comparison](../2026-10-28/README.md) remains unchanged. Product contracts/features remain owned by #54 and the gated follow-ups; #43 owns vector/hybrid candidate-recall work.

## Reproduce and identify the evidence

```sh
python3 evaluations/decision-models/run.py --offline \
  --replay evaluations/decision-models/results/openrouter-2026-10-28
```

This replays measured API predictions and regenerates quality/calibration/ranking reports without credentials, downloads or inference. `report.json.gz` contains the full derived report, `predictions.jsonl.gz` and `calibration-predictions.jsonl.gz` the actual typed results, and `latency-receipts.jsonl.gz` the timed calls. Billing, policy, scope and continuation records are separate. No mocked unit-test reply is presented as measured inference.

Generation source: [`d3c18bc`](https://github.com/pennel-app/pennel/commit/d3c18bcd67e15617552d0e4f783139514940a4b8). Frozen manifest SHA-256: `20f0a59b3caef84e33e12cf2a6fead8617ed80ca44e20f7e685d897a6b16a4ad`. Original corpus/task criteria, ordered labels and one-case question grouping were retained. Quality concurrency was four; timing concurrency was one. Calibration uses different meeting groups and was fitted before held-out testing.

All five routes use the actual Decisions endpoint, typed distributions, fixed requested/canonical models and upstream tags, `allow_fallbacks:false`, `data_collection:deny` and provider `zdr:true`. Processing is explicitly **global**; there is no EU/US residency claim. Account logging/opt-in settings were not inspected. Public route policy evidence is in [the snapshot](../../evidence/openrouter-2026-10-28.json). Opaque server weights, quantization, tokenizer/hardware and internal service revisions remain unmeasured beyond returned identifiers/metadata.

## Quality workload and transport outcomes

Each model/split has 1,620 decision cases and 48 ranking workloads. Ranking expands to 810 passage requests; long inputs are rejected locally rather than silently truncated. Held-out totals: **8,340 planned cases = 7,856 successful cases + 480 bounded-input abstentions + three Jev distribution-validation errors + one Mercury gateway error**. Input refusals remain in task coverage denominators.

Jev Choice documentation describes approximately normalized probabilities; the frozen validator rejects sums outside its declared tolerance. The rejected vectors were not retained, so no rounding explanation or broad provider-defect claim is inferred. Validation was not relaxed after results were inspected.

One calibration Cloudflare429 and one held-out Mercury502 halted later paid requests until current-key usage reconciled with known per-request charges. Only never-sent placeholders were continued, with the original failed calls retained. Two additional service failures occurred in timing: Flash timeout and Mercury502. All four missing inline costs were reconciled to zero from quiescent key-usage observations; original error responses/unknown inline usage remain preserved.

## Ranking quality

Equal-meeting comparisons follow the frozen protocol: 10 meeting groups per language, 15 answerable ranking queries and one unanswerable query, nDCG@5 gain ≥0.03, strictly positive 95% paired-bootstrap lower bound, and no recall@5 regression. Descriptive question-mean and meeting-weighted results differ; do not substitute a favorable question mean for the preregistered comparison.

| Model | Language | Question-mean nDCG@5 before / after | Meeting-weighted gain [95% CI] | Question-mean recall@5 before / after | Quality gate |
| --- | --- | --- | --- | --- | --- |
| openrouter-clef-flash | en | 0.589 / 0.687 | 0.055 [0.000, 0.148] | 0.933 / 0.933 | Fail |
| openrouter-clef-flash | mixed | 0.571 / 0.680 | 0.027 [0.000, 0.082] | 0.933 / 1.000 | Fail |
| openrouter-clef-flash | nl | 0.572 / 0.907 | 0.330 [0.256, 0.369] | 0.933 / 0.933 | Pass |
| openrouter-decider | en | 0.589 / 0.909 | 0.326 [0.252, 0.369] | 0.933 / 0.933 | Pass |
| openrouter-decider | mixed | 0.571 / 0.951 | 0.310 [0.199, 0.399] | 0.933 / 1.000 | Pass |
| openrouter-decider | nl | 0.572 / 0.908 | 0.361 [0.344, 0.369] | 0.933 / 0.933 | Pass |
| openrouter-decider-flash | en | 0.589 / 0.761 | 0.105 [0.000, 0.209] | 0.933 / 0.933 | Fail |
| openrouter-decider-flash | mixed | 0.571 / 0.975 | 0.347 [0.258, 0.414] | 0.933 / 1.000 | Pass |
| openrouter-decider-flash | nl | 0.572 / 0.932 | 0.367 [0.362, 0.369] | 0.933 / 0.933 | Pass |
| openrouter-jev | en | 0.589 / 0.663 | 0.018 [0.000, 0.055] | 0.933 / 0.933 | Fail |
| openrouter-jev | mixed | 0.571 / 0.705 | 0.033 [0.000, 0.100] | 0.933 / 1.000 | Fail |
| openrouter-jev | nl | 0.572 / 0.664 | 0.054 [0.000, 0.127] | 0.933 / 0.933 | Fail |
| openrouter-mercury | en | 0.589 / 0.810 | 0.209 [0.098, 0.308] | 0.933 / 0.933 | Pass |
| openrouter-mercury | mixed | 0.571 / 0.705 | 0.033 [0.000, 0.100] | 0.933 / 1.000 | Fail |
| openrouter-mercury | nl | 0.572 / 0.882 | 0.293 [0.185, 0.369] | 0.933 / 0.933 | Pass |

Candidate recall stays fixed at 0.933 for English/Dutch and 1.0 for mixed language; reranking cannot recover a source omitted by retrieval. Standard Decider is the only candidate meeting these ranking-quality gates in all languages. Its sequential network workload still requires a latency design decision.

## Answerability and claim support

All five meet the frozen point precision/coverage target on bounded synthetic inputs. Accuracy below is accepted-label accuracy; unsupported/unfitted cases remain uncovered. Full per-class precision/recall/F1, Brier/ECE bins and selective curves for all tasks are in the compressed report.

| Model | Language | Task | Raw macro-F1 | Calibrated selective accuracy / coverage | Raw Brier / ECE | Calibrated Brier / ECE |
| --- | --- | --- | ---: | --- | --- | --- |
| openrouter-clef-flash | en | answerability | 0.961 | 100.0% / 93.3% | 0.004 / 0.043 | 0.000 / 0.005 |
| openrouter-clef-flash | en | claim_support | 0.947 | 100.0% / 86.7% | 0.001 / 0.023 | 0.000 / 0.003 |
| openrouter-clef-flash | mixed | answerability | 0.961 | 100.0% / 90.0% | 0.003 / 0.036 | 0.000 / 0.004 |
| openrouter-clef-flash | mixed | claim_support | 0.947 | 100.0% / 86.7% | 0.001 / 0.023 | 0.000 / 0.003 |
| openrouter-clef-flash | nl | answerability | 0.961 | 100.0% / 93.3% | 0.004 / 0.044 | 0.000 / 0.005 |
| openrouter-clef-flash | nl | claim_support | 0.947 | 100.0% / 83.3% | 0.001 / 0.025 | 0.000 / 0.004 |
| openrouter-decider | en | answerability | 0.961 | 100.0% / 93.3% | 0.000 / 0.003 | 0.000 / 0.001 |
| openrouter-decider | en | claim_support | 0.947 | 100.0% / 90.0% | 0.000 / 0.000 | 0.000 / 0.000 |
| openrouter-decider | mixed | answerability | 0.961 | 100.0% / 86.7% | 0.000 / 0.004 | 0.000 / 0.001 |
| openrouter-decider | mixed | claim_support | 0.947 | 100.0% / 83.3% | 0.000 / 0.001 | 0.000 / 0.000 |
| openrouter-decider | nl | answerability | 0.961 | 100.0% / 93.3% | 0.000 / 0.002 | 0.000 / 0.001 |
| openrouter-decider | nl | claim_support | 0.947 | 100.0% / 90.0% | 0.000 / 0.000 | 0.000 / 0.000 |
| openrouter-decider-flash | en | answerability | 0.961 | 100.0% / 93.3% | 0.000 / 0.007 | 0.000 / 0.001 |
| openrouter-decider-flash | en | claim_support | 0.947 | 100.0% / 86.7% | 0.000 / 0.002 | 0.000 / 0.001 |
| openrouter-decider-flash | mixed | answerability | 0.961 | 100.0% / 93.3% | 0.000 / 0.009 | 0.000 / 0.002 |
| openrouter-decider-flash | mixed | claim_support | 0.947 | 100.0% / 86.7% | 0.000 / 0.005 | 0.000 / 0.001 |
| openrouter-decider-flash | nl | answerability | 0.961 | 100.0% / 93.3% | 0.000 / 0.010 | 0.000 / 0.002 |
| openrouter-decider-flash | nl | claim_support | 0.947 | 100.0% / 90.0% | 0.000 / 0.007 | 0.000 / 0.001 |
| openrouter-jev | en | answerability | 0.961 | 100.0% / 90.0% | 0.003 / 0.036 | 0.000 / 0.004 |
| openrouter-jev | en | claim_support | 0.947 | 100.0% / 90.0% | 0.001 / 0.022 | 0.000 / 0.003 |
| openrouter-jev | mixed | answerability | 0.961 | 100.0% / 88.3% | 0.003 / 0.037 | 0.000 / 0.004 |
| openrouter-jev | mixed | claim_support | 0.947 | 100.0% / 90.0% | 0.001 / 0.021 | 0.000 / 0.003 |
| openrouter-jev | nl | answerability | 0.961 | 100.0% / 91.7% | 0.003 / 0.033 | 0.000 / 0.004 |
| openrouter-jev | nl | claim_support | 0.947 | 100.0% / 90.0% | 0.001 / 0.021 | 0.000 / 0.003 |
| openrouter-mercury | en | answerability | 0.961 | 100.0% / 93.3% | 0.000 / 0.001 | 0.000 / 0.000 |
| openrouter-mercury | en | claim_support | 0.947 | 100.0% / 86.7% | 0.000 / 0.000 | 0.000 / 0.000 |
| openrouter-mercury | mixed | answerability | 0.947 | 100.0% / 88.3% | 0.000 / 0.002 | 0.000 / 0.001 |
| openrouter-mercury | mixed | claim_support | 0.947 | 100.0% / 78.3% | 0.000 / 0.000 | 0.000 / 0.000 |
| openrouter-mercury | nl | answerability | 0.961 | 100.0% / 93.3% | 0.000 / 0.001 | 0.000 / 0.000 |
| openrouter-mercury | nl | claim_support | 0.947 | 100.0% / 86.7% | 0.000 / 0.000 | 0.000 / 0.000 |

Brier uses the frozen unnormalised categorical definition (range 0–2); binary Brier is reported separately. Provider confidence is retained as metadata and is not substituted for a class distribution. These point gates do not establish calibration under natural-meeting distribution shift.

## Cleanup: permit valid text without permitting corruption

Each model/language has 60 valid and 120 critical-unsafe proposal cases, including label-order variants. A confidently accepted FALSE/reject decision is not permission to use a valid cleanup.

| Model | Language | Raw valid allowed /60 | Raw unsafe allowed /120 | Calibrated valid allowed /60 | Calibrated unsafe allowed /120 | Point gate |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| openrouter-clef-flash | en | 60 | 24 | 60 | 0 | Pass |
| openrouter-clef-flash | mixed | 40 | 32 | 0 | 0 | Fail |
| openrouter-clef-flash | nl | 60 | 22 | 40 | 0 | Fail |
| openrouter-decider | en | 60 | 20 | 60 | 0 | Pass |
| openrouter-decider | mixed | 60 | 0 | 60 | 0 | Pass |
| openrouter-decider | nl | 60 | 20 | 60 | 0 | Pass |
| openrouter-decider-flash | en | 60 | 20 | 60 | 0 | Pass |
| openrouter-decider-flash | mixed | 60 | 20 | 60 | 0 | Pass |
| openrouter-decider-flash | nl | 60 | 0 | 60 | 0 | Pass |
| openrouter-jev | en | 60 | 0 | 60 | 0 | Pass |
| openrouter-jev | mixed | 40 | 0 | 40 | 0 | Fail |
| openrouter-jev | nl | 60 | 0 | 59 | 0 | Pass |
| openrouter-mercury | en | 60 | 0 | 59 | 0 | Pass |
| openrouter-mercury | mixed | 48 | 0 | 60 | 0 | Pass |
| openrouter-mercury | nl | 60 | 0 | 60 | 0 | Pass |

Decider, Flash and Mercury pass the frozen-calibrated point gate in all languages. Several raw configurations permit 20/120 critical corruptions, so these results do not authorize an uncalibrated model or a transferred threshold. Retain raw-text fallback. Real provider-generated cleanup and ordinary user dictation remain separate acceptance evidence.

## Corpus and calibration feasibility limits

- **Templates:** per language 40 cases = 20 positive + 20 abstain. Two positive long cases exceed the configured bounds, leaving at most18 correct suggestions:45% coverage. Fifty-percent coverage requires 20 suggestions and95% precision requires at least19 correct—mathematically impossible with18 eligible positives. Keep the frozen result, repair feasibility in a separately preregistered v2, and do not label this a classification-quality failure.
- **Owners:** per language 20 cases, two bounded-input refusals; all18 eligible targets are speaker_1. Each ordered-label calibration context has nine examples and one observed class, so no valid calibration fit. Kind detection qualifies but does not establish owner attribution or complete structured extraction.
- **Names:**60 cases = 40 positive + 20 unknown, with four positive/two unknown bound refusals. The positive-coverage ceiling is60%, so the 50% gate is feasible. However24 unmatched cases include six bound refusals and18 ambiguous cases; those calibration contexts each contain nine single-class unknown targets. Zero forced names after automatic abstention does not prove ambiguity discrimination.
- Authored procedural grammar, repeated semantic skeletons and one frozen zero-shot prompt family limit generalization. Meeting grouping prevents adjacent-passage leakage; label variants do not increase the number of independent meetings.

## Measured serial end-to-end latency

The panel was admitted solely from reconciled remaining budget and full conservative cost bounds, never from quality scores. Five fresh-client requests per language and 30 warm attempts per task/language were planned after excluded warmups. Fresh-client metadata setup is separate and is not server/model-cold measurement. Remote model load time, memory and native/download sizes are unmeasured.

Jev, standard Decider and Cloudflare completed their panels. Flash has29 successful NL-template warm samples plus one preserved 60.14-second timeout. Mercury completed cold-client samples and part of English warm timing before a second gateway error; remaining cells are unmeasured. Error counts accompany successful-sample percentiles.

| Model | Language | Task | Successful warm n | p50 / p95 ms | Error samples |
| --- | --- | --- | ---: | --- | ---: |
| openrouter-jev | en | answerability | 30 | 309.118 / 402.278 | 0 |
| openrouter-jev | en | claim_support | 30 | 318.587 / 513.731 | 0 |
| openrouter-jev | en | cleanup | 30 | 342.255 / 475.476 | 0 |
| openrouter-jev | en | events | 30 | 507.772 / 634.717 | 0 |
| openrouter-jev | en | ranking | 30 | 4439.621 / 7699.825 | 0 |
| openrouter-jev | en | speaker_names | 30 | 326.086 / 593.605 | 0 |
| openrouter-jev | en | templates | 30 | 308.537 / 382.949 | 0 |
| openrouter-jev | mixed | answerability | 30 | 293.141 / 436.598 | 0 |
| openrouter-jev | mixed | claim_support | 30 | 292.594 / 366.526 | 0 |
| openrouter-jev | mixed | cleanup | 30 | 292.962 / 384.398 | 0 |
| openrouter-jev | mixed | events | 30 | 310.986 / 400.674 | 0 |
| openrouter-jev | mixed | ranking | 30 | 4556.338 / 4904.928 | 0 |
| openrouter-jev | mixed | speaker_names | 30 | 340.694 / 414.919 | 0 |
| openrouter-jev | mixed | templates | 30 | 339.348 / 419.070 | 0 |
| openrouter-jev | nl | answerability | 30 | 292.250 / 391.072 | 0 |
| openrouter-jev | nl | claim_support | 30 | 312.099 / 411.678 | 0 |
| openrouter-jev | nl | cleanup | 30 | 305.166 / 379.495 | 0 |
| openrouter-jev | nl | events | 30 | 300.765 / 412.269 | 0 |
| openrouter-jev | nl | ranking | 30 | 4416.113 / 6054.282 | 0 |
| openrouter-jev | nl | speaker_names | 30 | 296.660 / 396.013 | 0 |
| openrouter-jev | nl | templates | 30 | 291.904 / 349.771 | 0 |
| openrouter-decider | en | answerability | 30 | 355.290 / 435.943 | 0 |
| openrouter-decider | en | claim_support | 30 | 399.711 / 453.709 | 0 |
| openrouter-decider | en | cleanup | 30 | 413.950 / 568.311 | 0 |
| openrouter-decider | en | events | 30 | 408.755 / 595.251 | 0 |
| openrouter-decider | en | ranking | 30 | 5405.675 / 8428.161 | 0 |
| openrouter-decider | en | speaker_names | 30 | 382.249 / 717.205 | 0 |
| openrouter-decider | en | templates | 30 | 372.770 / 566.322 | 0 |
| openrouter-decider | mixed | answerability | 30 | 380.240 / 573.215 | 0 |
| openrouter-decider | mixed | claim_support | 30 | 399.856 / 487.920 | 0 |
| openrouter-decider | mixed | cleanup | 30 | 390.477 / 515.991 | 0 |
| openrouter-decider | mixed | events | 30 | 409.322 / 502.451 | 0 |
| openrouter-decider | mixed | ranking | 30 | 5664.495 / 7285.054 | 0 |
| openrouter-decider | mixed | speaker_names | 30 | 397.527 / 1039.055 | 0 |
| openrouter-decider | mixed | templates | 30 | 401.565 / 513.113 | 0 |
| openrouter-decider | nl | answerability | 30 | 391.832 / 806.767 | 0 |
| openrouter-decider | nl | claim_support | 30 | 427.133 / 2295.050 | 0 |
| openrouter-decider | nl | cleanup | 30 | 409.326 / 1252.209 | 0 |
| openrouter-decider | nl | events | 30 | 386.375 / 448.157 | 0 |
| openrouter-decider | nl | ranking | 30 | 5626.699 / 8810.198 | 0 |
| openrouter-decider | nl | speaker_names | 30 | 406.145 / 713.911 | 0 |
| openrouter-decider | nl | templates | 30 | 365.291 / 545.538 | 0 |
| openrouter-decider-flash | en | answerability | 30 | 273.099 / 366.527 | 0 |
| openrouter-decider-flash | en | claim_support | 30 | 289.142 / 446.078 | 0 |
| openrouter-decider-flash | en | cleanup | 30 | 265.548 / 368.814 | 0 |
| openrouter-decider-flash | en | events | 30 | 292.959 / 392.537 | 0 |
| openrouter-decider-flash | en | ranking | 30 | 4391.668 / 4744.642 | 0 |
| openrouter-decider-flash | en | speaker_names | 30 | 277.078 / 398.712 | 0 |
| openrouter-decider-flash | en | templates | 30 | 272.584 / 333.062 | 0 |
| openrouter-decider-flash | mixed | answerability | 30 | 283.688 / 391.566 | 0 |
| openrouter-decider-flash | mixed | claim_support | 30 | 286.582 / 375.609 | 0 |
| openrouter-decider-flash | mixed | cleanup | 30 | 274.032 / 354.309 | 0 |
| openrouter-decider-flash | mixed | events | 30 | 279.496 / 407.865 | 0 |
| openrouter-decider-flash | mixed | ranking | 30 | 4346.057 / 4698.413 | 0 |
| openrouter-decider-flash | mixed | speaker_names | 30 | 290.606 / 551.127 | 0 |
| openrouter-decider-flash | mixed | templates | 30 | 269.221 / 382.864 | 0 |
| openrouter-decider-flash | nl | answerability | 30 | 276.226 / 385.880 | 0 |
| openrouter-decider-flash | nl | claim_support | 30 | 291.283 / 398.065 | 0 |
| openrouter-decider-flash | nl | cleanup | 30 | 270.294 / 347.796 | 0 |
| openrouter-decider-flash | nl | events | 30 | 304.455 / 404.789 | 0 |
| openrouter-decider-flash | nl | ranking | 30 | 4470.135 / 5206.463 | 0 |
| openrouter-decider-flash | nl | speaker_names | 30 | 304.448 / 420.048 | 0 |
| openrouter-decider-flash | nl | templates | 29 | 291.497 / 416.327 | 1 |
| openrouter-clef-flash | en | answerability | 30 | 453.514 / 631.109 | 0 |
| openrouter-clef-flash | en | claim_support | 30 | 510.038 / 748.376 | 0 |
| openrouter-clef-flash | en | cleanup | 30 | 506.971 / 648.847 | 0 |
| openrouter-clef-flash | en | events | 30 | 533.459 / 1006.549 | 0 |
| openrouter-clef-flash | en | ranking | 30 | 6759.101 / 7835.997 | 0 |
| openrouter-clef-flash | en | speaker_names | 30 | 552.775 / 969.131 | 0 |
| openrouter-clef-flash | en | templates | 30 | 465.053 / 670.270 | 0 |
| openrouter-clef-flash | mixed | answerability | 30 | 455.589 / 703.430 | 0 |
| openrouter-clef-flash | mixed | claim_support | 30 | 520.546 / 703.709 | 0 |
| openrouter-clef-flash | mixed | cleanup | 30 | 489.901 / 720.434 | 0 |
| openrouter-clef-flash | mixed | events | 30 | 503.306 / 645.715 | 0 |
| openrouter-clef-flash | mixed | ranking | 30 | 7054.914 / 10318.107 | 0 |
| openrouter-clef-flash | mixed | speaker_names | 30 | 539.032 / 697.470 | 0 |
| openrouter-clef-flash | mixed | templates | 30 | 446.638 / 635.739 | 0 |
| openrouter-clef-flash | nl | answerability | 30 | 453.812 / 611.239 | 0 |
| openrouter-clef-flash | nl | claim_support | 30 | 541.642 / 788.397 | 0 |
| openrouter-clef-flash | nl | cleanup | 30 | 453.434 / 721.320 | 0 |
| openrouter-clef-flash | nl | events | 30 | 540.562 / 800.176 | 0 |
| openrouter-clef-flash | nl | ranking | 30 | 7106.950 / 8687.676 | 0 |
| openrouter-clef-flash | nl | speaker_names | 30 | 548.778 / 763.048 | 0 |
| openrouter-clef-flash | nl | templates | 30 | 552.158 / 785.481 | 0 |
| openrouter-mercury | en | answerability | 30 | 419.444 / 736.881 | 0 |
| openrouter-mercury | en | claim_support | 7 | 471.695 / 721.643 | 1 |
| openrouter-mercury | en | cleanup | 0 | — / — | 0 |
| openrouter-mercury | en | events | 0 | — / — | 0 |
| openrouter-mercury | en | ranking | 0 | — / — | 0 |
| openrouter-mercury | en | speaker_names | 0 | — / — | 0 |
| openrouter-mercury | en | templates | 0 | — / — | 0 |
| openrouter-mercury | mixed | answerability | 0 | — / — | 0 |
| openrouter-mercury | mixed | claim_support | 0 | — / — | 0 |
| openrouter-mercury | mixed | cleanup | 0 | — / — | 0 |
| openrouter-mercury | mixed | events | 0 | — / — | 0 |
| openrouter-mercury | mixed | ranking | 0 | — / — | 0 |
| openrouter-mercury | mixed | speaker_names | 0 | — / — | 0 |
| openrouter-mercury | mixed | templates | 0 | — / — | 0 |
| openrouter-mercury | nl | answerability | 0 | — / — | 0 |
| openrouter-mercury | nl | claim_support | 0 | — / — | 0 |
| openrouter-mercury | nl | cleanup | 0 | — / — | 0 |
| openrouter-mercury | nl | events | 0 | — / — | 0 |
| openrouter-mercury | nl | ranking | 0 | — / — | 0 |
| openrouter-mercury | nl | speaker_names | 0 | — / — | 0 |
| openrouter-mercury | nl | templates | 0 | — / — | 0 |

Whole ranking wall times include evaluation audit/accounting I/O. The serial HTTP sums below isolate the actual API round trips; neither is provider-only compute time. A production batching/parallel-rerank design needs new grouping/calibration and product acceptance rather than assuming this sequential implementation is interactive.

| Model | Language | Ranking wall p50 / p95 ms | Sum of serial HTTP p50 / p95 ms | n |
| --- | --- | --- | --- | ---: |
| openrouter-jev | en | 4439.621 / 7699.825 | 3812.256 / 7045.954 | 30 |
| openrouter-jev | mixed | 4556.338 / 4904.928 | 3834.981 / 4174.794 | 30 |
| openrouter-jev | nl | 4416.113 / 6054.282 | 3752.372 / 5351.623 | 30 |
| openrouter-decider | en | 5405.675 / 8428.161 | 4740.530 / 7611.228 | 30 |
| openrouter-decider | mixed | 5664.495 / 7285.054 | 4840.530 / 6458.980 | 30 |
| openrouter-decider | nl | 5626.699 / 8810.198 | 4905.689 / 8041.713 | 30 |
| openrouter-decider-flash | en | 4391.668 / 4744.642 | 3553.953 / 3921.337 | 30 |
| openrouter-decider-flash | mixed | 4346.057 / 4698.413 | 3516.266 / 3848.476 | 30 |
| openrouter-decider-flash | nl | 4470.135 / 5206.463 | 3599.765 / 4299.243 | 30 |
| openrouter-clef-flash | en | 6759.101 / 7835.997 | 5964.133 / 7035.092 | 30 |
| openrouter-clef-flash | mixed | 7054.914 / 10318.107 | 6190.100 / 9502.765 | 30 |
| openrouter-clef-flash | nl | 7106.950 / 8687.676 | 6187.941 / 7810.125 | 30 |
| openrouter-mercury | en | — / — | — / — | 0 |
| openrouter-mercury | mixed | — / — | — / — | 0 |
| openrouter-mercury | nl | — / — | — / — | 0 |

Per-case cold-client/setup samples, request IDs, actual providers, routing/privacy configuration, rate snapshots and sample counts remain in resources/receipts. Hosted figures are separate from the original direct/local measurements.

## Tokens and actual cost by defined workload

| Stage | Requests | Billed US$ |
| --- | ---: | ---: |
| Binary/categorical development smoke |10|0.000060710|
| Calibration |11670|0.124473208|
| Held-out quality |11670|0.124717286|
| Timing, warmups and continuations |6812|0.063813184|
| **Total** |**30162**|**0.313064388**|

Per-language table covers the frozen held-out workload, including every repeated-state/passage call and declared label-order variant. Tokens are provider-reported inputs/outputs; cache and billing detail is retained as returned. No counts are invented for errors with no usage. One Mercury502 inline cost is unknown but separately reconciled to zero; this does not fabricate its token counts.

| Model | Language | Requests | Input tokens | Output tokens | Known/reconciled billed US$ |
| --- | --- | ---: | ---: | ---: | ---: |
| openrouter-clef-flash | en | 778 | 278177 | 0 | 0.010570726 |
| openrouter-clef-flash | mixed | 778 | 282583 | 0 | 0.010738154 |
| openrouter-clef-flash | nl | 778 | 323602 | 0 | 0.012296876 |
| openrouter-decider | en | 778 | 238167 | 778 | 0.00476334 |
| openrouter-decider | mixed | 778 | 242605 | 778 | 0.00485210 |
| openrouter-decider | nl | 778 | 283114 | 778 | 0.00566228 |
| openrouter-decider-flash | en | 778 | 238167 | 778 | 0.00238167 |
| openrouter-decider-flash | mixed | 778 | 242605 | 778 | 0.00242605 |
| openrouter-decider-flash | nl | 778 | 283114 | 778 | 0.00283114 |
| openrouter-jev | en | 778 | 405944 | 21934 | 0.017049648 |
| openrouter-jev | mixed | 778 | 412181 | 21938 | 0.017311602 |
| openrouter-jev | nl | 778 | 464410 | 21936 | 0.019505220 |
| openrouter-mercury | en | 778 | 222683 | 778 | 0.00445366 |
| openrouter-mercury | mixed | 778 | 226844 | 777 | 0.00453688 |
| openrouter-mercury | nl | 778 | 266897 | 778 | 0.00533794 |

The ledger preserves every attempt, including all warmups and the four reconciled missing-cost failures. Neither a catalog price calculation nor a mocked response is described as an actual bill. The budget was not reset between stages.

## Remaining work

- No production provider/default, recording rule, speaker enrollment or external action is enabled by this report. Preserve explicit user choice and raw/manual fallbacks.
- Generative answer/citation grounding, summary fidelity and provider-generated cleanup remain unmeasured in the configured local baseline; a classifier score is not generated-answer evidence.
- Template feasibility and diverse owner/ambiguity calibration require a new preregistered corpus; do not retroactively change this experiment.
- Mercury serial latency is partial due service failure; future availability/reliability evidence remains necessary.
- Hosted opaque runtime/hardware and account/region policy, packaged runtime/linkage, physical devices and field/pilot/deployment acceptance remain separate gates. Keep #53 open.
