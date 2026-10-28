# Decision-model follow-up: grounding and calibrated suggestions

Actual cumulative hosted inference: **35,555 attempts / US$0.532481940**, under the shared US$2 ceiling. No new model downloads or production defaults. Original local and hosted records remain unchanged.

This follow-up completes the original Mercury timing panel, measures current-core generated answers/cleanup/summaries, and tests a separately preregistered corpus that repairs the suggestion-coverage and owner/name calibration gaps. A benchmark point-gate pass is a research result, not production or independent human acceptance.

## Findings and decisions

| Task | Evidence | Recommendation |
| --- | --- | --- |
| Ranking | Original standard Decider passes all three languages; Mercury timing is now complete, including errors. Serial per-passage API work still takes seconds. | Retain current retrieval; defer integration and share the candidate/latency evidence with #43. |
| Answerability / support | Original bounded typed classifiers pass point gates. Actual generated Q&A adds unsupported Wi-Fi/access-code equivalence and gender attributes; all three languages miss the 98% grounding requirement. | Retain explicit sources/user review; defer automated verifier rollout until generated-claim transfer and product contracts are validated. |
| Actions / decisions / questions | Diverse-owner component quality varies by model/language. Current-core summary checkboxes promote tentative or unassigned work into actions. | Defer automatic structured extraction; use the qualified components for bounded prototypes, not independent claim of end-to-end reliability. |
| Templates | Calibrated passes: EN Jev; NL Decider; mixed Jev/Decider/Flash. No provider passes all languages. | Retain manual selection; defer rollout. Do not replace frozen calibrated gates with favorable raw scores. |
| Speaker names | All five pass EN/mixed point gates; only Clef passes NL. Raw models sometimes force ambiguous names; calibration abstains. | Retain explicit names; defer bounded candidate-list suggestions pending broader calibration and product acceptance. |
| Cleanup | Current-core EN/NL generated examples preserve fidelity; mixed examples translate the English date phrase. The existing guard accepts both language failures. Original calibrated Decider/Flash/Mercury verifier results remain separate. | Retain raw fallback and explicit user control; defer model changes until end-to-end language-preservation validation. |

## Protocol and provenance

- Complete implementation source: `8e693d9`; source-bound experiment freezes: `8c525a3`. All prompts, labels/order, grouping, model routes, privacy settings, budgets and criteria were recorded before new held-out inference.
- Original results: [local comparison](../2026-10-28/README.md), [initial hosted comparison](../openrouter-2026-10-28/README.md). No old fixture, cutoff or result was replaced.
- Supplement: 36 meeting groups / 108 translated meeting documents, 12 groups per split. Each task/language/order has 12 calibration observations before failures; 90 fitted contexts remain, minimum 11. Template positive prevalence is 2/3; name positive prevalence 3/4. All states are bounded below 2,000 bytes. Distinct split-specific statements, topics, objects, quantities and deadlines prevent verbatim split reuse.
- Quality: 1,080 calibration cases (1,079 successful, one preserved Mercury gateway error) and 1,080 held-out cases (1,079 successful, one preserved Clef rate-limit error). Only previously unsent cases were continued. The final held-out continuation uses one active request under the original at-most-four bound; wire inputs, grouping, route and calibration remain fixed. Quality-workload timings are not the serial-panel timings.
- Generation: 42 Q&A, six cleanups and six summaries on two original held-out meetings per language. Every substantive claim, cited evidence span, raw cleanup and six marked event targets per summary has a stored native-AI judgment. Leader review applied the same entailment rule across languages. These reviews are inspectable AI adjudications; they are not human or natural-meeting acceptance.
- GPT-4.1 mini requests the dated `openai/gpt-4.1-mini-2025-04-14` through exact `azure/swedencentral`. The returned alias is recorded separately from the catalog canonical identity; neither proves remote weight contents. Global processing, provider ZDR, denied data collection and disabled fallbacks are explicit. No regional-residency or account-logging claim is made.

## Per-language generated-output measurements

