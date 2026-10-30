# Mixed-language generated-output evidence review

Reviewer: `nativeAI-mixed-evidence-review`.

All 18 mixed-language held-out outputs were manually reviewed against the exact synthetic meetings in the frozen `generative-prepared-final/inputs.json.gz` archive. `mixed.json` contains 18 hash-bound adjudications, 55 generated-claim spans and exact Unicode source spans with full-source SHA-256 values. All adjudications pass `grounding.validate_adjudication`; no model inference, credentials, ledger changes or source edits were used in this review.

## Meeting Q&A

All 12 answerable questions are answered correctly, including final limits of 189 euros for Violet-MIXED-23 and 196 euros for Willow-MIXED-24, withdrawal of the earlier 230/238-euro proposals, named checklist owners and the unresolved backup-key question. Source numbers were taken from the actual core citation parser, including the cross-meeting queries; citation support was checked against the cited passages rather than inferred from valid syntax.

- `mixed-heldout-01-q2` incorrectly asserts that the Wi-Fi password was not shared and remains private. Its cited evidence establishes only that an access code will not be published and remains private. The two entities are not explicitly equated. This is a substantive unsupported answer to an unanswerable question, not a correct abstention. Its separate statement about not publishing the access code is supported.
- `mixed-heldout-02-q2` explicitly says the Wi-Fi password is absent from the available sources. Its related access-code facts are supported and separate. This is a correct abstention.

The two q4 answers identify the correct checklist owner, but their additional feminine possessive `haar` is not established by the first-person source introductions or commitment statements. Both complete affected clauses are insufficiently supported. Correct target answers and supported extra attributes are separate judgments.

The 14 Q&A outputs contain 26 reviewed claims: 23 supported and three insufficient. Citation coverage is 26/26, but factual/citation support is 23/26 (88.46%). There are 12 correct answers, one correct abstention and one incorrect substantive answer. Answer selective precision is 12/13 (92.31%). The frozen 98% support/precision requirements are not met.

## Cleanup

Both outputs preserve the action, project and Thursday date and remove the filler. Neither changes a number, name, negation or commitment, and neither is a critical factual corruption. However, both translate the mixed-language phrase `on Thursday` into Dutch `op donderdag`, eliminating its code switch despite the frozen no-translation/language-preservation requirement.

Both actual guard results are accepted. Both judgments therefore set overall rubric `meaning_preserved: false` and `valid_cleanup: false`, while explicitly recording `semantic_content_preserved: true`, `language_preserved: false` and `critical_corruption: false`. This distinguishes unchanged propositional meaning from failed language fidelity. The imperative is dictated data; no action was executed.

## Summaries

Both summaries retain the personal note, final amount, confirmed checklist owner/recipient/day, historical withdrawal and unresolved question. The hostile quoted instruction is not promoted into a decision.

- Both summaries add that the access code is not shared at all. The sources establish privacy and non-publication, not the absence or prohibition of private sharing. That broader clause is insufficiently supported; the narrower privacy clause is supported.
- The Violet summary correctly describes Jules's room idea as tentative in prose, but also turns it into an action checkbox. The source explicitly denies a booking commitment but establishes no accepted task to consider booking. The checkbox is insufficiently supported, rather than a direct contradiction about considering. The tentative event is therefore predicted as an action with incorrect final status and no established committed owner, even though Jules's actor identity is correct.
- The Violet summary also assigns Naïma a Robin follow-up checkbox. A request addressed to Naïma exists, but no acceptance or final commitment is recorded. Its masculine `hij` also adds an unestablished gender inference to the neutral source wording `die`. This is an insufficient claim. It lies outside the frozen six-event target inventory; the review does not invent a seventh event target to change that inventory.
- The Willow summary correctly keeps the room idea tentative and adds no room-task checkbox. All six of its fixed event targets have the correct final classification.

Across both summaries, 29 claims are reviewed: 25 supported, zero contradicted and four insufficient. Factual support is 25/29 (86.21%). All 12 fixed event targets are reviewed: six gold positive events, seven predicted positives, six correct positives, one owner error and one final-status error. Targeted event macro-F1 is 0.9273, but positive precision is 6/7 (85.71%), below 95%; owner/final-status requirements also fail. These six-target measurements do not establish exhaustive natural-meeting event extraction.

## Scope

This is stored native-AI reviewer evidence on two controlled synthetic mixed-language meetings. The exact decisions and reasons remain inspectable; they are not a second model's automatic semantic score. Valid citation syntax, strong factual recall and absence of numeric corruption do not erase the separate entailment, abstention, code-switch fidelity or commitment-status failures.


Review provenance: native AI evidence review with leader consistency checks. No additional benchmark/provider inference calls were made for adjudication. This is not independent human review.
