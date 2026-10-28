import copy
from pathlib import Path
import sys
import unittest


sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from grounding import RUBRIC_SHA256, grounding_report, pending_adjudication, response_hash, source_hash, validate_adjudication


def fixtures():
    text = "Zoë will not send 50 euros."
    meeting = {"id": "m", "split": "heldout", "transcript": [{"text": text}]}
    row = {"id": "q", "task": "meeting_qa", "language": "en", "split": "heldout", "meeting_id": "m", "status": "ok", "text": text + " [1]",
           "identity": {"request_id": "gen-synthetic"},
           "citations": {"numbers": [1], "citations": [{"number": 1, "passage": {"meeting_id": "m", "kind": "transcript", "text": text}}]}}
    span = {"meeting_id": "m", "kind": "transcript", "segment_index": 0, "start": 0, "end": len(text), "text": text, "source_sha256": source_hash(text)}
    judgment = {**pending_adjudication(row), "status": "adjudicated", "reviewer": "stored-synthetic-reviewer", "all_substantive_claims_reviewed": True,
                "claims": [{"start": 0, "end": len(text), "text": text, "verdict": "supported", "reason": "Exact polarity and amount are preserved.",
                            "evidence": [span], "citations": [1], "citation_status": "supported"}], "answer_outcome": "correct_answer"}
    return row, judgment, [meeting]