| Language | QA correct / 14 | Claim support | Citation support | Selective answer precision / coverage | Summary claim support | Event macro-F1 / positive precision | Valid generated cleanup |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| en | 12/14 | 19/21 | 19/21 | 0.8571/1.0000 | 38/43 | 0.8667/0.7500 | 2/2 |
| nl | 12/14 | 16/18 | 15/18 | 0.8571/1.0000 | 26/34 | 0.5750/0.5000 | 2/2 |
| mixed | 13/14 | 23/26 | 23/26 | 0.9231/0.9286 | 25/29 | 0.9273/0.8571 | 0/2 |

All 54 outputs are adjudicated; zero judgments remain pending. The mixed cleanup preserves its proposition/date but changes `on Thursday` to `op donderdag`. It is a language-fidelity failure, not an invented numeric corruption. The guard accepts it; no valid mixed cleanup is generated, so valid-cleanup acceptance has no denominator. EN-source summaries are in Dutch, recorded as an observation rather than adding a post-hoc gate. Summary event recall is scoped to the six preregistered marked statements, not exhaustive extraction from natural meetings.

The false-action examples include turning “might book; have not committed” into a checkbox, or assigning somebody to answer an unowned question. Privacy/non-publication does not prove a blanket prohibition of private sharing; neutral `die`/`they` does not establish `hij`/`haar`. Citation syntax alone does not establish these facts.

## Template suggestions

| Language | Model | Measured / 24 | Raw F1 | Calibrated F1 | Accepted precision | Coverage | Raw/cal Brier | Raw/cal ECE | Point gate |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| en | Clef | 23/24 | 0.8667 | 0.7333 | 0.8462 | 0.5417 | 0.1325/0.1742 | 0.1791/0.0826 | fail |
| en | Decider | 24/24 | 1.0000 | 0.6000 | 1.0000 | 0.5000 | 0.0446/0.0008 | 0.1198/0.0094 | fail |
| en | Flash | 24/24 | 0.8993 | 0.8805 | 1.0000 | 0.6250 | 0.1519/0.1514 | 0.1762/0.0937 | fail |
| en | Jev | 24/24 | 1.0000 | 1.0000 | 1.0000 | 0.6667 | 0.0232/0.0000 | 0.0767/0.0000 | pass |
| en | Mercury | 24/24 | 1.0000 | 0.5333 | 1.0000 | 0.4167 | 0.0000/0.0000 | 0.0006/0.0000 | fail |
| mixed | Clef | 24/24 | 0.7333 | 0.8000 | 1.0000 | 0.5000 | 0.2069/0.3045 | 0.1692/0.1523 | fail |
| mixed | Decider | 24/24 | 1.0000 | 0.9333 | 1.0000 | 0.6667 | 0.0517/0.0113 | 0.1277/0.0339 | pass |
| mixed | Flash | 24/24 | 0.8993 | 0.9333 | 1.0000 | 0.6667 | 0.1655/0.0947 | 0.1544/0.0644 | pass |
| mixed | Jev | 24/24 | 1.0000 | 0.9714 | 1.0000 | 0.6250 | 0.0150/0.0007 | 0.0608/0.0078 | pass |
| mixed | Mercury | 24/24 | 1.0000 | 0.7333 | 1.0000 | 0.5000 | 0.0001/0.0000 | 0.0039/0.0000 | fail |
| nl | Clef | 24/24 | 0.8381 | 0.7333 | 1.0000 | 0.5000 | 0.1827/0.1677 | 0.1327/0.1000 | fail |
| nl | Decider | 24/24 | 1.0000 | 0.9091 | 1.0000 | 0.6667 | 0.0533/0.0122 | 0.0992/0.0295 | pass |
| nl | Flash | 24/24 | 0.8993 | 0.7111 | 1.0000 | 0.5000 | 0.2011/0.1165 | 0.2025/0.0841 | fail |
| nl | Jev | 24/24 | 1.0000 | 0.6800 | 1.0000 | 0.5000 | 0.0758/0.0511 | 0.1483/0.1015 | fail |
| nl | Mercury | 24/24 | 1.0000 | 0.5048 | 1.0000 | 0.3750 | 0.0000/0.0000 | 0.0017/0.0000 | fail |

## Candidate-list speaker names

