# Independent supplement v2 quality audit

The final held-out receipt has 1,080 planned and attempted cases: 1,079 successful outputs and one preserved Clef HTTP 429. Calibration has 1,080 attempted cases, 1,079 successful outputs and one preserved Mercury HTTP 502. There are no unsent budget placeholders in this analysis. The initial budget-blocked run is excluded.

All 90 calibration contexts are fitted; the smallest has 11 observations. Every held-out cell has 24 planned cases, representing 12 meeting groups in two paired label orders. Exact stored metrics were reproduced using the existing evaluator and frozen calibration; all case IDs, gold labels, routes and artifact bindings validate. No inference, calibration fitting, probability normalization or threshold changes were performed.

Frozen manifest: `c3129022da30d4896020fb0cc046f3de45b8c0db483fe1495f182ca1233b0315`. Calibration artifact: `fa7ea17e16ee5d07296315d7e83c51918ce54ebd8be5e32028cf13be42fccea7`.

## Frozen calibrated point gates

These are quality gates only. The separate serial resource/timing panel is pending and is not replaced by concurrent quality-call latency. A point gate pass does not establish production integration, device acceptance or an unrestricted model recommendation.

| Task | English | Dutch | Mixed |
| --- | --- | --- | --- |
| Templates | jev | decider | decider, decider-flash, jev |
| Name suggestions | clef-flash, decider, decider-flash, jev, mercury | clef-flash | clef-flash, decider, decider-flash, jev, mercury |
| Owner classification | decider, decider-flash, jev | clef-flash, decider, decider-flash, jev | clef-flash, decider, decider-flash, jev, mercury |

Template gates use the existing calibrated macro-F1 (abstentions count as false negatives), positive-suggestion precision ≥0.95 and positive-suggestion coverage ≥0.50. Names use precision ≥0.98, actual positive-name coverage ≥0.50 and zero accepted names on gold unknown cases. Owners use macro-F1 ≥0.90 and accepted-label precision ≥0.95, with the exact owner/status diagnostics disclosed separately.

## Raw versus calibrated interpretation

Jev, Decider and Mercury classify every eligible raw template case correctly in all three languages. Their stricter frozen calibrated policies nevertheless discard some correct labels, so calibrated macro-F1 falls. For example, English Decider has raw F1 1.0000 but calibrated F1 0.6000, despite perfect precision at 50% actual suggestion coverage. This is the preregistered abstention-as-false-negative calculation; it does not show that accepted labels became wrong. We do not replace the calibrated macro-F1 with raw macro-F1 after observing the test results. If a future product wants unconditional classifier F1 plus separately thresholded suggestion precision/coverage, that contract must be declared separately.

Flash raw template F1 is 0.8993006993 in each language, strictly below 0.90; rounding it to 0.90 must not turn a failure into a pass.

## Per-cell quality and calibration

Each row below has 24 planned cases; all have 24 successful outputs except Clef English templates (23/24). Full precision/recall, class counts, sample denominators, ECE bins, actual suggestion counts and owner diagnostics are retained in the JSON companion. Brier is the unnormalised categorical sum; ECE uses the frozen ten equal-width bins.

