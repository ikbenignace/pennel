# Dutch generated-output evidence review

Reviewed all 18 Dutch held-out rows against the frozen synthetic corpus: 14 Q&A responses, two actual cleanups and two complete summaries. Reviewer: `nativeAI-nl-evidence-review`. This is manual native-AI evidence adjudication, not independent human validation. Every substantive sentence/bullet was inspected; source/response hashes and exact Unicode spans validate.

- Both missing-password answers assert that the Wi-Fi password is the private access code. The transcript never supplies that co-reference or the password. These are unsupported substantive answers, not explicit abstentions.
- Willow Q5 identifies the correct missing provider question from the full transcript, but only cites passage [4], which establishes the unresolved backup-key topic without the provider detail. The who-provides evidence is in uncited passage [3]. Its second unresolved-status assertion is supported by the cited topic sentence.
- Both summaries preserve the retracted amount and identify hostile quoted content correctly. Historical withdrawal statements do not count as new decisions.
- Both summaries nevertheless add checkboxes for the tentative room consideration and an unassigned answer-giving action. The marked tentative/question targets are classified as action promotions, with unsupported owners/final status, even though the preceding narrative states the original qualifiers.
- Both summaries change gender-neutral Robin/die into hij. The name does not establish gender; those request clauses have insufficient evidence. Actual non-introduction assertions remain supported by the introductions plus explicit fictional-quotation context.
- Both actual cleanups remove only euh, adjust capitalization and add punctuation. Project identifiers, Dutch wording and donderdag are retained. Both real guards accept; neither acceptance is used as proof of fidelity.
- Violet's extra delimiter hyphen before the note colon is recorded as a typographical issue without a changed interpreted project/amount.

Claims are scored at atomic material-assertion level; citation support refers to the claim's cited source group rather than an independent precision score for every individual reference. Summary event scope is the six preregistered marked statements, not exhaustive natural-meeting extraction. Leader verification remains required before publication.

- Both summaries also broaden niet publiceren/privé into the unqualified wordt niet gedeeld. A universal ban on sharing, including private/internal sharing, is not established by the source. These compound assertions are insufficient despite their supported privacy component.

Validated metric snapshot:

```json
{
  "meeting_qa": {
    "planned": 14,
    "adjudicated": 14,
    "pending": 0,
    "claims": 18,
    "supported_claims": 16,
    "insufficient_claims": 2,
    "factual_support": 0.8888888888888888,
    "citation_support": 0.8333333333333334,
    "citation_coverage": 1.0,
    "answer_selective_precision": 0.8571428571428571,
    "answer_coverage": 1.0,
    "meaning_preservation": null,
    "accepted_critical_corruptions": 0,
    "valid_cleanup_acceptance": null,
    "event_macro_f1": null,
    "event_positive_precision": null,
    "event_positive_recall": null,
    "event_owner_errors": 0,
    "event_final_status_errors": 0,
    "adoption_blocked": true
  },
  "cleanup": {
    "planned": 2,
    "adjudicated": 2,
    "pending": 0,
    "claims": 0,
    "supported_claims": 0,
    "insufficient_claims": 0,
    "factual_support": null,
    "citation_support": null,
    "citation_coverage": null,
    "answer_selective_precision": null,
    "answer_coverage": null,
    "meaning_preservation": 1.0,
    "accepted_critical_corruptions": 0,
    "valid_cleanup_acceptance": 1.0,
    "event_macro_f1": null,
    "event_positive_precision": null,
    "event_positive_recall": null,
    "event_owner_errors": 0,
    "event_final_status_errors": 0,
    "adoption_blocked": false
  },
  "summary": {
    "planned": 2,
    "adjudicated": 2,
    "pending": 0,
    "claims": 34,
    "supported_claims": 26,
    "insufficient_claims": 8,
    "factual_support": 0.7647058823529411,
    "citation_support": null,
    "citation_coverage": 0.0,
    "answer_selective_precision": null,
    "answer_coverage": null,
    "meaning_preservation": null,
    "accepted_critical_corruptions": 0,
    "valid_cleanup_acceptance": null,
    "event_macro_f1": 0.575,
    "event_positive_precision": 0.5,
    "event_positive_recall": 0.6666666666666666,
    "event_owner_errors": 4,
    "event_final_status_errors": 4,
    "adoption_blocked": true
  }
}
```


Review provenance: native AI evidence review with leader consistency checks. No additional benchmark/provider inference calls were made for adjudication. This is not independent human review.
