import importlib.util
from pathlib import Path
import sys
import unittest


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from runners import SpendLedger
SPEC = importlib.util.spec_from_file_location("generative", ROOT / "generative.py")
generative = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(generative)


class MockTransport:
    is_mock = True

    def __init__(self, responses):
        self.responses = iter(responses)
        self.calls = []

    def __call__(self, method, url, headers, payload, timeout):
        self.calls.append((method, url, headers, payload, timeout))
        return next(self.responses)


def response(body, status=200):
    return {"status": status, "headers": {}, "body": body}


def config(**changes):
    return {"enabled": True, "provider": "lmstudio", "base_url": "http://127.0.0.1:1234/v1", "model": "synthetic-model", **changes}


def answer(text="Synthetic answer. [1]", **changes):
    return response({"model": "synthetic-model", "choices": [{"message": {"content": text}}], "usage": {"prompt_tokens": 20, "completion_tokens": 7, "total_tokens": 27}, **changes})


class GenerativeTests(unittest.TestCase):
    def test_disabled_does_no_io(self):
        transport = MockTransport([])
        runner = generative.GenerativeRunner(config(enabled=False), transport)
        self.assertEqual(runner.preflight()["status"], "unavailable")
        self.assertEqual(runner.complete([])["status"], "unavailable")
        self.assertEqual(transport.calls, [])

    def test_remote_or_credential_bearing_url_is_rejected(self):
        for url in ("http://example.invalid/v1", "https://openrouter.ai/api/v1", "http://secret@127.0.0.1:1234/v1"):
            runner = generative.GenerativeRunner(config(base_url=url), MockTransport([]))
            self.assertEqual(runner.preflight()["status"], "unavailable")

    def test_unavailable_model_is_not_substituted(self):
        transport = MockTransport([response({"data": [{"id": "different-model"}]})])
        runner = generative.GenerativeRunner(config(), transport)
        self.assertEqual(runner.preflight()["status"], "unavailable")
        self.assertEqual(runner.complete([])["status"], "unavailable")
        self.assertEqual(len(transport.calls), 1)

    def test_exact_core_request_body_and_actual_usage_are_preserved(self):
        transport = MockTransport([response({"data": [{"id": "synthetic-model"}]}), answer()])
        runner = generative.GenerativeRunner(config(), transport)
        messages = [{"role": "system", "content": "Existing production prompt"}, {"role": "user", "content": "Synthetic content"}]
        seen = []
        def core_bridge(request):
            seen.append(request)
            return {"body": {"model": request["model"], "messages": request["messages"], "temperature": 0}}
        runner.bridge_call = core_bridge
        result = runner.complete(messages, reasoning="low")
        self.assertEqual(seen[0]["op"], "request_body")
        self.assertEqual(seen[0]["reasoning"], "low")
        self.assertEqual(transport.calls[-1][3]["messages"], messages)
        self.assertEqual(transport.calls[-1][3]["temperature"], 0)
        self.assertEqual(result["usage"]["prompt_tokens"], 20)
        self.assertEqual(result["cost_usd"], "0")
        self.assertEqual(result["identity"]["returned_model"], "synthetic-model")

    def test_model_drift_and_missing_usage_are_explicit(self):
        for provider_response, status in ((answer(model="other-model"), "error"), (answer(usage=None), "ok")):
            runner = generative.GenerativeRunner(config(), MockTransport([response({"data": [{"id": "synthetic-model"}]}), provider_response]))
            result = runner.complete([{"role": "user", "content": "Synthetic"}])
            self.assertEqual(result["status"], status)
            if status == "ok":
                self.assertIsNone(result["usage"])

    def test_ollama_artifact_pin_uses_selected_inventory_digest(self):
        for observed, status in (("sha256:exact", "ok"), ("sha256:other", "unavailable")):
            transport = MockTransport([response({"data": [{"id": "synthetic-model"}]}), response({"models": [{"name": "synthetic-model", "digest": observed, "details": {"quantization_level": "Q4_K_M"}}]})])
            runner = generative.GenerativeRunner(config(provider="ollama", model_digest="sha256:exact"), transport)
            self.assertEqual(runner.preflight()["status"], status)
            self.assertTrue(transport.calls[-1][1].endswith("/api/tags"))
            self.assertEqual(runner.identity["observed_artifact_digest"], observed)

    def test_status_and_exceptions_do_not_trigger_fallback(self):
        for code in (401, 402, 429, 500):
            transport = MockTransport([response({"data": [{"id": "synthetic-model"}]}), response({"error": "private body must not be logged"}, code)])
            result = generative.GenerativeRunner(config(), transport).complete([{"role": "user", "content": "Synthetic"}])
            self.assertEqual(result["status"], "error")
            self.assertEqual(len(transport.calls), 2)
            self.assertNotIn("private", str(result))

    def test_actual_prompt_chains_guard_and_citations_are_requested(self):
        qa_prompt = [{"role": "user", "content": "core QA prompt"}]
        cleanup_prompt = [{"role": "user", "content": "core cleanup prompt"}]
        bridge = {"queries": [{"id": "q", "messages": qa_prompt, "ask": [{"meeting_id": "m", "kind": "transcript", "text": "Synthetic fact"}]}], "cleanup": [{"id": "p", "messages": cleanup_prompt}], "summary_prompts": [{"meeting_id": "m", "map_reduce": True, "map_messages": [[{"role": "user", "content": "map one"}], [{"role": "user", "content": "map two"}]], "messages": [], "notes": "Preserve these notes", "template_id": "algemeen"}]}
        corpus = {"meetings": [{"id": "m", "split": "development", "language": "nl"}], "queries": [{"id": "q", "meeting_id": "m", "split": "development", "language": "nl", "expected_answer": "Synthetic", "evidence": []}], "cleanup_pairs": [{"id": "p", "meeting_id": "m", "split": "development", "language": "nl", "raw": "raw synthetic", "cleaned": "irrelevant unsafe pair target", "expected_safe": False}]}
        calls = []
        def core_bridge(request):
            calls.append(request)
            if request["op"] == "request_body":
                return {"body": {"model": request["model"], "messages": request["messages"], "temperature": 0}}
            if request["op"] == "citations":
                return {"numbers": [1], "citations": [{"number": 1}]}
            if request["op"] == "cleanup_guard":
                return {"accepted": True, "text": request["cleaned"]}
            if request["op"] == "summary_prompt":
                return {"messages": [{"role": "user", "content": request["transcript"]}]}
            self.fail("unexpected core operation")
        transport = MockTransport([response({"data": [{"id": "synthetic-model"}]}), answer(), answer("Cleaned synthetic."), answer("Map result one"), answer("Map result two"), answer("Final summary")])
        runner = generative.GenerativeRunner(config(), transport)
        rows = generative.run_baselines(corpus, bridge, runner, "development", core_bridge)
        self.assertEqual([r["task"] for r in rows], ["meeting_qa", "cleanup", "summary"])
        self.assertEqual(rows[0]["citations"]["numbers"], [1])
        self.assertEqual(rows[0]["adjudication"]["status"], "pending")
        self.assertIsNone(rows[0]["adjudication"]["citation_support"])
        self.assertEqual(rows[1]["guard"]["text"], "Cleaned synthetic.")
        reduce = next(c for c in calls if c["op"] == "summary_prompt")
        self.assertEqual(reduce["transcript"], "Map result one\n\nMap result two")
        self.assertEqual(reduce["notes"], "Preserve these notes")
        self.assertEqual(len(rows[2]["map_results"]), 2)

    def test_map_failure_does_not_bypass_reduce_semantics(self):
        class Runner:
            config = {"tasks": ["summary"]}
            def preflight(self):
                return {"status": "ok"}
            def complete(self, messages, reasoning="off"):
                return {"status": "error", "error": "unavailable"}
        bridge = {"summary_prompts": [{"meeting_id": "m", "map_reduce": True, "map_messages": [[{"role": "user", "content": "map"}]], "messages": []}]}
        corpus = {"meetings": [{"id": "m", "split": "development", "language": "en"}]}
        rows = generative.run_baselines(corpus, bridge, Runner(), "development", lambda request: self.fail("reduce must not run"))
        self.assertEqual(rows[0]["status"], "error")
        self.assertEqual(rows[0]["error"], "summary_map_failed")