| Provider | Language | Task | Raw F1 | Calibrated F1 | Calibrated accepted precision | Actual suggestion or decision coverage | Raw/calibrated Brier | Raw/calibrated ECE | Frozen point gate |
| --- | --- | --- | ---: | ---: | ---: | ---: | --- | --- | --- |
| clef-flash | en | owner | 0.5556 | 0.5556 | 1.0000 | 0.5000 | 0.5007 / 0.5451 | 0.2273 / 0.2444 | Fail |
| clef-flash | en | speaker_names | 1.0000 | 0.6250 | 1.0000 | 0.5833 | 0.0384 / 0.0000 | 0.1072 / 0.0001 | Pass |
| clef-flash | en | templates | 0.8667 | 0.7333 | 0.8462 | 0.5417 | 0.1325 / 0.1742 | 0.1791 / 0.0826 | Fail |
| clef-flash | mixed | owner | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.0084 / 0.0000 | 0.0704 / 0.0000 | Pass |
| clef-flash | mixed | speaker_names | 0.9143 | 0.7000 | 1.0000 | 0.6667 | 0.1352 / 0.1405 | 0.0928 / 0.1988 | Pass |
| clef-flash | mixed | templates | 0.7333 | 0.8000 | 1.0000 | 0.5000 | 0.2069 / 0.3045 | 0.1692 / 0.1523 | Fail |
| clef-flash | nl | owner | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.0012 / 0.0000 | 0.0279 / 0.0000 | Pass |
| clef-flash | nl | speaker_names | 0.8125 | 0.7000 | 1.0000 | 0.6667 | 0.1879 / 0.1898 | 0.1390 / 0.1373 | Pass |
| clef-flash | nl | templates | 0.8381 | 0.7333 | 1.0000 | 0.5000 | 0.1827 / 0.1677 | 0.1327 / 0.1000 | Fail |
| decider | en | owner | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.0000 / 0.0000 | 0.0019 / 0.0001 | Pass |
| decider | en | speaker_names | 1.0000 | 0.7000 | 1.0000 | 0.6667 | 0.0000 / 0.0000 | 0.0018 / 0.0001 | Pass |
| decider | en | templates | 1.0000 | 0.6000 | 1.0000 | 0.5000 | 0.0446 / 0.0008 | 0.1198 / 0.0094 | Fail |
| decider | mixed | owner | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.0000 / 0.0000 | 0.0041 / 0.0000 | Pass |
| decider | mixed | speaker_names | 1.0000 | 0.7500 | 1.0000 | 0.7500 | 0.0024 / 0.0000 | 0.0156 / 0.0008 | Pass |
| decider | mixed | templates | 1.0000 | 0.9333 | 1.0000 | 0.6667 | 0.0517 / 0.0113 | 0.1277 / 0.0339 | Pass |
| decider | nl | owner | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.0000 / 0.0000 | 0.0009 / 0.0000 | Pass |
| decider | nl | speaker_names | 1.0000 | 0.4714 | 1.0000 | 0.3750 | 0.0002 / 0.0000 | 0.0056 / 0.0013 | Fail |
| decider | nl | templates | 1.0000 | 0.9091 | 1.0000 | 0.6667 | 0.0533 / 0.0122 | 0.0992 / 0.0295 | Pass |
| decider-flash | en | owner | 1.0000 | 0.9524 | 1.0000 | 0.9167 | 0.0013 / 0.0000 | 0.0239 / 0.0002 | Pass |
| decider-flash | en | speaker_names | 1.0000 | 0.7045 | 1.0000 | 0.6667 | 0.0012 / 0.0062 | 0.0144 / 0.0404 | Pass |
| decider-flash | en | templates | 0.8993 | 0.8805 | 1.0000 | 0.6250 | 0.1519 / 0.1514 | 0.1762 / 0.0937 | Fail |
| decider-flash | mixed | owner | 1.0000 | 0.9778 | 1.0000 | 0.9583 | 0.0030 / 0.0000 | 0.0377 / 0.0002 | Pass |
| decider-flash | mixed | speaker_names | 1.0000 | 0.7500 | 1.0000 | 0.7500 | 0.0009 / 0.0212 | 0.0149 / 0.1028 | Pass |
| decider-flash | mixed | templates | 0.8993 | 0.9333 | 1.0000 | 0.6667 | 0.1655 / 0.0947 | 0.1544 / 0.0644 | Pass |
| decider-flash | nl | owner | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.0005 / 0.0000 | 0.0160 / 0.0001 | Pass |
| decider-flash | nl | speaker_names | 1.0000 | 0.4917 | 1.0000 | 0.3750 | 0.0053 / 0.0109 | 0.0265 / 0.0476 | Fail |
| decider-flash | nl | templates | 0.8993 | 0.7111 | 1.0000 | 0.5000 | 0.2011 / 0.1165 | 0.2025 / 0.0841 | Fail |
| jev | en | owner | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.0000 / 0.0000 | 0.0000 / 0.0000 | Pass |
| jev | en | speaker_names | 0.6667 | 0.6439 | 1.0000 | 0.5833 | 0.2283 / 0.1681 | 0.1704 / 0.1774 | Pass |
| jev | en | templates | 1.0000 | 1.0000 | 1.0000 | 0.6667 | 0.0232 / 0.0000 | 0.0767 / 0.0000 | Pass |
| jev | mixed | owner | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.0000 / 0.0000 | 0.0008 / 0.0000 | Pass |
| jev | mixed | speaker_names | 0.6667 | 0.7500 | 1.0000 | 0.7500 | 0.2561 / 0.1515 | 0.1775 / 0.1429 | Pass |
| jev | mixed | templates | 1.0000 | 0.9714 | 1.0000 | 0.6250 | 0.0150 / 0.0007 | 0.0608 / 0.0078 | Pass |
| jev | nl | owner | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.0000 / 0.0000 | 0.0000 / 0.0000 | Pass |
| jev | nl | speaker_names | 0.9143 | 0.3631 | 1.0000 | 0.2500 | 0.1017 / 0.1513 | 0.0846 / 0.1175 | Fail |
| jev | nl | templates | 1.0000 | 0.6800 | 1.0000 | 0.5000 | 0.0758 / 0.0511 | 0.1483 / 0.1015 | Fail |
| mercury | en | owner | 1.0000 | 0.8485 | 1.0000 | 0.7917 | 0.0000 / 0.0000 | 0.0012 / 0.0002 | Fail |
| mercury | en | speaker_names | 1.0000 | 0.7500 | 1.0000 | 0.7500 | 0.0000 / 0.0000 | 0.0007 / 0.0001 | Pass |
| mercury | en | templates | 1.0000 | 0.5333 | 1.0000 | 0.4167 | 0.0000 / 0.0000 | 0.0006 / 0.0000 | Fail |
| mercury | mixed | owner | 1.0000 | 0.9302 | 1.0000 | 0.8750 | 0.0000 / 0.0000 | 0.0014 / 0.0002 | Pass |
| mercury | mixed | speaker_names | 1.0000 | 0.5750 | 1.0000 | 0.5000 | 0.0000 / 0.0030 | 0.0017 / 0.0219 | Pass |
| mercury | mixed | templates | 1.0000 | 0.7333 | 1.0000 | 0.5000 | 0.0001 / 0.0000 | 0.0039 / 0.0000 | Fail |
| mercury | nl | owner | 1.0000 | 0.8413 | 1.0000 | 0.7500 | 0.0000 / 0.0000 | 0.0006 / 0.0002 | Fail |
| mercury | nl | speaker_names | 1.0000 | 0.5523 | 1.0000 | 0.4583 | 0.0000 / 0.0000 | 0.0005 / 0.0001 | Fail |
| mercury | nl | templates | 1.0000 | 0.5048 | 1.0000 | 0.3750 | 0.0000 / 0.0000 | 0.0017 / 0.0000 | Fail |

