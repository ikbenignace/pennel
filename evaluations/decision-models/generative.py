"""Explicit evaluation transports running the core's existing generative prompts."""

import hashlib
import ipaddress
import json
import os
import time
import urllib.parse
from datetime import datetime, timezone

from runners import EvaluationError, HttpTransport, SpendLedger, _decimal


class OpenRouterGenerativeRunner:
    """Chat baseline only; never used as a replacement for typed decision inference."""

    def __init__(self, config, transport=None, ledger=None, environ=None):
        self.config = dict(config)
        self.transport = transport or HttpTransport()
        self.ledger = ledger or SpendLedger(config.get("max_cost_usd", "2.00"))
        self._shared_ledger = ledger is not None
        self.environ = os.environ if environ is None else environ
        self.bridge_call = None
        self._availability = None
        self._headers = {"Content-Type": "application/json"}
        self.identity = {"transport": "openrouter_generative", "provider": None,
                         "requested_model": config.get("model"), "returned_model": None,
                         "configured_provider": config.get("upstream_tag"),
                         "api_surface": "chat_completions"}

    def _get(self, url):
        response = self.transport("GET", url, dict(self._headers), None,
                                  self.config.get("timeout_secs", 60))
        if not isinstance(response, dict) or response.get("status") != 200 or not isinstance(response.get("body"), dict):
            raise EvaluationError("generative_metadata_unavailable")
        return response["body"]

    def preflight(self):
        if self._availability is not None:
            return dict(self._availability)
        try:
            if self.config.get("enabled") is not True:
                raise EvaluationError("generative_not_explicitly_enabled")
            if not self._shared_ledger:
                raise EvaluationError("hosted_generative_requires_shared_spend_ledger")
            if self.config.get("provider") != "openrouter":
                raise EvaluationError("unsupported_generative_provider")
            if self.config.get("base_url", "https://openrouter.ai") != "https://openrouter.ai":
                raise EvaluationError("generative_region_or_domain_unsupported")
            if self.config.get("credential_env", "OPENROUTER_API_KEY") != "OPENROUTER_API_KEY":
                raise EvaluationError("openrouter_requires_explicit_openrouter_api_key")
            model, upstream = self.config.get("model"), self.config.get("upstream_tag")
            if not isinstance(model, str) or not model or any(v in model.lower() for v in ("latest", "router", ":free", "~")):
                raise EvaluationError("generative_model_must_be_explicit")
            if not isinstance(upstream, str) or not upstream:
                raise EvaluationError("generative_upstream_required")
            if self.config.get("retries", 0) != 0 or self.config.get("fallbacks"):
                raise EvaluationError("automatic_retries_or_fallbacks_disabled")
            privacy = self.config.get("privacy", {})
            if privacy.get("processing_region") != "global":
                raise EvaluationError("generative_region_or_domain_unsupported")
            if privacy.get("policy_verified") is not True or not privacy.get("policy_url") or privacy.get("zdr") is not True or privacy.get("data_collection") != "deny":
                raise EvaluationError("generative_global_zdr_policy_required")
            for field, maximum in (("max_completion_tokens", 4096), ("max_input_tokens", 200000), ("billing_overhead_tokens", 4096)):
                value = self.config.get(field, 2048 if field == "billing_overhead_tokens" else None)
                if isinstance(value, bool) or not isinstance(value, int) or not 0 < value <= maximum:
                    raise EvaluationError("generative_invalid_" + field)
            if not isinstance(self.config.get("timeout_secs", 60), (int, float)) or not 0 < self.config.get("timeout_secs", 60) <= 180:
                raise EvaluationError("generative_invalid_timeout")
            key = self.environ.get("OPENROUTER_API_KEY")
            if not key:
                raise EvaluationError("evaluation_credential_unavailable")
            self._headers["Authorization"] = "Bearer " + key
            catalog = self._get("https://openrouter.ai/api/v1/models")
            self._catalog = next((m for m in catalog.get("data", []) if m.get("id") == model or m.get("canonical_slug") == model), None)
            if self._catalog is None or "text" not in self._catalog.get("architecture", {}).get("output_modalities", []):
                raise EvaluationError("configured_generative_model_unavailable")
            canonical = self._catalog.get("canonical_slug")
            if not isinstance(canonical, str) or canonical != self.config.get("expected_canonical_model"):
                raise EvaluationError("generative_catalog_canonical_identity_mismatch")
            self.identity["catalog_model"] = self._catalog["id"]
            self.identity["catalog_canonical_model"] = canonical
            expected = self.config.get("expected_returned_model")
            accepted = {self._catalog["id"], canonical}
            if not isinstance(expected, str) or expected not in accepted:
                raise EvaluationError("generative_returned_identity_pin_required")
            details = self._get("https://openrouter.ai/api/v1/models/" + model + "/endpoints")
            self._endpoint = next((e for e in details.get("data", {}).get("endpoints", []) if e.get("tag") == upstream), None)
            if self._endpoint is None:
                raise EvaluationError("pinned_generative_upstream_unavailable")
            if not self._endpoint.get("provider_name"):
                raise EvaluationError("generative_provider_identity_unavailable")
            zdr = self._get("https://openrouter.ai/api/v1/endpoints/zdr")
            self._zdr_endpoint = next((e for e in zdr.get("data", []) if e.get("model_id") == self._catalog["id"] and e.get("tag") == upstream and e.get("provider_name") == self._endpoint["provider_name"]), None)
            if self._zdr_endpoint is None:
                raise EvaluationError("pinned_generative_upstream_not_in_zdr_catalog")
            supported = self._endpoint.get("supported_parameters", [])
            self._output_limit_parameter = self.config.get("output_limit_parameter") or ("max_completion_tokens" if "max_completion_tokens" in supported else "max_tokens")
            if self._output_limit_parameter not in ("max_tokens", "max_completion_tokens") or supported and self._output_limit_parameter not in supported:
                raise EvaluationError("generative_output_bound_parameter_unsupported")
            self._pricing = self._endpoint.get("pricing")
            if not isinstance(self._pricing, dict):
                raise EvaluationError("generative_endpoint_pricing_required")
            self._input_price = _decimal(self._pricing.get("prompt"))
            self._output_price = _decimal(self._pricing.get("completion"))
            self._inactive_fees = {}
            for name, value in self._pricing.items():
                if name == "web_search":
                    self._inactive_fees[name] = str(_decimal(value))
                    continue
                if name not in ("prompt", "completion", "discount", "input_cache_read", "input_cache_write") and _decimal(value):
                    raise EvaluationError("unsupported_additional_generative_fee")
            if self._input_price + self._output_price <= 0:
                raise EvaluationError("positive_generative_rate_required")
            self._availability = {"status": "ok", "identity": dict(self.identity),
                                  "catalog": self._catalog, "endpoint": self._endpoint,
                                  "zdr_endpoint": self._zdr_endpoint,
                                  "inactive_conditional_fees": self._inactive_fees,
                                  "limitations": ["Hosted server weights, precision, load and memory are not independently established.",
                                                  "Global ZDR route controls do not establish regional residency or account logging settings."]}
        except EvaluationError as error:
            self._availability = {"status": "unavailable", "error": str(error), "identity": dict(self.identity)}
        except Exception:
            self._availability = {"status": "unavailable", "error": "generative_metadata_transport_failure", "identity": dict(self.identity)}
        return dict(self._availability)

    def complete(self, messages, reasoning="off"):
        availability = self.preflight()
        if availability["status"] != "ok":
            return availability
        result = {"status": "unavailable", "identity": dict(self.identity), "text": None,
                  "measurement": "mocked_transport" if getattr(self.transport, "is_mock", True) else "measured_inference",
                  "cost_usd": None, "usage": None, "latency_ms": None, "attempts": [],
                  "limitations": list(availability["limitations"])}
        try:
            if self.bridge_call is None:
                raise EvaluationError("hosted_generative_requires_core_request_body")
            if reasoning not in ("off", "low") or not isinstance(messages, list) or not messages or any(not isinstance(m, dict) or m.get("role") not in ("system", "user", "assistant") or not isinstance(m.get("content"), str) for m in messages):
                raise EvaluationError("invalid_generative_messages")
            if sum(len(m["content"]) for m in messages) > self.config.get("max_input_chars", 100000):
                raise EvaluationError("generative_input_exceeds_configured_bound")
            body = self.bridge_call({"op": "request_body", "provider": "openrouter", "model": self.config["model"],
                                     "messages": messages, "reasoning": reasoning})["body"]
            if not isinstance(body, dict) or body.get("model") != self.config["model"] or body.get("messages") != messages:
                raise EvaluationError("invalid_core_generative_request")
            if set(body) - {"model", "messages", "temperature", "reasoning"}:
                raise EvaluationError("generative_tools_plugins_or_extra_parameters_forbidden")
            if "reasoning" in body and (not isinstance(body["reasoning"], dict) or set(body["reasoning"]) - {"enabled", "effort"}):
                raise EvaluationError("generative_reasoning_tools_forbidden")
            body = dict(body)
            # All existing core prompt/request fields survive; only evaluation routing and output bounds are added.
            body["provider"] = {"only": [self._endpoint["tag"]], "allow_fallbacks": False,
                                "data_collection": "deny", "zdr": True,
                                "max_price": {"prompt": float(self._input_price * 1000000),
                                              "completion": float(self._output_price * 1000000)}}
            body.pop("max_tokens", None)
            body.pop("max_completion_tokens", None)
            body[self._output_limit_parameter] = self.config["max_completion_tokens"]
            bound = len(json.dumps(body, ensure_ascii=False).encode()) + self.config.get("billing_overhead_tokens", 2048)
            context = min(self.config["max_input_tokens"], self._endpoint.get("context_length") or self._catalog.get("context_length") or self.config["max_input_tokens"])
            if bound + self.config["max_completion_tokens"] > context:
                raise EvaluationError("context_bound_exceeded_no_truncation")
            estimate = bound * self._input_price + self.config["max_completion_tokens"] * self._output_price
            if estimate > _decimal(self.config.get("max_request_cost_usd", "0.01")):
                raise EvaluationError("request_reservation_exceeds_configured_cap")
            token = self.ledger.reserve(estimate)
        except EvaluationError as error:
            result["error"] = str(error)
            return result
        started_at, started = datetime.now(timezone.utc).isoformat(), time.perf_counter()
        response, failure = None, None
        try:
            response = self.transport("POST", "https://openrouter.ai/api/v1/chat/completions", dict(self._headers), body, self.config.get("timeout_secs", 60))
        except EvaluationError as error:
            failure = str(error)
        except Exception:
            failure = "generative_transport_failure"
        elapsed = (time.perf_counter() - started) * 1000
        value = response.get("body", {}) if isinstance(response, dict) else {}
        if not isinstance(value, dict):
            value, failure = {}, "invalid_generative_response"
        usage = value.get("usage") if isinstance(value.get("usage"), dict) else {}
        try:
            cost = None if usage.get("cost") is None else _decimal(usage["cost"])
        except EvaluationError:
            cost, failure = None, "invalid_billed_cost"
        actual_model, actual_provider, request_id = value.get("model"), value.get("provider"), value.get("id")
        identity = {**self.identity, "returned_model": actual_model, "provider": actual_provider,
                    "request_id": request_id, "endpoint": "https://openrouter.ai/api/v1/chat/completions",
                    "upstream_request_id": (response.get("headers", {}) if isinstance(response, dict) else {}).get("x-request-id"),
                    "routing": body["provider"], "privacy": self.config["privacy"],
                    "pricing": self._pricing, "pricing_source": "pinned_endpoint",
                    "inactive_conditional_fees": self._inactive_fees,
                    "request_sha256": hashlib.sha256(json.dumps(body, ensure_ascii=False).encode()).hexdigest(),
                    "catalog_sha256": hashlib.sha256(json.dumps(self._catalog, sort_keys=True).encode()).hexdigest(),
                    "endpoint_sha256": hashlib.sha256(json.dumps(self._endpoint, sort_keys=True).encode()).hexdigest(),
                    "zdr_endpoint_sha256": hashlib.sha256(json.dumps(self._zdr_endpoint, sort_keys=True).encode()).hexdigest(),
                    "quantization": self._endpoint.get("quantization"),
                    "tokenizer": self._catalog.get("architecture", {}).get("tokenizer")}
        identity["output_limit_parameter"] = self._output_limit_parameter
        identity["messages_sha256"] = hashlib.sha256(json.dumps(messages, ensure_ascii=False, sort_keys=True).encode()).hexdigest()
        self.ledger.settle(token, cost, {"kind": "generative", "request_id": request_id, "request_sha256": identity["request_sha256"]})
        result.update(status="error", identity=identity, usage=usage or None, latency_ms=elapsed,
                      cost_usd=None if cost is None else str(cost), request_body_source="core",
                      attempts=[{"started_at": started_at, "finished_at": datetime.now(timezone.utc).isoformat(),
                                 "latency_ms": elapsed, "http_status": response.get("status") if isinstance(response, dict) else None,
                                 "identity": identity, "usage": usage or None,
                                 "billed_cost_usd": None if cost is None else str(cost),
                                 "input_token_upper_bound": bound, "output_token_upper_bound": self.config["max_completion_tokens"]}])
        if failure is None and (not isinstance(response, dict) or response.get("status") != 200):
            failure = "generative_http_" + str(response.get("status") if isinstance(response, dict) else "unknown")
        if failure is None and value.get("error"):
            failure = "generative_provider_error"
        if failure is None and actual_model != self.config["expected_returned_model"]:
            failure = "generative_returned_model_mismatch"
        if failure is None and actual_provider != self._endpoint["provider_name"]:
            failure = "generative_returned_provider_mismatch"
        if not request_id:
            result["limitations"].append("request_id_unreported")
        if actual_model == self._catalog["id"] and actual_model != self._catalog["canonical_slug"]:
            result["limitations"].append("returned_alias_does_not_independently_prove_served_weights")
        for field in ("prompt_tokens", "completion_tokens", "total_tokens"):
            count = usage.get(field)
            if count is None:
                result["limitations"].append(field + "_unreported")
            elif isinstance(count, bool) or not isinstance(count, int) or count < 0:
                failure = "invalid_generative_token_usage"
        if cost is None:
            result["limitations"].append("billed_cost_unknown_subsequent_paid_work_halted")
        choices = value.get("choices")
        choice = choices[0] if isinstance(choices, list) and choices and isinstance(choices[0], dict) else {}
        content = choice.get("message", {}).get("content") if isinstance(choice.get("message"), dict) else None
        if failure is None and (not isinstance(content, str) or not content.strip()):
            failure = "generative_missing_text"
        if failure is None and choice.get("finish_reason") != "stop":
            failure = "generative_output_not_complete"
        if failure is None:
            result.update(status="ok", text=content.strip())
        else:
            result["error"] = failure
        if failure in ("generative_returned_model_mismatch", "generative_returned_provider_mismatch"):
            self._availability = {"status": "unavailable", "error": "identity_drift_requires_revalidation", "identity": identity}
        return result


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


