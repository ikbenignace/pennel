import importlib.util
from pathlib import Path
import sys
import unittest


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
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


if __name__ == "__main__":
    unittest.main()
