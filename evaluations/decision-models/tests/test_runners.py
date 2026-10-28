import copy
import importlib.util
import json
import math
import sys
import unittest
from pathlib import Path
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from runners import DecisionRunner, EvaluationError, SpendLedger, validate_answers, _NoRedirect


MODEL = "typesafe/jev-1.14"
CANONICAL = "typesafe/jev-1.14-20261014"


def config(**changes):
    result = {
        "enabled": True, "transport": "openrouter", "model": MODEL,
        "upstream_tag": "typesafe", "credential_env": "OPENROUTER_API_KEY",
        "max_input_tokens": 32000, "billing_overhead_tokens": 64,
        "privacy": {"processing_region": "global", "policy_verified": True,
                    "policy_url": "https://docs.typesafe.ai/models", "data_collection": "deny"},
    }
    result.update(changes)
    return result


def case(question=None):
    return {"id": "synthetic-case", "task": "answerability", "state": "Synthetic meeting.",
            "question": question or {"type": "noul", "instructions": "Does the source contain the answer?"}}


def reply(**changes):
    body = {"model": CANONICAL, "provider": "TypeSafe", "id": "gen-synthetic",
            "answers": {"decision": {"type": "noul", "noul": 0.8}},
            "usage": {"input_tokens": 120, "output_tokens": 5, "cost": 0.000001}}
    body.update(changes)
    return body


class Transport:
    is_mock = True

    def __init__(self, body=None, status=200, failure=None):
        self.body = reply() if body is None else body
        self.status = status
        self.failure = failure
        self.calls = []
        self.catalog = {"data": [{"id": MODEL, "canonical_slug": CANONICAL,
                                 "architecture": {"output_modalities": ["decisions"]},
                                 "pricing": {"prompt": "0.000000042", "completion": "0"},
                                 "context_length": 64000}]}
        self.endpoints = {"data": {"endpoints": [{"tag": "typesafe", "provider_name": "TypeSafe",
                                                  "context_length": 64000}]}}

    def __call__(self, method, url, headers, payload, timeout):
        self.calls.append({"method": method, "url": url, "headers": headers, "payload": payload})
        if method == "GET":
            body = self.endpoints if url.endswith("/endpoints") else self.catalog
            return {"status": 200, "headers": {}, "body": copy.deepcopy(body)}
        if self.failure:
            raise self.failure
        return {"status": self.status, "headers": {"x-request-id": "upstream-synthetic"},
                "body": copy.deepcopy(self.body)}