def summary_targets(corpus, meeting_id):
    """Gold review metadata only; never included in the generative model request."""
    from grounding import source_hash
    meeting = next(m for m in corpus["meetings"] if m["id"] == meeting_id)
    cases = [d for d in corpus.get("decisions", []) if d.get("meeting_id") == meeting_id and d.get("task") == "events" and not d.get("variant_of")]
    owner = next((d for d in cases if d.get("subtask") == "owner"), None)

    def evidence(spans):
        result = []
        for span in spans:
            text = meeting["transcript"][span["segment_index"]]["text"] if span["kind"] == "transcript" else meeting[span["kind"]]
            result.append({**span, "source_sha256": source_hash(text)})
        return result

    targets = []
    for case in cases:
        if case.get("subtask") != "kind":
            continue
        expected_owner = None
        if case["expected"] == "action":
            if owner is None:
                raise ValueError("summary_action_owner_gold_required")
            expected_owner = {"speaker_id": owner["expected"], "name": owner["question"]["criteria"][owner["expected"]], "evidence": evidence(owner.get("evidence", []))}
        targets.append({"id": case["id"], "expected": case["expected"], "expected_owner": expected_owner, "evidence": evidence(case.get("evidence", []))})
    return targets


def run_baselines(corpus, bridge, runner, split, bridge_call, checkpoint=None):
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
        if task == "summary":
            prediction.setdefault("adjudication", _adjudication("summary", {}, prediction.get("text")))
            prediction["adjudication"]["source_meeting_id"] = meeting["id"]
            prediction["adjudication"]["event_targets"] = summary_targets(corpus, meeting["id"])
        row = {"id": case_id, "task": task, "meeting_id": meeting["id"], "language": meeting["language"], "split": split, "provenance": "current_core_prompt_and_explicit_evaluation_model", **prediction}
        rows.append(row)
        if checkpoint is not None:
            checkpoint(row)

    def stopped():
        return bool(getattr(getattr(runner, "ledger", None), "unresolved", False))

    if "meeting_qa" in tasks:
        for source in bridge.get("queries", []):
            if stopped():
                return rows
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
            if stopped():
                return rows
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
            if stopped():
                return rows
            meeting = meetings.get(source["meeting_id"])
            if meeting is None or limit is not None and len(rows) >= limit:
                continue
            maps = []
            messages = source.get("messages", [])
            if source.get("map_reduce"):
                for part in source.get("map_messages", []):
                    maps.append(runner.complete(part, reasoning="low"))
                    if maps[-1]["status"] != "ok" or stopped():
                        break
                if not maps or any(m["status"] != "ok" for m in maps) or stopped():
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