| Language | Model | Measured / 24 | Raw F1 | Calibrated F1 | Accepted precision | Coverage | Raw/cal Brier | Raw/cal ECE | Point gate |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| en | Clef | 24/24 | 1.0000 | 0.6250 | 1.0000 | 0.5833 | 0.0384/0.0000 | 0.1072/0.0001 | pass |
| en | Decider | 24/24 | 1.0000 | 0.7000 | 1.0000 | 0.6667 | 0.0000/0.0000 | 0.0018/0.0001 | pass |
| en | Flash | 24/24 | 1.0000 | 0.7045 | 1.0000 | 0.6667 | 0.0012/0.0062 | 0.0144/0.0404 | pass |
| en | Jev | 24/24 | 0.6667 | 0.6439 | 1.0000 | 0.5833 | 0.2283/0.1681 | 0.1704/0.1774 | pass |
| en | Mercury | 24/24 | 1.0000 | 0.7500 | 1.0000 | 0.7500 | 0.0000/0.0000 | 0.0007/0.0001 | pass |
| mixed | Clef | 24/24 | 0.9143 | 0.7000 | 1.0000 | 0.6667 | 0.1352/0.1405 | 0.0928/0.1988 | pass |
| mixed | Decider | 24/24 | 1.0000 | 0.7500 | 1.0000 | 0.7500 | 0.0024/0.0000 | 0.0156/0.0008 | pass |
| mixed | Flash | 24/24 | 1.0000 | 0.7500 | 1.0000 | 0.7500 | 0.0009/0.0212 | 0.0149/0.1028 | pass |
| mixed | Jev | 24/24 | 0.6667 | 0.7500 | 1.0000 | 0.7500 | 0.2561/0.1515 | 0.1775/0.1429 | pass |
| mixed | Mercury | 24/24 | 1.0000 | 0.5750 | 1.0000 | 0.5000 | 0.0000/0.0030 | 0.0017/0.0219 | pass |
| nl | Clef | 24/24 | 0.8125 | 0.7000 | 1.0000 | 0.6667 | 0.1879/0.1898 | 0.1390/0.1373 | pass |
| nl | Decider | 24/24 | 1.0000 | 0.4714 | 1.0000 | 0.3750 | 0.0002/0.0000 | 0.0056/0.0013 | fail |
| nl | Flash | 24/24 | 1.0000 | 0.4917 | 1.0000 | 0.3750 | 0.0053/0.0109 | 0.0265/0.0476 | fail |
| nl | Jev | 24/24 | 0.9143 | 0.3631 | 1.0000 | 0.2500 | 0.1017/0.1513 | 0.0846/0.1175 | fail |
| nl | Mercury | 24/24 | 1.0000 | 0.5523 | 1.0000 | 0.4583 | 0.0000/0.0000 | 0.0005/0.0001 | fail |

## Final action owners

| Language | Model | Measured / 24 | Raw F1 | Calibrated F1 | Accepted precision | Coverage | Raw/cal Brier | Raw/cal ECE | Point gate |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| en | Clef | 24/24 | 0.5556 | 0.5556 | 1.0000 | 0.5000 | 0.5007/0.5451 | 0.2273/0.2444 | fail |
| en | Decider | 24/24 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.0000/0.0000 | 0.0019/0.0001 | pass |
| en | Flash | 24/24 | 1.0000 | 0.9524 | 1.0000 | 0.9167 | 0.0013/0.0000 | 0.0239/0.0002 | pass |
| en | Jev | 24/24 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.0000/0.0000 | 0.0000/0.0000 | pass |
| en | Mercury | 24/24 | 1.0000 | 0.8485 | 1.0000 | 0.7917 | 0.0000/0.0000 | 0.0012/0.0002 | fail |
| mixed | Clef | 24/24 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.0084/0.0000 | 0.0704/0.0000 | pass |
| mixed | Decider | 24/24 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.0000/0.0000 | 0.0041/0.0000 | pass |
| mixed | Flash | 24/24 | 1.0000 | 0.9778 | 1.0000 | 0.9583 | 0.0030/0.0000 | 0.0377/0.0002 | pass |
| mixed | Jev | 24/24 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.0000/0.0000 | 0.0008/0.0000 | pass |
| mixed | Mercury | 24/24 | 1.0000 | 0.9302 | 1.0000 | 0.8750 | 0.0000/0.0000 | 0.0014/0.0002 | pass |
| nl | Clef | 24/24 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.0012/0.0000 | 0.0279/0.0000 | pass |
| nl | Decider | 24/24 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.0000/0.0000 | 0.0009/0.0000 | pass |
| nl | Flash | 24/24 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.0005/0.0000 | 0.0160/0.0001 | pass |
| nl | Jev | 24/24 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.0000/0.0000 | 0.0000/0.0000 | pass |
| nl | Mercury | 24/24 | 1.0000 | 0.8413 | 1.0000 | 0.7500 | 0.0000/0.0000 | 0.0006/0.0002 | fail |

