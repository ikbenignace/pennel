"""Hash-bound, stored bilingual evidence judgments; no automated semantic judge."""

import hashlib
import json

from metrics import classification_metrics


RUBRIC = {
    "version": "bilingual-grounding-v1",
    "supported": {"en": "Every material fact, owner, amount, polarity and final status is established by the evidence.",
                  "nl": "Elk materieel feit, eigenaar, bedrag, ontkenning en definitieve status blijkt uit het bewijs."},
    "contradicted": {"en": "Evidence or a later correction contradicts a material part of the claim.",
                     "nl": "Bewijs of een latere correctie spreekt een materieel deel van de bewering tegen."},
    "insufficient": {"en": "Related text does not establish the claim. Valid citation syntax alone is insufficient.",
                     "nl": "Verwante tekst bewijst de bewering niet. Geldige bronverwijzingen op zich volstaan niet."},
    "cleanup": {"en": "Preserve meaning, language, polarity, names, owners, numbers, dates and final commitments; grammar alone may change.",
                "nl": "Behoud betekenis, taal, ontkenningen, namen, eigenaars, getallen, datums en definitieve afspraken; alleen taalverzorging mag wijzigen."},
    "summary_events": {"en": "Classify each of six marked original statements using final context. Omitted events are none; a correctly described retracted or tentative event remains none. Check action owners and final status.",
                       "nl": "Classificeer elk van zes oorspronkelijke gemarkeerde uitspraken in de definitieve context. Ontbrekende gebeurtenissen zijn none; correct vermelde ingetrokken of voorlopige gebeurtenissen blijven none. Controleer eigenaars en definitieve status."},
    "review": "A stored reviewer judgment covers every substantive generated claim. Unresolved rows block adoption. Citation coverage and factual support are separate denominators.",
}


def _hash(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, ensure_ascii=False, separators=(",", ":")).encode()).hexdigest()


RUBRIC_SHA256 = _hash(RUBRIC)
ACCEPTANCE = {"answer_claim_precision_min": 0.98, "answer_coverage_min": 0.5,
              "citation_support_min": 0.98, "citation_coverage_min": 0.98,
              "cleanup_accepted_critical_corruptions_max": 0,
              "cleanup_valid_acceptance_min": 0.9, "unresolved_judgments_max": 0,
              "event_macro_f1_min": 0.9, "event_positive_precision_min": 0.95,
              "event_owner_errors_max": 0, "event_final_status_errors_max": 0}


def response_hash(row):
    value = {key: row.get(key) for key in ("id", "task", "meeting_id", "language", "split", "status", "text", "identity", "citations", "guard")}
    value["gold"] = {key: row.get("adjudication", {})[key] for key in ("expected_answer", "gold_evidence", "raw", "event_targets") if key in row.get("adjudication", {})}
    return _hash(value)


def source_hash(text):
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def pending_adjudication(row):
    return {"id": row["id"], "task": row["task"], "response_sha256": response_hash(row),
            "rubric_sha256": RUBRIC_SHA256, "status": "pending", "reviewer": None,
            "all_substantive_claims_reviewed": False, "claims": [], "answer_outcome": None,
            "cleanup": None, "summary_events": None, "notes": "Apply the frozen bilingual rubric to exact synthetic evidence; do not equate citation syntax with support."}


def _source(span, meetings, split):
    meeting = meetings.get(span.get("meeting_id"))
    if meeting is None or meeting.get("split") != split:
        raise ValueError("grounding_cross_split_or_missing_source")
    kind = span.get("kind")
    if kind == "transcript":
        index = span.get("segment_index")
        if isinstance(index, bool) or not isinstance(index, int) or not 0 <= index < len(meeting.get("transcript", [])):
            raise ValueError("grounding_invalid_segment_index")
    try:
        text = meeting["transcript"][span["segment_index"]]["text"] if kind == "transcript" else meeting[kind]
    except (IndexError, KeyError, TypeError):
        raise ValueError("grounding_invalid_source") from None
    if kind not in ("transcript", "notes", "summary") or not isinstance(text, str):
        raise ValueError("grounding_invalid_source")
    start, end = span.get("start"), span.get("end")
    if any(isinstance(x, bool) or not isinstance(x, int) for x in (start, end)) or not 0 <= start < end <= len(text):
        raise ValueError("grounding_invalid_unicode_span")
    if text[start:end] != span.get("text") or source_hash(text) != span.get("source_sha256"):
        raise ValueError("grounding_source_hash_or_text_mismatch")
    return span