class RunnerTests(unittest.TestCase):
    def runner(self, settings=None, transport=None, ledger=None, environ=None):
        return DecisionRunner(settings or config(), transport=transport or Transport(), ledger=ledger,
                              environ={"OPENROUTER_API_KEY": "synthetic-secret"} if environ is None else environ)

    def test_import_and_disabled_runner_cannot_use_network_or_credentials(self):
        transport = Transport()
        with patch("urllib.request.OpenerDirector.open", side_effect=AssertionError("network forbidden")):
            runner = self.runner(config(enabled=False), transport)
            self.assertEqual(runner.predict(case())["reason"], "live_evaluation_disabled")
        self.assertEqual(transport.calls, [])

    def test_missing_credentials_do_not_probe_network(self):
        transport = Transport()
        self.assertEqual(self.runner(transport=transport, environ={}).preflight()["reason"],
                         "evaluation_credential_unavailable")
        self.assertEqual(transport.calls, [])

    def test_decisions_typed_result_and_billed_metadata(self):
        transport, ledger = Transport(), SpendLedger()
        result = self.runner(transport=transport, ledger=ledger).predict(case())
        self.assertEqual(result["status"], "ok")
        self.assertEqual(result["prediction"], "true")
        self.assertEqual(result["probabilities"], {"false": 0.19999999999999996, "true": 0.8})
        self.assertEqual(result["identity"]["returned_model"], CANONICAL)
        self.assertEqual(result["identity"]["provider"], "TypeSafe")
        self.assertEqual(result["identity"]["request_id"], "gen-synthetic")
        self.assertEqual(result["cost_usd"], "0.000001")
        self.assertEqual(result["measurement"], "mocked_transport")
        self.assertEqual(ledger.snapshot()["known_billed_usd"], "0.000001")
        post = transport.calls[-1]
        self.assertTrue(post["url"].endswith("/api/alpha/decisions"))
        self.assertEqual(post["payload"]["provider"]["only"], ["typesafe"])
        self.assertIs(post["payload"]["provider"]["allow_fallbacks"], False)
        self.assertEqual(post["payload"]["provider"]["data_collection"], "deny")
        self.assertNotIn("messages", post["payload"])
        self.assertIn("output_modalities=decisions", transport.calls[0]["url"])
        self.assertEqual(result["identity"]["upstream_request_id"], "upstream-synthetic")

    def test_systemone_is_explicit_and_does_not_change_api_on_error(self):
        transport = Transport(status=503)
        result = self.runner(config(api_surface="systemone"), transport).predict(case())
        self.assertEqual(result["reason"], "http_503")
        posts = [call for call in transport.calls if call["method"] == "POST"]
        self.assertEqual(len(posts), 1)
        self.assertTrue(posts[0]["url"].endswith("/api/v1/systemone"))

    def test_catalog_or_route_absence_is_unavailable_without_post(self):
        for route_missing in (False, True):
            transport = Transport()
            if route_missing:
                transport.endpoints = {"data": {"endpoints": []}}
            else:
                transport.catalog = {"data": []}
            result = self.runner(transport=transport).predict(case())
            self.assertEqual(result["status"], "unavailable")
            self.assertFalse(any(call["method"] == "POST" for call in transport.calls))

    def test_text_output_catalog_is_not_a_decision_capability(self):
        transport = Transport()
        transport.catalog["data"][0]["architecture"]["output_modalities"] = ["text"]
        self.assertEqual(self.runner(transport=transport).preflight()["reason"],
                         "requested_decision_model_unavailable")

    def test_liquid_or_other_checkpoint_is_never_equated(self):
        result = self.runner(transport=Transport(reply(model="liquid/d1.1"))).predict(case())
        self.assertEqual(result["reason"], "returned_model_mismatch")
        self.assertEqual(result["identity"]["returned_model"], "liquid/d1.1")
        transport = Transport()
        self.assertEqual(self.runner(config(expected_returned_model="LiquidAI/d1-2B"), transport).preflight()["reason"],
                         "configured_checkpoint_catalog_mismatch")

    def test_provider_mismatch_records_cost_and_rejects_quality_result(self):
        transport = Transport(reply(provider="Different provider"))
        runner = self.runner(transport=transport)
        result = runner.predict(case())
        self.assertEqual(result["reason"], "returned_provider_mismatch")
        self.assertEqual(result["cost_usd"], "0.000001")
        self.assertEqual(runner.predict(case())["reason"], "identity_drift_requires_revalidation")
        self.assertEqual(sum(call["method"] == "POST" for call in transport.calls), 1)

    def test_missing_metadata_stays_unknown(self):
        body = reply()
        del body["provider"]
        del body["id"]
        result = self.runner(transport=Transport(body)).predict(case())
        self.assertIsNone(result["identity"]["provider"])
        self.assertIn("actual_provider_unreported", result["limitations"])
        self.assertIn("request_id_unreported", result["limitations"])
        self.assertEqual(result["reproducibility"], "partial")

    def test_malformed_identity_is_rejected_sanitized_and_still_billed(self):
        for field, reason in (("model", "returned_model_invalid_type"),
                              ("provider", "returned_provider_invalid_type"),
                              ("id", "returned_request_id_invalid_type")):
            for malformed in (["private-synthetic-content"], {"private-synthetic-content": "value"}, 42, True):
                with self.subTest(field=field, value_type=type(malformed).__name__):
                    transport, ledger = Transport(reply(**{field: malformed})), SpendLedger()
                    result = self.runner(transport=transport, ledger=ledger).predict(case())
                    self.assertEqual(result["status"], "error")
                    self.assertEqual(result["reason"], reason)
                    self.assertEqual(ledger.snapshot()["known_billed_usd"], "0.000001")
                    self.assertEqual(ledger.snapshot()["attempt_count"], 1)
                    self.assertEqual(ledger.snapshot()["reserved_usd"], "0")
                    self.assertNotIn("private-synthetic-content", json.dumps(result))
                    self.assertNotIn("private-synthetic-content", json.dumps(ledger.snapshot()))

    def test_unhashable_choice_yields_error_result_without_losing_billed_attempt(self):
        question = {"type": "choice", "instructions": "Choose a supported label.",
                    "criteria": {"a": "Supported", "b": "Unsupported"}}
        for malformed in (["a"], {"a": "private-synthetic-content"}):
            body = reply(answers={"decision": {"type": "choice", "choice": malformed,
                                               "probabilities": {"a": 0.8, "b": 0.2}}})
            ledger = SpendLedger()
            result = self.runner(transport=Transport(body), ledger=ledger).predict(case(question))
            self.assertEqual(result["status"], "error")
            self.assertEqual(result["reason"], "unknown_answer_id")
            self.assertEqual(ledger.snapshot()["known_billed_usd"], "0.000001")
            self.assertEqual(ledger.snapshot()["attempt_count"], 1)
            self.assertNotIn("private-synthetic-content", json.dumps(result))

    def test_unknown_billed_cost_halts_subsequent_paid_work(self):
        ledger, transport = SpendLedger(), Transport(reply(usage={"input_tokens": 120, "output_tokens": 5}))
        runner = self.runner(transport=transport, ledger=ledger)
        result = runner.predict(case())
        self.assertEqual(result["status"], "ok")
        self.assertIsNone(result["cost_usd"])
        self.assertTrue(ledger.snapshot()["unresolved_charge"])
        self.assertEqual(runner.predict(case())["reason"], "unresolved_charge")
        self.assertEqual(sum(call["method"] == "POST" for call in transport.calls), 1)

    def test_http_errors_and_timeout_never_retry_or_use_direct_provider(self):
        for status in (401, 402, 429, 500, 503):
            with self.subTest(status=status):
                transport = Transport({"error": {"message": "synthetic-secret Synthetic meeting."}}, status)
                result = self.runner(transport=transport).predict(case())
                self.assertEqual(result["reason"], "http_" + str(status))
                self.assertEqual(len([call for call in transport.calls if call["method"] == "POST"]), 1)
                self.assertNotIn("synthetic-secret", json.dumps(result))
                self.assertNotIn("Synthetic meeting", json.dumps(result))
        for failure, reason in ((TimeoutError(), "transport_error_or_timeout"),
                                (EvaluationError("redirect_rejected"), "redirect_rejected")):
            transport = Transport(failure=failure)
            result = self.runner(transport=transport).predict(case())
            self.assertEqual(result["reason"], reason)
            self.assertEqual(len([call for call in transport.calls if call["method"] == "POST"]), 1)

    def test_unverified_privacy_and_wrong_region_reject_before_network(self):
        for privacy, expected in (({}, "route_policy_not_verified"),
                                  ({"policy_verified": True, "policy_url": "https://example.com", "processing_region": "eu"},
                                   "openrouter_region_host_mismatch")):
            transport = Transport()
            result = self.runner(config(privacy=privacy), transport).predict(case())
            self.assertEqual(result["reason"], expected)
            self.assertEqual(transport.calls, [])

    def test_required_region_and_zdr_are_preserved(self):
        privacy = {"policy_verified": True, "policy_url": "https://example.com", "processing_region": "eu",
                   "data_collection": "deny", "zdr": True}
        transport = Transport()
        result = self.runner(config(base_url="https://eu.openrouter.ai", privacy=privacy), transport).predict(case())
        self.assertEqual(result["status"], "ok")
        self.assertTrue(all(call["url"].startswith("https://eu.openrouter.ai/") for call in transport.calls))
        self.assertIs(transport.calls[-1]["payload"]["provider"]["zdr"], True)

    def test_oversized_inputs_are_not_sent_or_truncated(self):
        transport = Transport()
        large = case()
        large["state"] = "x" * 64000
        result = self.runner(transport=transport).predict(large)
        self.assertEqual(result["reason"], "context_bound_exceeded_no_truncation")
        self.assertFalse(any(call["method"] == "POST" for call in transport.calls))

    def test_aliases_retries_and_fallbacks_are_rejected(self):
        for settings in (config(model="~typesafe/jev-latest"), config(model="openrouter/auto-router"),
                         config(retries=1), config(fallbacks=["liquid/d1"]), config(api_surface="chat")):
            transport = Transport()
            self.assertEqual(self.runner(settings, transport).predict(case())["status"], "unavailable")
            self.assertEqual(transport.calls, [])

    def test_unsupported_question_never_uses_chat(self):
        transport = Transport()
        result = self.runner(transport=transport).predict(case({"type": "extract", "instructions": "Find a name"}))
        self.assertEqual(result["reason"], "unsupported_decision_capability")
        self.assertFalse(any(call["method"] == "POST" for call in transport.calls))

    def test_direct_jev_and_local_decider_are_independent(self):
        direct = config(transport="direct", provider="jev", model="jev-1.14.0",
                        credential_env="TYPESAFE_API_KEY", input_usd_per_million="0.042")
        transport = Transport(reply(model="jev-1.14.0"))
        result = self.runner(direct, transport, environ={"TYPESAFE_API_KEY": "separate-synthetic"}).predict(case())
        self.assertEqual(result["status"], "ok")
        self.assertEqual(len(transport.calls), 1)
        self.assertEqual(transport.calls[0]["url"], "https://api.typesafe.ai/v1/systemone")
        self.assertNotIn("provider", transport.calls[0]["payload"])
        self.assertEqual(self.runner(dict(direct, credential_env="OPENROUTER_API_KEY")).preflight()["reason"],
                         "openrouter_key_cannot_be_forwarded")
        transport = Transport(reply(model="pplx-decider-flash-v1.2", usage={"input_tokens": 120, "output_tokens": 5}))
        local = {"enabled": True, "transport": "local_decider", "model": "pplx-decider-flash-v1.2",
                 "max_input_tokens": 32768}
        result = self.runner(local, transport, environ={}).predict(case())
        self.assertEqual(result["status"], "ok")
        self.assertTrue(transport.calls[0]["url"].startswith("http://127.0.0.1:8080/"))
        self.assertNotIn("Authorization", transport.calls[0]["headers"])
        self.assertFalse(result["cost_usd"])

    def test_direct_provider_routes_and_cloudflare_result_envelope(self):
        routes = (
            ("perplexity", "pplx-decider-v1.2", "https://api.perplexity.ai/v1/decisions", "PERPLEXITY_API_KEY"),
            ("mercury", "mercury-decide", "https://api.inceptionlabs.ai/v1/decisions", "INCEPTION_API_KEY"),
            ("clef", "clef-flash", "https://api.cloudflare.com/client/v4/accounts/synthetic/ai/run/@cf/cloudflare/clef-flash", "CLOUDFLARE_API_TOKEN"),
        )
        for provider, model, endpoint, credential in routes:
            settings = config(transport="direct", provider=provider, model=model, endpoint=endpoint,
                              credential_env=credential, input_usd_per_million="0.042")
            body = reply(model=model)
            if provider == "clef":
                body = {"success": True, "result": body}
            transport = Transport(body)
            result = self.runner(settings, transport, environ={credential: "synthetic-direct-key"}).predict(case())
            self.assertEqual(result["status"], "ok")
            self.assertEqual(len(transport.calls), 1)
            self.assertEqual(transport.calls[0]["url"], endpoint)

    def test_explicit_label_order_reaches_wire_and_receipt(self):
        transport = Transport()
        ordered = case({"type": "noul", "instructions": "Supported?",
                        "criteria": {"false": "No", "true": "Yes"}})
        ordered["label_order"] = ["true", "false"]
        result = self.runner(transport=transport).predict(ordered)
        self.assertEqual(list(transport.calls[-1]["payload"]["questions"]["decision"]["criteria"]), ["true", "false"])
        self.assertEqual(result["identity"]["label_order"]["decision"], ["true", "false"])
        self.assertEqual(list(ordered["question"]["criteria"]), ["false", "true"])

    def test_real_http_redirect_handler_rejects_other_origins(self):
        with self.assertRaisesRegex(EvaluationError, "redirect_rejected"):
            _NoRedirect().redirect_request(None, None, 307, "redirect", {}, "https://other.example")

    def test_remote_insecure_and_remote_local_decider_urls_rejected(self):
        for settings in (config(transport="direct", provider="jev", endpoint="http://api.typesafe.ai/v1/systemone"),
                         {"enabled": True, "transport": "local_decider", "model": "pplx-decider-flash-v1.2",
                          "endpoint": "http://example.com/v1/decisions"}):
            self.assertEqual(self.runner(settings).preflight()["status"], "unavailable")

    def test_repeated_state_reservation_and_returned_billed_usage(self):
        runner = self.runner()
        runner.preflight()
        grouped = case()
        grouped["questions"] = {"a": grouped["question"], "b": grouped["question"]}
        payload, bound = runner._payload(grouped, grouped["questions"])
        single_payload, single_bound = runner._payload(case(), {"decision": case()["question"]})
        self.assertGreaterEqual(bound, single_bound * 2)
        self.assertEqual(len(payload["questions"]), 2)

    def test_explicit_repeated_request_is_counted_as_another_billed_attempt(self):
        ledger = SpendLedger()
        runner = self.runner(ledger=ledger)
        first, second = runner.predict(case()), runner.predict(case())
        self.assertEqual(first["status"], "ok")
        self.assertEqual(second["status"], "ok")
        self.assertEqual(ledger.snapshot()["attempt_count"], 2)
        self.assertEqual(ledger.snapshot()["known_billed_usd"], "0.000002")