Each cell has 12 meeting groups, with authored/reversed labels kept together. Both orders are evaluated; no favorable order is selected. Brier/ECE use probability-bearing cases independently of selection, while missing outputs remain in the full quality/coverage denominator. Class and decline-label precision/recall/F1, selective curves and owner error counts are in the replayable JSON.

The preregistered calibrated macro-F1 counts threshold abstentions as false negatives. For example, standard Decider has perfect raw EN template classification, but calibrated macro-F1 is 0.6000 at 50% positive coverage. That is a frozen-policy failure, not a reclassification error; switching to the raw score after testing would change the experiment. Raw Flash template F1 is 0.8993, below the 0.90 requirement. Raw forced ambiguous names occur for Jev and Clef; calibrated forced-name count is zero in every cell. Dutch name coverage is 0.2500 Jev, 0.3750 Decider/Flash and 0.4583 Mercury; only Clef reaches the frozen coverage gate.

Raw label-order disagreement occurs only in Flash templates: EN 1/12, NL 3/12 and mixed 1/12. The Clef EN template pair containing the gateway error is incomplete and is not reported as an agreeing pair. All other evaluable pairs agree. These are small-sample point results, not calibrated reliability guarantees.

## Timing and resources

Mercury’s original panel now covers all 630 warm slots: 628 successes and two preserved failures, plus 15 fresh-client samples. There are 24 excluded warmups, including one failed warmup. Its 1,699 atomic attempts cost US$0.00754672. Summing only complete outer ranking rows would omit US$0.00002066 from successful requests inside failed ranking invocations. API-only durations and ranking wall/accounting time remain separate; failed ranking wall time was not measured.

The generative panel completed all 294 requests, including 15 fresh-client samples and 30 warm samples per task/language after excluded warmups. It cost US$0.14832004; the separate 54-case quality workload cost US$0.02497616. The owned quality+timing command took 660.28 seconds with OS maximum RSS 446,185,472 bytes. This is client-command memory, not remote-model memory.

| Language | Generative task | Cold-client n / p50 / p95 ms | Warm n / p50 / p95 ms |
| --- | --- | ---: | ---: |
| en | cleanup | 0/—/— | 30/841.0886/1360.7633 |
| en | meeting_qa | 5/781.7199/1052.3534 | 30/834.5001/1262.1257 |
| en | summary | 0/—/— | 30/3954.2156/4384.2622 |
| mixed | cleanup | 0/—/— | 30/764.2774/1025.5325 |
| mixed | meeting_qa | 5/886.0344/1141.9668 | 30/893.7914/1337.7837 |
| mixed | summary | 0/—/— | 30/3306.9487/3725.8130 |
| nl | cleanup | 0/—/— | 30/813.6361/1130.1917 |
| nl | meeting_qa | 5/814.8585/1243.3950 | 30/896.7713/1824.0134 |
| nl | summary | 0/—/— | 30/3832.2165/4871.9010 |

### Revised suggestion prompts

All 75 cold-client request samples are present. Four models cover every warm slot; Clef has 235 unsent warm slots after two HTTP429 stops, including a paced continuation. Two Jev responses fail the frozen distribution validator and one Mercury request fails with HTTP502; their consumed slots remain failures. No failed sample is replaced. Request p50/p95 below use successful samples only; consumed counts and raw errors remain separate. Server load/memory/precision and missing Clef warm values are unknown.

