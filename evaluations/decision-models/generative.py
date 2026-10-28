"""Optional local endpoints running the core's existing generative prompts."""

import ipaddress
import time
import urllib.parse

from runners import HttpTransport


class GenerativeRunner:
    def __init__(self, config, transport=None):
        self.config = dict(config)
        self.transport = transport or HttpTransport()
        self.bridge_call = None
        self._availability = None
        self.identity = {"transport": "local_generative_endpoint", "provider": config.get("provider", "ollama"), "requested_model": config.get("model"), "returned_model": None, "configured_artifact_digest": config.get("model_digest"), "observed_artifact_digest": None}

    def preflight(self):
        if self._availability is not None:
            return self._availability
        failure = None
        base = self.config.get("base_url", "")
        parsed = urllib.parse.urlparse(base)
        try:
            local = parsed.hostname == "localhost" or ipaddress.ip_address(parsed.hostname or "").is_loopback
        except ValueError:
            local = False
        if self.config.get("enabled") is not True:
            failure = "generative_not_explicitly_enabled"
        elif parsed.scheme not in ("http", "https") or not local or parsed.username or parsed.password or parsed.query or parsed.fragment:
            failure = "generative_endpoint_must_be_explicit_loopback"
        elif self.identity["provider"] not in ("ollama", "lmstudio"):
            failure = "unsupported_generative_provider"
        elif not isinstance(self.config.get("model"), str) or not self.config["model"].strip():
            failure = "generative_model_required"
        if failure is None:
            try:
                response = self.transport("GET", base.rstrip("/") + "/models", {}, None, self.config.get("timeout_secs", 60))
                models = response.get("body", {}).get("data", [])
                selected = next((m for m in models if isinstance(m, dict) and m.get("id") == self.config["model"]), None)
                if response.get("status") != 200 or selected is None:
                    failure = "configured_generative_model_unavailable"
                else:
                    self.identity["observed_artifact_digest"] = selected.get("digest")
                    if self.identity["provider"] == "ollama":
                        tags = self.transport("GET", f"{parsed.scheme}://{parsed.netloc}/api/tags", {}, None, self.config.get("timeout_secs", 60))
                        if tags.get("status") == 200:
                            selected_tag = next((m for m in tags.get("body", {}).get("models", []) if m.get("name") == self.config["model"] or m.get("model") == self.config["model"]), {})
                            self.identity["observed_artifact_digest"] = selected_tag.get("digest")
                            self.identity["observed_artifact_details"] = selected_tag.get("details")
                            self.identity["artifact_identity_source"] = "ollama_tags_inventory"
                    if self.config.get("model_digest") and self.identity["observed_artifact_digest"] != self.config["model_digest"]:
                        failure = "generative_artifact_identity_unverified"
            except Exception:
                failure = "generative_endpoint_unavailable"
        self._availability = {"status": "unavailable" if failure else "ok", "identity": dict(self.identity), "limitations": ["Evaluation-selected model with current core prompts; this is not the user's saved provider selection.", "Loopback endpoint usage tokens are local inference counts, not billed hosted tokens."]}
        if self.identity["observed_artifact_digest"] is None:
            self._availability["limitations"].append("Endpoint inventory does not establish an immutable artifact revision.")
        if failure:
            self._availability["error"] = failure
        return self._availability

    def complete(self, messages, reasoning="off"):
        availability = self.preflight()
        if availability["status"] != "ok":
            return dict(availability)
        if reasoning not in ("off", "low"):
            return {"status": "error", "error": "invalid_reasoning_mode"}
        if not isinstance(messages, list) or not messages or any(not isinstance(m, dict) or m.get("role") not in ("system", "user", "assistant") or not isinstance(m.get("content"), str) for m in messages):
            return {"status": "error", "error": "invalid_generative_messages"}
        if sum(len(m["content"]) for m in messages) > self.config.get("max_input_chars", 100_000):
            return {"status": "unavailable", "error": "generative_input_exceeds_configured_bound"}
        request = {"op": "request_body", "provider": "ollama" if self.identity["provider"] == "ollama" else "custom", "model": self.config["model"], "messages": messages, "reasoning": reasoning}
        if self.bridge_call is not None:
            body = self.bridge_call(request)["body"]
        else:
            body = {"model": self.config["model"], "messages": messages, "temperature": 0}
        started = time.perf_counter()
        result = {"status": "error", "text": None, "identity": dict(self.identity), "usage": None, "cost_usd": "0", "cost_semantics": "No paid external request is made by this loopback-only transport.", "limitations": list(availability["limitations"])}
        try:
            response = self.transport("POST", self.config["base_url"].rstrip("/") + "/chat/completions", {"Content-Type": "application/json"}, body, self.config.get("timeout_secs", 60))
            if response.get("status") != 200:
                result["error"] = "generative_http_" + str(response.get("status", "unknown"))
            else:
                value = response["body"]
                result["identity"]["returned_model"] = value.get("model")
                result["usage"] = value.get("usage")
                content = value.get("choices", [{}])[0].get("message", {}).get("content")
                if value.get("model") != self.config["model"]:
                    result["error"] = "generative_returned_model_mismatch"
                elif not isinstance(content, str) or not content.strip():
                    result["error"] = "generative_missing_text"
                elif value["choices"][0].get("finish_reason") == "length":
                    result["error"] = "generative_output_truncated"
                else:
                    result.update(status="ok", text=content.strip())
        except Exception:
            result["error"] = "generative_transport_or_response_failure"
        result["latency_ms"] = (time.perf_counter() - started) * 1000
        result["request_body_source"] = "core" if self.bridge_call else "equivalent_local_temperature_zero"
        return result