def validate_adjudication(row, judgment, meetings):
    """Validate provenance and review completeness, not the reviewer's semantic decision."""
    if isinstance(meetings, list):
        meetings = {m["id"]: m for m in meetings}
    if judgment.get("id") != row.get("id") or judgment.get("task") != row.get("task") or judgment.get("response_sha256") != response_hash(row):
        raise ValueError("grounding_response_hash_or_identity_mismatch")
    if judgment.get("rubric_sha256") != RUBRIC_SHA256:
        raise ValueError("grounding_rubric_mismatch")
    if judgment.get("status") == "pending":
        return judgment
    if row.get("status") != "ok" or judgment.get("status") != "adjudicated" or not isinstance(judgment.get("reviewer"), str) or not judgment["reviewer"].strip():
        raise ValueError("grounding_complete_reviewer_required")
    if judgment.get("all_substantive_claims_reviewed") is not True:
        raise ValueError("grounding_incomplete_claim_review")
    claims = judgment.get("claims")
    if not isinstance(claims, list):
        raise ValueError("grounding_claims_required")
    generated = row.get("text", "")
    citations = {c["number"]: c.get("passage", {}) for c in row.get("citations", {}).get("citations", [])}
    seen_spans = set()
    for claim in claims:
        start, end = claim.get("start"), claim.get("end")
        if any(isinstance(x, bool) or not isinstance(x, int) for x in (start, end)) or not 0 <= start < end <= len(generated) or generated[start:end] != claim.get("text"):
            raise ValueError("grounding_invalid_generated_span")
        if (start, end) in seen_spans:
            raise ValueError("grounding_duplicate_claim")
        seen_spans.add((start, end))
        if claim.get("verdict") not in ("supported", "contradicted", "insufficient") or not isinstance(claim.get("reason"), str) or not claim["reason"].strip():
            raise ValueError("grounding_claim_judgment_required")
        evidence = claim.get("evidence", [])
        if not isinstance(evidence, list):
            raise ValueError("grounding_evidence_required")
        for span in evidence:
            _source(span, meetings, row["split"])
        if claim["verdict"] in ("supported", "contradicted") and not evidence:
            raise ValueError("grounding_supported_or_contradicted_requires_evidence")
        numbers = claim.get("citations", [])
        if not isinstance(numbers, list) or len(numbers) != len(set(numbers)) or any(isinstance(n, bool) or not isinstance(n, int) or n not in citations for n in numbers):
            raise ValueError("grounding_invalid_citation_number")
        status = claim.get("citation_status")
        if status not in ("supported", "contradicted", "insufficient", "missing", "not_applicable"):
            raise ValueError("grounding_citation_judgment_required")
        if status == "supported":
            if claim["verdict"] != "supported" or not numbers:
                raise ValueError("grounding_citation_support_requires_supported_claim")
            for span in evidence:
                if not any(citations[n].get("meeting_id") == span["meeting_id"] and citations[n].get("kind") == span["kind"] and span["text"] in citations[n].get("text", "") for n in numbers):
                    raise ValueError("grounding_cited_passage_does_not_contain_evidence")
        if status == "missing" and numbers:
            raise ValueError("grounding_missing_citation_has_number")
    if row["task"] == "meeting_qa":
        if judgment.get("answer_outcome") not in ("correct_answer", "correct_abstention", "incorrect_answer", "incorrect_abstention"):
            raise ValueError("grounding_answer_outcome_required")
        if judgment["answer_outcome"] in ("correct_answer", "incorrect_answer") and not claims:
            raise ValueError("grounding_answer_requires_claims")
        if "expected_answer" in row.get("adjudication", {}):
            answerable = row["adjudication"]["expected_answer"] is not None
            if judgment["answer_outcome"] == "correct_answer" and not answerable or judgment["answer_outcome"] == "correct_abstention" and answerable or judgment["answer_outcome"] == "incorrect_abstention" and not answerable:
                raise ValueError("grounding_answer_outcome_conflicts_with_gold_answerability")
    if row["task"] == "cleanup":
        cleanup = judgment.get("cleanup")
        if not isinstance(cleanup, dict) or any(not isinstance(cleanup.get(k), bool) for k in ("meaning_preserved", "critical_corruption", "valid_cleanup")) or not cleanup.get("reason"):
            raise ValueError("grounding_cleanup_judgment_required")
        raw = row.get("adjudication", {}).get("raw")
        if not isinstance(raw, str) or cleanup.get("raw_sha256") != source_hash(raw):
            raise ValueError("grounding_cleanup_raw_hash_mismatch")
        if cleanup["valid_cleanup"] != (cleanup["meaning_preserved"] and not cleanup["critical_corruption"]):
            raise ValueError("grounding_cleanup_inconsistent_judgment")
    if row["task"] == "summary":
        targets = row.get("adjudication", {}).get("event_targets", [])
        events = judgment.get("summary_events")
        if targets:
            if not isinstance(targets, list) or len(targets) != 6 or any(not isinstance(t, dict) or not isinstance(t.get("id"), str) or t.get("expected") not in ("action", "decision", "open_question", "none") for t in targets) or len({t["id"] for t in targets}) != 6:
                raise ValueError("grounding_six_distinct_summary_targets_required")
            if not isinstance(events, list) or len(events) != len(targets) or any(not isinstance(e, dict) or not isinstance(e.get("id"), str) for e in events) or {e["id"] for e in events} != {t["id"] for t in targets}:
                raise ValueError("grounding_summary_events_must_cover_every_target_once")
            if sorted(t.get("expected") for t in targets) != ["action", "decision", "none", "none", "none", "open_question"]:
                raise ValueError("grounding_invalid_summary_target_inventory")
            for target in targets:
                if not isinstance(target["id"], str) or not target["id"] or not target.get("evidence"):
                    raise ValueError("grounding_summary_target_identity_and_evidence_required")
                for span in target["evidence"]:
                    _source(span, meetings, row["split"])
                    if span["meeting_id"] != row["meeting_id"]:
                        raise ValueError("grounding_summary_target_wrong_meeting")
                owner = target.get("expected_owner")
                if target["expected"] == "action":
                    if not isinstance(owner, dict) or not isinstance(owner.get("speaker_id"), str) or not isinstance(owner.get("name"), str) or not owner.get("evidence"):
                        raise ValueError("grounding_summary_owner_gold_required")
                    for span in owner["evidence"]:
                        _source(span, meetings, row["split"])
                elif owner is not None:
                    raise ValueError("grounding_summary_non_action_owner_must_be_null")
            for event in events:
                prediction, indices = event.get("prediction"), event.get("claim_indices")
                if prediction not in ("action", "decision", "open_question", "none") or not isinstance(indices, list) or any(isinstance(i, bool) or not isinstance(i, int) or not 0 <= i < len(claims) for i in indices) or len(indices) != len(set(indices)):
                    raise ValueError("grounding_invalid_summary_event_prediction_or_claims")
                if prediction != "none" and not indices:
                    raise ValueError("grounding_positive_summary_event_requires_claim")
                if prediction == "action" and not isinstance(event.get("owner_correct"), bool) or prediction != "action" and event.get("owner_correct") is not None:
                    raise ValueError("grounding_invalid_summary_event_owner_judgment")
                if not isinstance(event.get("final_status_correct"), bool) or not isinstance(event.get("reason"), str) or not event["reason"].strip():
                    raise ValueError("grounding_summary_event_status_and_reason_required")
        elif events not in (None, []):
            raise ValueError("grounding_summary_events_without_gold_targets")
    return judgment