| Language | Model | Task | Warm consumed / 30 | Successful n / p50 / p95 ms | Cold-client n / p50 / p95 ms |
| --- | --- | --- | ---: | ---: | ---: |
| en | Jev | events | 30/30 | 30/312.2259/420.7559 | 0/—/— |
| en | Jev | speaker_names | 30/30 | 30/324.4704/417.1830 | 5/301.3384/361.9481 |
| en | Jev | templates | 30/30 | 30/319.4493/411.6403 | 0/—/— |
| mixed | Jev | events | 30/30 | 30/340.8042/420.3465 | 0/—/— |
| mixed | Jev | speaker_names | 30/30 | 30/301.6149/444.8602 | 5/361.2962/401.0364 |
| mixed | Jev | templates | 30/30 | 28/319.5096/518.3872 | 0/—/— |
| nl | Jev | events | 30/30 | 30/328.0711/400.5578 | 0/—/— |
| nl | Jev | speaker_names | 30/30 | 30/337.3311/409.7541 | 5/319.6087/366.7245 |
| nl | Jev | templates | 30/30 | 30/332.4974/404.2673 | 0/—/— |
| en | Decider | events | 30/30 | 30/412.5128/622.0897 | 0/—/— |
| en | Decider | speaker_names | 30/30 | 30/392.2729/641.1362 | 5/331.5426/419.6144 |
| en | Decider | templates | 30/30 | 30/394.3891/568.1393 | 0/—/— |
| mixed | Decider | events | 30/30 | 30/429.8939/750.1463 | 0/—/— |
| mixed | Decider | speaker_names | 30/30 | 30/403.4224/679.8403 | 5/412.7986/671.8817 |
| mixed | Decider | templates | 30/30 | 30/405.9580/451.7569 | 0/—/— |
| nl | Decider | events | 30/30 | 30/414.4682/617.0190 | 0/—/— |
| nl | Decider | speaker_names | 30/30 | 30/424.8895/843.8249 | 5/360.9860/465.7642 |
| nl | Decider | templates | 30/30 | 30/387.3071/565.4284 | 0/—/— |
| en | Flash | events | 30/30 | 30/401.4416/730.7127 | 0/—/— |
| en | Flash | speaker_names | 30/30 | 30/347.9038/513.7967 | 5/276.3608/356.3814 |
| en | Flash | templates | 30/30 | 30/307.5477/383.2553 | 0/—/— |
| mixed | Flash | events | 30/30 | 30/324.6090/599.9309 | 0/—/— |
| mixed | Flash | speaker_names | 30/30 | 30/279.8907/363.4402 | 5/261.6506/329.9988 |
| mixed | Flash | templates | 30/30 | 30/299.5727/674.3895 | 0/—/— |
| nl | Flash | events | 30/30 | 30/317.9417/439.6026 | 0/—/— |
| nl | Flash | speaker_names | 30/30 | 30/346.1400/446.5956 | 5/336.1402/351.4983 |
| nl | Flash | templates | 30/30 | 30/340.9844/436.5143 | 0/—/— |
| en | Clef | events | 30/30 | 30/530.2889/1328.4229 | 0/—/— |
| en | Clef | speaker_names | 5/30 | 3/901.4612/2416.1714 | 5/665.4689/1096.8077 |
| en | Clef | templates | 0/30 | 0/—/— | 0/—/— |
| mixed | Clef | events | 0/30 | 0/—/— | 0/—/— |
| mixed | Clef | speaker_names | 0/30 | 0/—/— | 5/595.3189/759.8206 |
| mixed | Clef | templates | 0/30 | 0/—/— | 0/—/— |
| nl | Clef | events | 0/30 | 0/—/— | 0/—/— |
| nl | Clef | speaker_names | 0/30 | 0/—/— | 5/493.3103/819.9484 |
| nl | Clef | templates | 0/30 | 0/—/— | 0/—/— |
| en | Mercury | events | 30/30 | 30/433.8613/819.8195 | 0/—/— |
| en | Mercury | speaker_names | 30/30 | 30/414.4667/451.2317 | 5/427.8487/470.2183 |
| en | Mercury | templates | 30/30 | 30/433.5885/675.9676 | 0/—/— |
| mixed | Mercury | events | 30/30 | 30/428.5381/622.8774 | 0/—/— |
| mixed | Mercury | speaker_names | 30/30 | 30/409.5958/625.0398 | 5/415.5463/466.3224 |
| mixed | Mercury | templates | 30/30 | 29/430.1229/1542.9929 | 0/—/— |
| nl | Mercury | events | 30/30 | 30/437.5331/553.8440 | 0/—/— |
| nl | Mercury | speaker_names | 30/30 | 30/442.2609/522.4065 | 5/679.8788/2190.3711 |
| nl | Mercury | templates | 30/30 | 30/438.6257/619.4081 | 0/—/— |

