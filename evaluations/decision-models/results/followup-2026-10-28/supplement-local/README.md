# Cached local supplement follow-up

Freeze commit: `fbad0ff`. Same v2 corpus, full ordered-label contexts and original thresholds; cached artifacts only. Both calibration and held-out runs produced 432/432 successful rows. All 18 GLiClass contexts fitted on calibration only. GLiNER produced actual labels but no complete probability distribution, so its 216 held-out rows retain explicit calibration abstentions and no Brier/ECE. Offline measured-data replay passes.

| Model | Language | Owner raw/cal macro-F1 | Template raw/cal macro-F1 | Name raw macro-F1 | Calibrated name precision / suggestion coverage |
|---|---|---|---|---|---|
| gliclass-multilingual | en | 0.1667 / 0.1839 | 0.3143 / 0.3143 | 0.6667 | 1.0 / 0.3333333333333333 |
| gliclass-multilingual | nl | 0.1667 / 0.2807 | 0.2073 / 0.2788 | 0.6667 | 1.0 / 0.5833333333333334 |
| gliclass-multilingual | mixed | 0.1667 / 0.2593 | 0.3143 / 0.2275 | 0.6572 | 1.0 / 0.6666666666666666 |
| gliner-multilingual | en | 0.4539 / 0.0000 | 0.1415 / 0.0000 | 0.6667 | None / 0.0 |
| gliner-multilingual | nl | 0.3111 / 0.0000 | 0.1000 / 0.0000 | 0.6439 | None / 0.0 |
| gliner-multilingual | mixed | 0.4382 / 0.0000 | 0.0615 / 0.0000 | 0.6167 | None / 0.0 |

Owner and template quality fails in every language for both local models. GLiClass name suggestions meet point precision/coverage/ambiguity gates for Dutch and mixed (1.0 precision, 58.3% / 66.7% coverage, no forced ambiguous names) but English coverage is only 33.3%. These are fixed-candidate textual-name observations, not general naming acceptance. GLiNER has no supported fitted selective decision path.

Peak owned-process RSS: GLiClass about1.65GB; GLiNER about3.58GB, both below24GiB. Held-out model loads about2.88s /6.49s. Quality-case latency is one pass, not a replacement for fixed latency panels.

Zero hosted calls, zero hosted cost and zero new downloads. Original total artifact acquisition remains2,770,266,421bytes below3GiB. GTE is ranking-only and inapplicable; Q8 remains deferred for unverified reference parity. No model, prompt, threshold, production setting or credential changed.

Commands:

```sh
python3 evaluations/decision-models/followup.py --prepare --config evaluations/decision-models/configs/supplement-local-2026-10-28.json --frozen evaluations/decision-models/locks/supplement-local-2026-10-28.json --output build/supplement-local-prepared
# Freeze committed as fbad0ff before the following calibration and held-out inference.
python3 evaluations/decision-models/followup.py --live --config evaluations/decision-models/configs/supplement-local-2026-10-28.json --frozen evaluations/decision-models/locks/supplement-local-2026-10-28.json --split calibration --output build/supplement-local-calibration
python3 evaluations/decision-models/followup.py --live --config evaluations/decision-models/configs/supplement-local-2026-10-28.json --frozen evaluations/decision-models/locks/supplement-local-2026-10-28.json --split heldout --calibration build/supplement-local-calibration/calibration.json --output build/supplement-local-heldout
python3 evaluations/decision-models/followup.py --offline --replay build/supplement-local-heldout --output build/supplement-local-replayed
```