class AnswerTests(unittest.TestCase):
    def test_label_order_does_not_change_interpreted_identity(self):
        answer = {"type": "choice", "choice": "yes", "probabilities": {"yes": 0.7, "no": 0.3}, "confidence": 0.4}
        for labels in ({"yes": "Supported", "no": "Unsupported"}, {"no": "Unsupported", "yes": "Supported"}):
            result = validate_answers({"answers": {"q": answer}}, {"q": {"type": "choice", "criteria": labels}})["q"]
            self.assertEqual(result["prediction"], "yes")
            self.assertEqual(result["confidence"], 0.4)
            self.assertEqual(result["raw_score"], 0.7)

    def test_malformed_distributions_unknown_ids_and_nonfinite_rejected(self):
        question = {"q": {"type": "choice", "criteria": {"yes": "yes", "no": "no"}}}
        for answer in (
            {"type": "choice", "choice": ["yes"], "probabilities": {"yes": 0.8, "no": 0.2}},
            {"type": "choice", "choice": {"yes": True}, "probabilities": {"yes": 0.8, "no": 0.2}},
            {"type": "choice", "choice": "other", "probabilities": {"yes": 0.8, "no": 0.2}},
            {"type": "choice", "choice": "yes", "probabilities": {"yes": 0.8}},
            {"type": "choice", "choice": "yes", "probabilities": {"yes": 0.8, "no": 0.5}},
            {"type": "choice", "choice": "yes", "probabilities": {"yes": float("nan"), "no": 0.2}},
            {"type": "choice", "choice": "yes", "probabilities": {"yes": 0.2, "no": 0.8}},
        ):
            with self.assertRaises(EvaluationError):
                validate_answers({"answers": {"q": answer}}, question)
        with self.assertRaises(EvaluationError):
            validate_answers({"answers": {"other": {"type": "noul", "noul": 0.9}}}, {"q": {"type": "noul"}})

    def test_binary_and_ordinal_semantics(self):
        for probability in (-0.1, float("inf"), True, "0.8"):
            with self.assertRaises(EvaluationError):
                validate_answers({"answers": {"q": {"type": "noul", "noul": probability}}}, {"q": {"type": "noul"}})
        questions = {"q": {"type": "score", "criteria": ["low", "medium", "high"]}}
        answer = {"type": "score", "probabilities": {"0": 0.1, "1": 0.2, "2": 0.7}, "score": 1.6,
                  "legend": {"0": "low", "1": "medium", "2": "high"}}
        result = validate_answers({"answers": {"q": answer}}, questions)["q"]
        self.assertEqual(result["prediction"], "2")
        self.assertEqual(result["raw_score_type"], "ordinal_probabilities")
        answer["score"] = 0.2
        with self.assertRaises(EvaluationError):
            validate_answers({"answers": {"q": answer}}, questions)