The supplemental resource receipts use the revised task prompts. Any quota-limited cells, validation failures and pacing strata are explicit in `supplement/resources.json`; completed samples are never repeated to manufacture a passing panel. Client idle between paced requests is outside the request timer and remains part of command elapsed time. Cold-client setup does not establish server/model-cold latency. Opaque server load, memory, weights, precision and native-package sizes remain unknown. Local, direct-provider and OpenRouter measurements are separate series; no cross-route subtraction estimates gateway overhead.

## Cumulative billing and availability

| Stage | Returned/reconciled billed USD |
| --- | ---: |
| Original hosted experiment | 0.313064388 |
| Additional Mercury timing attempts | 0.007351540 |
| Generative development, quality and timing | 0.178265560 |
| Supplement calibration and held-out quality | 0.022218930 |
| Supplement resource panels/continuations | 0.011581522 |
| **Cumulative** | **0.532481940** |

| Model | Stage (216 scheduled cases) | Returned input tokens | Returned output tokens | Returned billed USD | Usage missing attempts |
| --- | --- | ---: | ---: | ---: | ---: |
| Clef | calibration | 78960 | 0 | 0.003000480 | 0 |
| Decider | calibration | 64848 | 216 | 0.00129696 | 0 |
| Flash | calibration | 64848 | 216 | 0.00064848 | 0 |
| Jev | calibration | 117030 | 11336 | 0.004915260 | 0 |
| Mercury | calibration | 63524 | 215 | 0.00127048 | 1 |
| Clef | heldout | 78465 | 0 | 0.002981670 | 1 |
| Decider | heldout | 64734 | 216 | 0.00129468 | 0 |
| Flash | heldout | 64734 | 216 | 0.00064734 | 0 |
| Jev | heldout | 116310 | 11336 | 0.004885020 | 0 |
| Mercury | heldout | 63928 | 216 | 0.00127856 | 0 |

One persistent ledger covers every stage. Default automatic retries and model/provider fallbacks are disabled. Unknown charges halted paid work until authenticated key-usage observations exactly reconciled all known charges from the zero baseline. Raw failed responses still show missing inline usage/cost; reconciliation does not invent token counts. Explicit continuation warmups, repeated state and partial ranking requests are included in billed totals. Catalog rates are reservation estimates; the table uses returned charges and separately stored reconciliation evidence.

Model downloads remain 2,770,266,421 bytes under the 3 GiB ceiling. Q8 reference parity, the larger official local Decider and secondary Laya remain unavailable within that ceiling. Their existing artifact/environment records are preserved. No new runtime was packaged; Windows/Linux model inference, physical devices and production deployment remain unmeasured. These are explicit deferrals, not failed CI or measured production acceptance.

## Reproduce and inspect

```sh
python3 -B -m unittest discover -s evaluations/decision-models/tests -v
python3 evaluations/decision-models/followup.py --offline \
  --replay evaluations/decision-models/results/followup-2026-10-28/supplement \
  --output build/decision-model-evaluation/replay-suggestions
python3 evaluations/decision-models/generate.py \
  --replay evaluations/decision-models/results/followup-2026-10-28/generative \
  --output build/decision-model-evaluation/replay-generation
```

These commands require no key, paid request, model download or optional model dependency. Temperature normalization uses explicit compensated math.fsum to preserve the original Python3.14 selections on Python3.11; no fit, cutoff or label changed. Reports otherwise differ only by floating-point roundoff; counts and gates stay fixed. The [guide](../../../../docs/decision-model-evaluation.md), frozen manifests and archived continuation scripts document explicit live commands. Future source/hardware/route changes need a new declared experiment, not relabelling these predictions or resetting the shared ledger.

[Issue #53](https://github.com/pennel-app/pennel/issues/53) · [PR #64](https://github.com/pennel-app/pennel/pull/64) · [Retrieval coordination #43](https://github.com/pennel-app/pennel/issues/43) · [Provider contracts #54](https://github.com/pennel-app/pennel/issues/54). Keep the issue open for review/merge and any explicitly unavailable evidence; no production feature is enabled.