## Unknown names and owner status

The repaired name label space contains both actual self-introductions and unknown cases with the same candidates. Jev raw outputs force a name on all six unknown cases in English and mixed language, and on two Dutch unknown cases. Clef forces names on two mixed and four Dutch unknown cases. The frozen calibrated policies produce no accepted forced name in any cell; these unknown cases are often abstained rather than explicitly predicted unknown. The JSON distinguishes explicit unknown labels from policy abstentions. This is a bounded fixed-candidate result, not open-vocabulary or acoustic identification.

Only Clef meets the calibrated name coverage/precision gates in Dutch. Dutch coverage is Jev 0.2500, Decider/Flash 0.3750 and Mercury 0.4583, below the frozen 0.50 threshold; their accepted positive names remain correct. Clef Dutch coverage is 0.6667 with precision 1.0000.

Raw owner classification is perfect except Clef English, which accepts unknown on eight gold committed-owner cases (raw F1 0.5556). Its calibrated policy has perfect accepted precision but abstains on 12 known commitments and still fails F1. Mercury also fails the frozen calibrated owner F1 gate in English/Dutch because of abstentions, despite perfect raw labels. All calibrated accepted owner decisions have no wrong-known-owner assignment, false committed owner or accepted false-unknown claim in this sample. Exact missing-owner counts remain separate from positive assignment errors; no new numerical zero-error threshold is substituted for the original owner criteria.

## Authored versus reversed labels

Pairing uses original and `variant_of` case IDs, not independent-row averages. There are 12 planned pairs per cell. Clef English templates have 11 evaluable pairs and one incomplete pair from the preserved 429; this missing comparison is not counted as agreement.

Flash templates disagree in 1/12 English pairs, 3/12 Dutch pairs and 1/12 mixed pairs. All other fully observed cells have 0/12 disagreements; Clef English templates have 0/11 observed disagreements plus the incomplete pair. The JSON includes each disagreeing pair and its expected/actual labels. No favorable order was selected after testing.

## Recorded attempt costs

One typed question is sent per request, and both label orders repeat their state and count as separate attempts. Costs below sum unique atomic attempt records; row-level totals are not added again.

| Provider | Calibration known returned cost | Held-out known returned cost | Missing per-attempt cost records |
| --- | ---: | ---: | ---: |
| clef-flash | $0.003000480 | $0.002981670 | 1 |
| decider | $0.00129696 | $0.00129468 | 0 |
| decider-flash | $0.00064848 | $0.00064734 | 0 |
| jev | $0.004915260 | $0.004885020 | 0 |
| mercury | $0.00127048 | $0.00127856 | 1 |

Combined known returned charges for the two quality splits are **$0.022218930** across 2,160 recorded attempts. The Mercury calibration 502 and Clef held-out 429 have no inline per-attempt cost or token usage. They remain unreported here, rather than being assigned a zero charge; the coordinating agent owns account/key-usage reconciliation. No duplicate request IDs were observed. Per-language/task costs and actual returned input/output token counts are available in the JSON.

## Limits

- Twelve meeting groups per order are small samples; paired orders are not 24 independent meetings. Language translations share scenario families, so there is no pooled universal quality winner.
- This is a separately preregistered repair after a known v1 design issue. Its bounded grammar, fixed names and short inputs do not erase v1 long-input refusals or establish natural-meeting acceptance.
- Ninety fitted calibration contexts above the minimum do not establish a broad calibration or 98% population-precision guarantee.
- Serial latency/resource evidence, hosted server internals and production feature/device acceptance remain separate from this quality audit.