class LedgerTests(unittest.TestCase):
    def test_persisted_experiment_budget_does_not_reset_between_splits(self):
        ledger = SpendLedger("2.00")
        token = ledger.reserve("1.50")
        ledger.settle(token, "1.25", {"split": "development"})
        restored = SpendLedger.from_dict(json.loads(json.dumps(ledger.to_dict())))
        self.assertEqual(restored.snapshot()["known_billed_usd"], "1.25")
        with self.assertRaises(EvaluationError):
            restored.reserve("0.76")
        token = restored.reserve("0.75")
        restored.settle(token, "0.75", {"split": "calibration"})
        self.assertEqual(SpendLedger.from_dict(restored.to_dict()).snapshot()["known_billed_usd"], "2.00")

    def test_unresolved_and_interrupted_reservations_survive_reload(self):
        for settle in (False, True):
            ledger = SpendLedger()
            token = ledger.reserve("0.01")
            if settle:
                ledger.settle(token, None)
            restored = SpendLedger.from_dict(ledger.to_dict())
            self.assertTrue(restored.snapshot()["unresolved_charge"])
            with self.assertRaises(EvaluationError):
                restored.reserve("0.01")

    def test_checkpoint_runs_before_request_and_after_settlement(self):
        writes = []
        ledger = SpendLedger(checkpoint=writes.append)
        token = ledger.reserve("0.01")
        self.assertEqual(writes[-1]["pending_reservations"], {token: "0.01"})
        ledger.settle(token, "0.002")
        self.assertEqual(writes[-1]["pending_reservations"], {})
        self.assertEqual(writes[-1]["known_billed_usd"], "0.002")

    def test_malformed_persisted_ledger_is_not_silently_reset(self):
        for state in ({}, {"limit_usd": "3", "known_billed_usd": "0"},
                      {"limit_usd": "2", "known_billed_usd": "nan"}):
            with self.assertRaises(EvaluationError):
                SpendLedger.from_dict(state)

    def test_authorized_limit_reservations_billing_and_exhaustion(self):
        with self.assertRaises(EvaluationError):
            SpendLedger("2.01")
        ledger = SpendLedger("0.20")
        first = ledger.reserve("0.15")
        with self.assertRaises(EvaluationError):
            ledger.reserve("0.06")
        ledger.settle(first, "0.10")
        second = ledger.reserve("0.10")
        ledger.settle(second, "0.10")
        self.assertEqual(ledger.snapshot()["known_billed_usd"], "0.20")
        with self.assertRaises(EvaluationError):
            ledger.reserve("0.01")

    def test_unknown_or_excess_charge_blocks_later_reservations(self):
        for cost in (None, "0.02"):
            ledger = SpendLedger()
            token = ledger.reserve("0.01")
            ledger.settle(token, cost)
            with self.assertRaises(EvaluationError):
                ledger.reserve("0.01")


if __name__ == "__main__":
    unittest.main()