def _adjudication(kind, case, generated, sources=None):
    return {"status": "pending", "rubric": kind + "-evidence-v1", "factual_support": None, "citation_support": None, "generated_text": generated, "expected_answer": case.get("expected_answer"), "gold_evidence": case.get("evidence", []), "sources": sources or [], "instruction": "Review every substantive claim against exact source evidence, final retractions, negation, owners and language. Valid citation syntax does not prove factual support."}


def run_baselines(corpus, bridge, runner, split, bridge_call):
    runner.bridge_call = bridge_call
    availability = runner.preflight()
    if availability["status"] != "ok":
        return [{"task": "generative_availability", "split": split, **availability}]
    meetings = {m["id"]: m for m in corpus["meetings"] if m["split"] == split}
    queries = {q["id"]: q for q in corpus.get("queries", []) if q["split"] == split}
    pairs = {p["id"]: p for p in corpus.get("cleanup_pairs", []) if p["split"] == split}
    tasks = runner.config.get("tasks", ["meeting_qa", "cleanup", "summary"])
    limit = runner.config.get("max_cases")
    rows = []

    def append(task, case_id, meeting, prediction):
        rows.append({"id": case_id, "task": task, "meeting_id": meeting["id"], "language": meeting["language"], "split": split, "provenance": "current_core_prompt_and_explicit_evaluation_model", **prediction})

    if "meeting_qa" in tasks:
        for source in bridge.get("queries", []):
            case = queries.get(source["id"])
            if case is None or limit is not None and len(rows) >= limit:
                continue
            meeting = meetings[case.get("origin_meeting_id") or case["meeting_id"]]
            prediction = runner.complete(source["messages"], reasoning="low")
            if prediction["status"] == "ok":
                prediction["citations"] = bridge_call({"op": "citations", "id": case["id"], "answer": prediction["text"], "sources": source.get("ask", [])})
                prediction["adjudication"] = _adjudication("answer", case, prediction["text"], source.get("ask", []))
            append("meeting_qa", case["id"], meeting, prediction)
    if "cleanup" in tasks:
        seen = set()
        for source in bridge.get("cleanup", []):
            case = pairs.get(source["id"])
            if case is None or limit is not None and len(rows) >= limit:
                continue
            key = (case["meeting_id"], case["raw"])
            if key in seen:
                continue
            seen.add(key)
            prediction = runner.complete(source["messages"], reasoning="off")
            if prediction["status"] == "ok":
                prediction["guard"] = bridge_call({"op": "cleanup_guard", "raw": case["raw"], "cleaned": prediction["text"]})
                prediction["adjudication"] = _adjudication("cleanup-fidelity", case, prediction["text"])
                prediction["adjudication"]["raw"] = case["raw"]
            append("cleanup", case["id"], meetings[case["meeting_id"]], prediction)
    if "summary" in tasks:
        for source in bridge.get("summary_prompts", []):
            meeting = meetings.get(source["meeting_id"])
            if meeting is None or limit is not None and len(rows) >= limit:
                continue
            maps = []
            messages = source.get("messages", [])
            if source.get("map_reduce"):
                for part in source.get("map_messages", []):
                    maps.append(runner.complete(part, reasoning="low"))
                    if maps[-1]["status"] != "ok":
                        break
                if not maps or any(m["status"] != "ok" for m in maps):
                    append("summary", meeting["id"] + "-summary", meeting, {"status": "error", "error": "summary_map_failed", "map_results": maps})
                    continue
                messages = bridge_call({"op": "summary_prompt", "template_id": source.get("template_id", "algemeen"), "notes": source.get("notes", ""), "transcript": "\n\n".join(m["text"] for m in maps)})["messages"]
            prediction = runner.complete(messages, reasoning="low")
            prediction["map_results"] = maps
            prediction["map_reduce"] = bool(source.get("map_reduce"))
            if prediction["status"] == "ok":
                prediction["adjudication"] = _adjudication("summary", {}, prediction["text"])
                prediction["adjudication"]["source_meeting_id"] = meeting["id"]
            append("summary", meeting["id"] + "-summary", meeting, prediction)
    return rows