def _ratio(numerator, denominator):
    return None if denominator == 0 else numerator / denominator


def grounding_report(rows, adjudications, meetings):
    """Regenerate semantic metrics from saved, hash-validated reviewer judgments."""
    judgments = {}
    for judgment in adjudications:
        key = (judgment.get("id"), judgment.get("task"))
        if key in judgments:
            raise ValueError("grounding_duplicate_adjudication")
        judgments[key] = judgment
    result = {"rubric": RUBRIC, "rubric_sha256": RUBRIC_SHA256, "acceptance": ACCEPTANCE,
              "judgment_kind": "stored_reviewer_evidence_adjudication", "languages": {}}
    for row in rows:
        if row.get("task") == "generative_availability":
            continue
        language = result["languages"].setdefault(row["language"], {})
        cell = language.setdefault(row["task"], {"planned": 0, "inference_errors": 0, "adjudicated": 0, "pending": 0,
                                               "claims": 0, "supported_claims": 0, "contradicted_claims": 0,
                                               "insufficient_claims": 0, "cited_claims": 0, "citation_supported_claims": 0,
                                               "correct_answers": 0, "correct_abstentions": 0,
                                               "incorrect_answers": 0, "incorrect_abstentions": 0,
                                               "gold_answerable": 0, "gold_unanswerable": 0, "gold_answerability_unknown": 0,
                                               "answered_gold_answerable": 0, "answered_gold_unanswerable": 0,
                                               "correct_answers_gold_answerable": 0,
                                               "cleanups": 0, "valid_cleanups": 0, "accepted_valid_cleanups": 0,
                                               "critical_corruptions": 0, "accepted_critical_corruptions": 0,
                                               "event_target_count": 0, "event_reviewed_targets": 0, "event_gold_missing_rows": 0,
                                               "gold_positive_events": 0, "predicted_positive_events": 0, "correct_positive_events": 0,
                                               "event_owner_errors": 0, "event_final_status_errors": 0,
                                               "_event_rows": [], "_event_index": {}})
        cell["planned"] += 1
        if row["task"] == "summary":
            targets = row.get("adjudication", {}).get("event_targets", [])
            cell["event_gold_missing_rows"] += not bool(targets)
            for target in targets:
                event_row = {"id": target["id"], "expected": target["expected"], "prediction": None}
                cell["_event_rows"].append(event_row)
                cell["_event_index"][target["id"]] = event_row
                cell["event_target_count"] += 1
                cell["gold_positive_events"] += target["expected"] != "none"
        if row["task"] == "meeting_qa":
            if "expected_answer" not in row.get("adjudication", {}):
                cell["gold_answerability_unknown"] += 1
            else:
                cell["gold_answerable" if row["adjudication"]["expected_answer"] is not None else "gold_unanswerable"] += 1
        if row.get("status") != "ok":
            cell["inference_errors"] += 1
            continue
        judgment = judgments.get((row["id"], row["task"]))
        if judgment is None:
            cell["pending"] += 1
            continue
        validate_adjudication(row, judgment, meetings)
        if judgment["status"] == "pending":
            cell["pending"] += 1
            continue
        cell["adjudicated"] += 1
        for claim in judgment["claims"]:
            cell["claims"] += 1
            cell[claim["verdict"] + "_claims"] += 1
            cell["cited_claims"] += bool(claim.get("citations"))
            cell["citation_supported_claims"] += claim["citation_status"] == "supported"
        outcome = judgment.get("answer_outcome")
        if outcome:
            cell[{"correct_answer": "correct_answers", "correct_abstention": "correct_abstentions", "incorrect_answer": "incorrect_answers", "incorrect_abstention": "incorrect_abstentions"}[outcome]] += 1
            if outcome in ("correct_answer", "incorrect_answer") and "expected_answer" in row.get("adjudication", {}):
                answerable = row["adjudication"]["expected_answer"] is not None
                cell["answered_gold_answerable" if answerable else "answered_gold_unanswerable"] += 1
                cell["correct_answers_gold_answerable"] += answerable and outcome == "correct_answer"
        if row["task"] == "cleanup":
            cleanup = judgment["cleanup"]
            accepted = row.get("guard", {}).get("accepted") is True
            cell["cleanups"] += 1
            cell["valid_cleanups"] += cleanup["valid_cleanup"]
            cell["accepted_valid_cleanups"] += accepted and cleanup["valid_cleanup"]
            cell["critical_corruptions"] += cleanup["critical_corruption"]
            cell["accepted_critical_corruptions"] += accepted and cleanup["critical_corruption"]
        if row["task"] == "summary":
            for event in judgment.get("summary_events") or []:
                target = cell["_event_index"][event["id"]]
                target["prediction"] = event["prediction"]
                cell["event_reviewed_targets"] += 1
                positive = event["prediction"] != "none"
                cell["predicted_positive_events"] += positive
                cell["correct_positive_events"] += positive and event["prediction"] == target["expected"] and event["final_status_correct"] and (event["prediction"] != "action" or event["owner_correct"])
                cell["event_owner_errors"] += event["prediction"] == "action" and not event["owner_correct"]
                cell["event_final_status_errors"] += not event["final_status_correct"]
    for language in result["languages"].values():
        for task, cell in language.items():
            cell.update(factual_support=_ratio(cell["supported_claims"], cell["claims"]),
                        citation_support=_ratio(cell["citation_supported_claims"], cell["cited_claims"]),
                        citation_coverage=_ratio(cell["cited_claims"], cell["claims"]),
                        answer_correctness=_ratio(cell["correct_answers"] + cell["correct_abstentions"], cell["adjudicated"]) if task == "meeting_qa" else None,
                        meaning_preservation=_ratio(cell["valid_cleanups"], cell["cleanups"]),
                        valid_cleanup_acceptance=_ratio(cell["accepted_valid_cleanups"], cell["valid_cleanups"]))
            gates = {"all_judgments_resolved": cell["pending"] <= ACCEPTANCE["unresolved_judgments_max"],
                     "all_planned_inference_successful": cell["inference_errors"] == 0,
                     "at_least_one_adjudication": cell["adjudicated"] > 0}
            if task == "meeting_qa":
                answered = cell["correct_answers"] + cell["incorrect_answers"]
                answerability_precision = _ratio(cell["answered_gold_answerable"], cell["answered_gold_answerable"] + cell["answered_gold_unanswerable"])
                answerability_recall = _ratio(cell["answered_gold_answerable"], cell["gold_answerable"])
                cell.update(answer_selective_precision=_ratio(cell["correct_answers"], answered),
                            answer_coverage=_ratio(answered, cell["planned"]),
                            answer_recall=_ratio(cell["correct_answers_gold_answerable"], cell["gold_answerable"]),
                            answerability_precision=answerability_precision, answerability_recall=answerability_recall,
                            answerability_f1=None if answerability_precision is None or answerability_recall is None else _ratio(2 * answerability_precision * answerability_recall, answerability_precision + answerability_recall) or 0.0,
                            answerability_metric_scope="Answerability treats any substantive answer as a positive decision; content recall counts only correct answers. Planned gold-positive rows remain in recall denominators, including errors and unresolved rows.")
                gates["gold_answerability_available"] = cell["gold_answerability_unknown"] == 0
                for metric, threshold in (("answer_selective_precision", "answer_claim_precision_min"), ("factual_support", "answer_claim_precision_min"),
                                          ("answer_coverage", "answer_coverage_min"), ("citation_support", "citation_support_min"), ("citation_coverage", "citation_coverage_min")):
                    gates[metric] = cell[metric] is not None and cell[metric] >= ACCEPTANCE[threshold]
            elif task == "cleanup":
                gates["accepted_critical_corruptions"] = cell["accepted_critical_corruptions"] <= ACCEPTANCE["cleanup_accepted_critical_corruptions_max"]
                gates["valid_cleanup_acceptance"] = cell["valid_cleanup_acceptance"] is not None and cell["valid_cleanup_acceptance"] >= ACCEPTANCE["cleanup_valid_acceptance_min"]
            elif task == "summary":
                gates["all_factual_claims_supported"] = cell["claims"] > 0 and cell["supported_claims"] == cell["claims"]
                event_metrics = classification_metrics(cell["_event_rows"])
                cell.update(event_classification=event_metrics, event_macro_f1=event_metrics["macro_f1"],
                            event_positive_precision=_ratio(cell["correct_positive_events"], cell["predicted_positive_events"]),
                            event_positive_recall=_ratio(cell["correct_positive_events"], cell["gold_positive_events"]),
                            structured_event_scope="Six marked original statements per meeting (action, final decision, retracted proposal, tentative commitment, open question, quoted injection). This is not exhaustive natural-meeting event extraction.")
                gates["structured_event_completeness_measured"] = cell["event_target_count"] > 0 and cell["event_gold_missing_rows"] == 0 and cell["event_reviewed_targets"] == cell["event_target_count"]
                gates["event_macro_f1"] = cell["event_macro_f1"] is not None and cell["event_macro_f1"] >= ACCEPTANCE["event_macro_f1_min"]
                gates["event_positive_precision"] = cell["event_positive_precision"] is not None and cell["event_positive_precision"] >= ACCEPTANCE["event_positive_precision_min"]
                gates["event_owner_correct"] = cell["event_owner_errors"] <= ACCEPTANCE["event_owner_errors_max"]
                gates["event_final_status_correct"] = cell["event_final_status_errors"] <= ACCEPTANCE["event_final_status_errors_max"]
                if not gates["structured_event_completeness_measured"]:
                    cell["structured_event_gap"] = "At least one marked statement lacks a terminal event judgment or frozen gold target; structured extraction adoption remains deferred."
            cell.pop("_event_rows")
            cell.pop("_event_index")
            cell["gates"] = gates
            cell["adoption_blocked"] = not all(gates.values())
    return result