class GroundingTests(unittest.TestCase):
    def test_exact_stored_judgments_replay_without_inference(self):
        row, judgment, meetings = fixtures()
        self.assertEqual(validate_adjudication(row, judgment, meetings), judgment)
        report = grounding_report([row], [judgment], meetings)
        cell = report["languages"]["en"]["meeting_qa"]
        self.assertEqual(cell["factual_support"], 1)
        self.assertEqual(cell["citation_support"], 1)
        self.assertEqual(cell["citation_coverage"], 1)
        self.assertEqual(cell["answer_correctness"], 1)
        self.assertEqual(report["rubric_sha256"], RUBRIC_SHA256)

    def test_unresolved_is_not_zero_and_blocks_adoption(self):
        row, judgment, meetings = fixtures()
        second = {**row, "id": "unresolved", "text": "Unreviewed text."}
        report = grounding_report([row, second], [judgment], meetings)
        cell = report["languages"]["en"]["meeting_qa"]
        self.assertEqual(cell["adjudicated"], 1)
        self.assertEqual(cell["pending"], 1)
        self.assertEqual(cell["factual_support"], 1)
        self.assertTrue(cell["adoption_blocked"])
        only_pending = grounding_report([second], [], meetings)["languages"]["en"]["meeting_qa"]
        self.assertIsNone(only_pending["factual_support"])

    def test_response_and_rubric_drift_rejected(self):
        row, judgment, meetings = fixtures()
        for changed in ({**row, "text": "Other output."}, {**row, "identity": {"request_id": "other"}}):
            with self.assertRaisesRegex(ValueError, "response_hash"):
                validate_adjudication(changed, judgment, meetings)
        judgment["rubric_sha256"] = "changed"
        with self.assertRaisesRegex(ValueError, "rubric"):
            validate_adjudication(row, judgment, meetings)

    def test_unicode_source_offsets_hash_and_meeting_split_are_validated(self):
        for change in ("offset", "hash", "split"):
            row, judgment, meetings = fixtures()
            if change == "offset":
                judgment["claims"][0]["evidence"][0]["end"] -= 1
            elif change == "hash":
                judgment["claims"][0]["evidence"][0]["source_sha256"] = "wrong"
            else:
                meetings[0]["split"] = "calibration"
            with self.assertRaises(ValueError):
                validate_adjudication(row, judgment, meetings)

    def test_valid_citation_syntax_does_not_establish_support(self):
        row, judgment, meetings = fixtures()
        judgment["claims"][0].update(verdict="insufficient", citation_status="insufficient", reason="Related source does not establish the generated claim.")
        cell = grounding_report([row], [judgment], meetings)["languages"]["en"]["meeting_qa"]
        self.assertEqual(cell["citation_coverage"], 1)
        self.assertEqual(cell["factual_support"], 0)
        self.assertEqual(cell["citation_support"], 0)

    def test_supported_citation_must_actually_include_exact_evidence(self):
        row, judgment, meetings = fixtures()
        row["citations"]["citations"][0]["passage"]["text"] = "Other evidence."
        judgment["response_sha256"] = response_hash(row)
        with self.assertRaisesRegex(ValueError, "does_not_contain"):
            validate_adjudication(row, judgment, meetings)

    def test_missing_citation_is_coverage_failure_separate_from_factual_support(self):
        row, judgment, meetings = fixtures()
        judgment["claims"][0].update(citations=[], citation_status="missing")
        cell = grounding_report([row], [judgment], meetings)["languages"]["en"]["meeting_qa"]
        self.assertEqual(cell["factual_support"], 1)
        self.assertEqual(cell["citation_coverage"], 0)
        self.assertIsNone(cell["citation_support"])

    def test_incomplete_or_duplicate_review_is_rejected(self):
        for change in ("reviewer", "all_substantive_claims_reviewed", "claim"):
            row, judgment, meetings = fixtures()
            if change == "claim":
                judgment["claims"].append(copy.deepcopy(judgment["claims"][0]))
            else:
                judgment[change] = None
            with self.assertRaises(ValueError):
                validate_adjudication(row, judgment, meetings)
        row, judgment, meetings = fixtures()
        with self.assertRaisesRegex(ValueError, "duplicate_adjudication"):
            grounding_report([row], [judgment, judgment], meetings)

    def test_correct_abstention_can_contain_no_factual_claims(self):
        row, judgment, meetings = fixtures()
        row["text"] = "This was not established by the meeting."
        judgment.update(response_sha256=response_hash(row), answer_outcome="correct_abstention", claims=[])
        cell = grounding_report([row], [judgment], meetings)["languages"]["en"]["meeting_qa"]
        self.assertEqual(cell["correct_abstentions"], 1)
        self.assertIsNone(cell["factual_support"])

    def test_cleanup_fidelity_is_not_overlap_guard_acceptance(self):
        row, _, meetings = fixtures()
        row.update(id="cleanup", task="cleanup", text="Zoë will send 50 euros.", guard={"accepted": True}, adjudication={"raw": "Zoë will not send 50 euros."})
        judgment = {**pending_adjudication(row), "status": "adjudicated", "reviewer": "stored-synthetic-reviewer", "all_substantive_claims_reviewed": True,
                    "cleanup": {"raw_sha256": source_hash(row["adjudication"]["raw"]), "meaning_preserved": False, "critical_corruption": True,
                                "valid_cleanup": False, "reason": "A negation was removed."}}
        cell = grounding_report([row], [judgment], meetings)["languages"]["en"]["cleanup"]
        self.assertEqual(cell["accepted_critical_corruptions"], 1)
        self.assertEqual(cell["meaning_preservation"], 0)
        self.assertIsNone(cell["valid_cleanup_acceptance"])
        self.assertTrue(cell["adoption_blocked"])
        judgment["cleanup"]["raw_sha256"] = "wrong"
        with self.assertRaisesRegex(ValueError, "raw_hash"):
            validate_adjudication(row, judgment, meetings)

    def test_always_abstaining_cannot_pass_selective_answer_gates(self):
        row, judgment, meetings = fixtures()
        row.update(text="The answer was not provided.", adjudication={"expected_answer": "50"})
        judgment.update(response_sha256=response_hash(row), answer_outcome="incorrect_abstention", claims=[])
        cell = grounding_report([row], [judgment], meetings)["languages"]["en"]["meeting_qa"]
        self.assertEqual(cell["answer_coverage"], 0)
        self.assertIsNone(cell["answer_selective_precision"])
        self.assertEqual(cell["answer_recall"], 0)
        self.assertEqual(cell["answerability_recall"], 0)
        self.assertTrue(cell["adoption_blocked"])

    def test_answer_content_quality_is_separate_from_answerability(self):
        row, judgment, meetings = fixtures()
        row["adjudication"] = {"expected_answer": "50"}
        judgment.update(response_sha256=response_hash(row), answer_outcome="incorrect_answer")
        judgment["claims"][0].update(verdict="insufficient", citation_status="insufficient")
        cell = grounding_report([row], [judgment], meetings)["languages"]["en"]["meeting_qa"]
        self.assertEqual(cell["answerability_precision"], 1)
        self.assertEqual(cell["answerability_recall"], 1)
        self.assertEqual(cell["answerability_f1"], 1)
        self.assertEqual(cell["answer_selective_precision"], 0)
        self.assertEqual(cell["answer_recall"], 0)
        self.assertTrue(cell["adoption_blocked"])

    def test_errors_stay_in_planned_coverage_and_gold_recall_denominators(self):
        row, judgment, meetings = fixtures()
        row["adjudication"] = {"expected_answer": "50"}
        judgment["response_sha256"] = response_hash(row)
        error = {**row, "id": "error", "status": "error", "text": None}
        cell = grounding_report([row, error], [judgment], meetings)["languages"]["en"]["meeting_qa"]
        self.assertEqual(cell["answer_selective_precision"], 1)
        self.assertEqual(cell["answer_coverage"], 0.5)
        self.assertEqual(cell["answer_recall"], 0.5)
        self.assertEqual(cell["answerability_recall"], 0.5)
        self.assertTrue(cell["adoption_blocked"])

    def test_answer_outcomes_and_response_hash_bind_gold_answerability(self):
        for expected, outcome in ((None, "correct_answer"), ("50", "correct_abstention"), (None, "incorrect_abstention")):
            row, judgment, meetings = fixtures()
            row["adjudication"] = {"expected_answer": expected}
            judgment.update(response_sha256=response_hash(row), answer_outcome=outcome)
            with self.assertRaisesRegex(ValueError, "conflicts_with_gold"):
                validate_adjudication(row, judgment, meetings)
        row, judgment, meetings = fixtures()
        row["adjudication"] = {"expected_answer": "50"}
        judgment["response_sha256"] = response_hash(row)
        row["adjudication"]["expected_answer"] = None
        with self.assertRaisesRegex(ValueError, "response_hash"):
            validate_adjudication(row, judgment, meetings)

    def test_summary_support_and_missing_event_coverage_block_adoption(self):
        row, judgment, meetings = fixtures()
        row["task"] = "summary"
        judgment.update(task="summary", response_sha256=response_hash(row), answer_outcome=None)
        cell = grounding_report([row], [judgment], meetings)["languages"]["en"]["summary"]
        self.assertTrue(cell["gates"]["all_factual_claims_supported"])
        self.assertFalse(cell["gates"]["structured_event_completeness_measured"])
        self.assertTrue(cell["adoption_blocked"])
        judgment["claims"][0].update(verdict="insufficient", citation_status="insufficient")
        cell = grounding_report([row], [judgment], meetings)["languages"]["en"]["summary"]
        self.assertFalse(cell["gates"]["all_factual_claims_supported"])


if __name__ == "__main__":
    unittest.main()