class HostedGenerativeTests(unittest.TestCase):
    def config(self, **changes):
        return {"enabled": True, "provider": "openrouter", "model": "synthetic/model-1",
                "expected_returned_model": "synthetic/model-1-20261028", "upstream_tag": "synthetic",
                "expected_canonical_model": "synthetic/model-1-20261028",
                "max_input_tokens": 10000, "max_completion_tokens": 256,
                "privacy": {"processing_region": "global", "policy_verified": True,
                            "policy_url": "https://example.invalid/synthetic-policy", "zdr": True,
                            "data_collection": "deny"}, **changes}

    def metadata(self):
        return [response({"data": [{"id": "synthetic/model-1", "canonical_slug": "synthetic/model-1-20261028",
                                     "architecture": {"output_modalities": ["text"]}, "context_length": 10000}]}),
                response({"data": {"endpoints": [{"tag": "synthetic", "provider_name": "Synthetic",
                                                   "context_length": 10000,
                                                   "pricing": {"prompt": "0.0000004", "completion": "0.0000016"}}]}}),
                response({"data": [{"model_id": "synthetic/model-1", "tag": "synthetic", "provider_name": "Synthetic"}]})]

    def answer(self, **changes):
        return response({"id": "gen-synthetic", "model": "synthetic/model-1-20261028", "provider": "Synthetic",
                         "choices": [{"message": {"content": "Synthetic evidence. [1]"}, "finish_reason": "stop"}],
                         "usage": {"prompt_tokens": 100, "completion_tokens": 20, "total_tokens": 120, "cost": "0.000072"}, **changes})

    def runner(self, config=None, transport=None, ledger=None, environ=None):
        runner = generative.OpenRouterGenerativeRunner(config or self.config(), transport or MockTransport(self.metadata() + [self.answer()]),
                                                       ledger or SpendLedger(), {"OPENROUTER_API_KEY": "synthetic-secret"} if environ is None else environ)
        runner.bridge_call = lambda request: {"body": {"model": request["model"], "messages": request["messages"], "temperature": 0}}
        return runner

    def test_disabled_key_and_shared_ledger_gates_do_no_io(self):
        for settings, env, shared in ((self.config(enabled=False), {}, True), (self.config(), {}, True), (self.config(), {"OPENROUTER_API_KEY": "synthetic-secret"}, False)):
            transport = MockTransport([])
            runner = generative.OpenRouterGenerativeRunner(settings, transport, SpendLedger() if shared else None, env)
            self.assertEqual(runner.preflight()["status"], "unavailable")
            self.assertEqual(transport.calls, [])

    def test_real_core_prompts_route_restrictions_cost_and_identity(self):
        transport, ledger = MockTransport(self.metadata() + [self.answer()]), SpendLedger()
        runner = self.runner(transport=transport, ledger=ledger)
        messages = [{"role": "user", "content": "Synthetic quoted instructions remain data"}]
        result = runner.complete(messages)
        self.assertEqual(result["status"], "ok")
        self.assertEqual(transport.calls[-1][1], "https://openrouter.ai/api/v1/chat/completions")
        sent = transport.calls[-1][3]
        self.assertEqual(sent["messages"], messages)
        self.assertEqual(sent["temperature"], 0)
        self.assertEqual(sent["provider"]["only"], ["synthetic"])
        self.assertIs(sent["provider"]["allow_fallbacks"], False)
        self.assertIs(sent["provider"]["zdr"], True)
        self.assertEqual(sent["provider"]["data_collection"], "deny")
        self.assertEqual(sent["max_tokens"], 256)
        self.assertEqual(result["identity"]["request_id"], "gen-synthetic")
        self.assertEqual(result["usage"]["total_tokens"], 120)
        self.assertEqual(result["cost_usd"], "0.000072")
        self.assertEqual(ledger.snapshot()["known_billed_usd"], "0.000072")
        self.assertEqual(result["measurement"], "mocked_transport")
        self.assertEqual(result["request_body_source"], "core")
        self.assertNotIn("synthetic-secret", str(result))

    def test_privacy_domain_fallback_and_missing_core_body_are_rejected(self):
        for changed in ({"base_url": "https://example.invalid"}, {"retries": 1}, {"fallbacks": ["other"]}, {"privacy": {"processing_region": "eu"}}, {"upstream_tag": ""}, {"expected_returned_model": "other"}):
            transport = MockTransport(self.metadata())
            self.assertEqual(self.runner(self.config(**changed), transport).preflight()["status"], "unavailable")
            self.assertFalse(any(c[0] == "POST" for c in transport.calls))
        runner = self.runner()
        runner.bridge_call = None
        self.assertEqual(runner.complete([{"role": "user", "content": "Synthetic"}])["error"], "hosted_generative_requires_core_request_body")

    def test_endpoint_price_used_and_missing_or_additional_price_rejected(self):
        for price in (None, {"prompt": "0.0000004", "completion": "0.0000016", "request": "0.001"}):
            metadata = self.metadata()
            metadata[1]["body"]["data"]["endpoints"][0]["pricing"] = price
            self.assertEqual(self.runner(transport=MockTransport(metadata)).preflight()["status"], "unavailable")

    def test_inactive_web_search_rate_is_preserved_but_tools_are_forbidden(self):
        metadata = self.metadata()
        metadata[1]["body"]["data"]["endpoints"][0]["pricing"]["web_search"] = "0.01"
        transport = MockTransport(metadata + [self.answer()])
        result = self.runner(transport=transport).complete([{"role": "user", "content": "Synthetic"}])
        self.assertEqual(result["status"], "ok")
        self.assertEqual(result["identity"]["inactive_conditional_fees"], {"web_search": "0.01"})
        self.assertFalse(set(transport.calls[-1][3]) & {"plugins", "tools", "web_search_options", "functions"})
        for parameter in ({"plugins": [{"id": "web"}]}, {"tools": []}, {"web_search_options": {}}, {"reasoning": {"tools": ["web_search"]}}, {"models": ["fallback"]}):
            transport = MockTransport(self.metadata())
            runner = self.runner(transport=transport)
            runner.bridge_call = lambda r: {"body": {"model": r["model"], "messages": r["messages"], **parameter}}
            self.assertEqual(runner.complete([{"role": "user", "content": "Synthetic"}])["status"], "unavailable")
            self.assertEqual(sum(c[0] == "POST" for c in transport.calls), 0)

    def test_zdr_catalog_requires_exact_model_tag_and_provider(self):
        for key, value in (("model_id", "different-model"), ("tag", "different-tag"), ("provider_name", "Different")):
            metadata = self.metadata()
            metadata[-1]["body"]["data"][0][key] = value
            transport = MockTransport(metadata)
            result = self.runner(transport=transport).preflight()
            self.assertEqual(result["error"], "pinned_generative_upstream_not_in_zdr_catalog")
            self.assertEqual(sum(c[0] == "POST" for c in transport.calls), 0)

    def test_dated_request_resolves_alias_catalog_and_zdr_without_claiming_weight_proof(self):
        transport = MockTransport(self.metadata() + [self.answer(model="synthetic/model-1")])
        settings = self.config(model="synthetic/model-1-20261028", expected_returned_model="synthetic/model-1")
        result = self.runner(config=settings, transport=transport).complete([{"role": "user", "content": "Synthetic"}])
        self.assertEqual(result["status"], "ok")
        self.assertEqual(result["identity"]["requested_model"], "synthetic/model-1-20261028")
        self.assertEqual(result["identity"]["returned_model"], "synthetic/model-1")
        self.assertEqual(result["identity"]["catalog_canonical_model"], "synthetic/model-1-20261028")
        self.assertIn("returned_alias_does_not_independently_prove_served_weights", result["limitations"])
        self.assertTrue(transport.calls[1][1].endswith("synthetic/model-1-20261028/endpoints"))
        self.assertEqual(transport.calls[-1][3]["model"], "synthetic/model-1-20261028")

    def test_changed_or_missing_canonical_catalog_identity_stops_before_post(self):
        for canonical in ("synthetic/model-1-other", None):
            metadata = self.metadata()
            metadata[0]["body"]["data"][0]["canonical_slug"] = canonical
            transport = MockTransport(metadata)
            result = self.runner(transport=transport).preflight()
            self.assertEqual(result["error"], "generative_catalog_canonical_identity_mismatch")
            self.assertEqual(sum(c[0] == "POST" for c in transport.calls), 0)

    def test_unknown_charge_halts_following_post_and_never_retries(self):
        transport = MockTransport(self.metadata() + [self.answer(usage={"prompt_tokens": 100})])
        runner = self.runner(transport=transport)
        runner.complete([{"role": "user", "content": "Synthetic"}])
        second = runner.complete([{"role": "user", "content": "Synthetic second"}])
        self.assertEqual(second["error"], "unresolved_charge")
        self.assertEqual(sum(c[0] == "POST" for c in transport.calls), 1)

    def test_identity_drift_blocks_subsequent_calls(self):
        for changed in ({"model": "wrong"}, {"provider": "wrong"}):
            transport = MockTransport(self.metadata() + [self.answer(**changed)])
            runner = self.runner(transport=transport)
            self.assertEqual(runner.complete([{"role": "user", "content": "Synthetic"}])["status"], "error")
            self.assertEqual(runner.complete([{"role": "user", "content": "Synthetic"}])["error"], "identity_drift_requires_revalidation")
            self.assertEqual(sum(c[0] == "POST" for c in transport.calls), 1)

    def test_errors_redirect_timeout_and_truncation_never_change_destination(self):
        for status in (301, 401, 402, 429, 500, 502):
            transport = MockTransport(self.metadata() + [response({"error": {"message": "synthetic-secret private content"}}, status)])
            runner = self.runner(transport=transport)
            result = runner.complete([{"role": "user", "content": "Synthetic"}])
            self.assertEqual(result["status"], "error")
            self.assertTrue(runner.ledger.unresolved)
            self.assertNotIn("private content", str(result))
            self.assertEqual(sum(c[0] == "POST" for c in transport.calls), 1)
        transport = MockTransport(self.metadata() + [self.answer(choices=[{"message": {"content": "partial"}, "finish_reason": "length"}])])
        self.assertEqual(self.runner(transport=transport).complete([{"role": "user", "content": "Synthetic"}])["error"], "generative_output_not_complete")

    def test_budget_and_input_bounds_prevent_post(self):
        transport = MockTransport(self.metadata())
        runner = self.runner(transport=transport, ledger=SpendLedger("0.000001"))
        self.assertEqual(runner.complete([{"role": "user", "content": "Synthetic"}])["error"], "budget_exhausted")
        self.assertEqual(sum(c[0] == "POST" for c in transport.calls), 0)
        runner = self.runner(config=self.config(max_input_tokens=100))
        self.assertEqual(runner.complete([{"role": "user", "content": "Synthetic"}])["error"], "context_bound_exceeded_no_truncation")

    def test_checkpoint_and_unknown_cost_stop_baselines_before_second_post(self):
        transport = MockTransport(self.metadata() + [self.answer(usage={})])
        runner = self.runner(transport=transport)
        runner.config["tasks"] = ["meeting_qa"]
        corpus = {"meetings": [{"id": "m", "language": "en", "split": "heldout"}],
                  "queries": [{"id": q, "meeting_id": "m", "split": "heldout"} for q in ("a", "b")]}
        bridge = {"queries": [{"id": q, "messages": [{"role": "user", "content": "Synthetic"}], "ask": []} for q in ("a", "b")]}
        saved = []
        def core(request):
            if request["op"] == "request_body":
                return {"body": {"model": request["model"], "messages": request["messages"]}}
            return {"citations": [], "numbers": []}
        rows = generative.run_baselines(corpus, bridge, runner, "heldout", core, checkpoint=saved.append)
        self.assertEqual(len(rows), 1)
        self.assertEqual(saved, rows)
        self.assertEqual(sum(c[0] == "POST" for c in transport.calls), 1)


if __name__ == "__main__":
    unittest.main()
