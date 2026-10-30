"""CPU inference process. Optional packages are imported only after artifact verification."""

import argparse
import contextlib
import hashlib
import importlib
import importlib.metadata
import io
import json
import math
import os
from pathlib import Path
import platform
import sys
import time
import types

try:
    import resource
except ImportError:
    resource = None


DIRECTORY = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(DIRECTORY))
from local_models import file_hash, model_status, registry


class Unavailable(ValueError):
    pass


def peak_rss_bytes():
    if resource is None:
        return None
    value = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    return int(value if sys.platform == "darwin" else value * 1024)


def ordered_labels(question):
    criteria = question.get("criteria")
    if question["type"] == "noul":
        criteria = criteria or {"false": "The condition does not hold.", "true": "The condition holds."}
        if set(criteria) != {"false", "true"}:
            raise Unavailable("invalid_binary_criteria")
    elif question["type"] == "score" and isinstance(criteria, list):
        criteria = {str(index): description for index, description in enumerate(criteria)}
    if not isinstance(criteria, dict) or not criteria or len(criteria) > 25:
        raise Unavailable("unsupported_label_set")
    if any(not isinstance(key, str) or not isinstance(value, str) for key, value in criteria.items()):
        raise Unavailable("invalid_label_set")
    return list(criteria), [f"{key} — {description}" for key, description in criteria.items()]


def softmax(values):
    if not values or any(not math.isfinite(float(value)) for value in values):
        raise Unavailable("invalid_model_scores")
    maximum = max(values)
    exponents = [math.exp(value - maximum) for value in values]
    total = sum(exponents)
    return [value / total for value in exponents]


def checked_ranking(passages, scores):
    ids = [passage["id"] for passage in passages]
    if len(ids) != len(set(ids)) or len(scores) != len(ids):
        raise Unavailable("invalid_ranking_ids_or_scores")
    if any(not math.isfinite(float(score)) for score in scores):
        raise Unavailable("invalid_ranking_scores")
    order = sorted(range(len(ids)), key=lambda index: (-float(scores[index]), index))
    return {"ranked_ids": [ids[index] for index in order],
            "scores": {key: float(score) for key, score in zip(ids, scores)}}


class Backend:
    def __init__(self, model_id, path):
        self.model_id = model_id
        self.path = Path(path)
        self.spec = registry()[model_id]
        self.limitations = self.spec.get("limitations", []).copy()
        state = model_status(model_id, self.path.parents[1])
        if state["status"] != "available" or Path(state["path"]) != self.path.resolve():
            raise Unavailable("artifact_verification_failed")
        self.identity = {**state["identity"], "device": "cpu", "threads": 4,
                         "model": self.spec["repo"], "model_id": model_id,
                         "actual_model": self.spec["repo"], "requested_model": self.spec["repo"],
                         "transport": "local", "upstream": "owned_local_worker",
                         "python": platform.python_version(), "packages": {}}
        self.backend = self.spec["backend"]
        self.limit = self.spec["max_input_tokens"]
        if self.backend == "gliclass_onnx":
            self._load_onnx()
            return
        import torch
        torch.set_num_threads(4)
        torch.set_num_interop_threads(1)
        torch.manual_seed(53)
        self.torch = torch
        if self.backend == "gliclass":
            from gliclass import GLiClassModel, ZeroShotClassificationPipeline
            from transformers import AutoTokenizer
            self.tokenizer = AutoTokenizer.from_pretrained(str(path), local_files_only=True)
            self.model = GLiClassModel.from_pretrained(str(path), local_files_only=True).float().eval()
            self.pipeline = ZeroShotClassificationPipeline(
                self.model, self.tokenizer, max_length=self.limit,
                classification_type="single-label", device="cpu", progress_bar=False)
        elif self.backend == "gliner":
            from gliner2 import AutoExtractor
            self.model = AutoExtractor.from_pretrained(str(path), local_files_only=True,
                                                      map_location="cpu").float().eval()
            self.tokenizer = getattr(self.model, "tokenizer", None)
            if self.tokenizer is None:
                processor = getattr(self.model, "processor", None)
                self.tokenizer = getattr(processor, "tokenizer", None)
            if self.tokenizer is None:
                raise Unavailable("tokenizer_preflight_unavailable")
        elif self.backend == "gte":
            from transformers import AutoTokenizer
            package_name = "_pennel_verified_gte"
            package = types.ModuleType(package_name)
            package.__path__ = [str(self.path / "custom_code")]
            sys.modules[package_name] = package
            configuration = importlib.import_module(package_name + ".configuration")
            modeling = importlib.import_module(package_name + ".modeling")
            config = configuration.NewConfig.from_json_file(str(path / "config.json"))
            self.model = modeling.NewForSequenceClassification.from_pretrained(
                str(path), config=config, torch_dtype=torch.float32, local_files_only=True).eval()
            self.tokenizer = AutoTokenizer.from_pretrained(str(path), local_files_only=True,
                                                          trust_remote_code=False)
            self.identity["custom_code_revision"] = self.spec["code_revision"]
        elif self.backend == "laya":
            import laya
            self.model = laya.load(str(path), device="cpu", backend="eager", compile=False)
            self.tokenizer = getattr(self.model, "tok", None)
            if self.tokenizer is None:
                raise Unavailable("tokenizer_preflight_unavailable")
            self.limitations.append("Owner probabilities are not task-calibrated; evaluate raw and fitted calibration separately.")
        else:
            raise Unavailable("unknown_backend")
        names = {"gliclass": ["gliclass", "torch", "transformers"],
                 "gliner": ["gliner2", "torch", "transformers"],
                 "gte": ["torch", "transformers"], "laya": ["laya", "torch", "transformers"]}
        self.identity["packages"] = {name: importlib.metadata.version(name) for name in names[self.backend]}

    def _load_onnx(self):
        contract_path = self.path / "onnx-contract.json"
        if not contract_path.is_file():
            raise Unavailable("onnx_graph_and_reference_parity_unverified")
        contract = json.loads(contract_path.read_text(encoding="utf-8"))
        graph = self.path / "onnx" / "model_q8.onnx"
        tokenizer_file = self.path / "onnx" / "tokenizer.json"
        if (contract.get("graph_sha256") != file_hash(graph)
                or contract.get("tokenizer_sha256") != file_hash(tokenizer_file)
                or not contract.get("reference_parity", {}).get("passed")
                or not contract.get("reference_parity", {}).get("reference_revision")
                or contract.get("preprocessing") != "gliclass-0.1.21-uni-encoder"):
            raise Unavailable("onnx_graph_and_reference_parity_unverified")
        import numpy as np
        import onnxruntime as ort
        from tokenizers import Tokenizer
        options = ort.SessionOptions()
        options.intra_op_num_threads = 4
        options.inter_op_num_threads = 1
        self.session = ort.InferenceSession(str(graph), sess_options=options,
                                            providers=["CPUExecutionProvider"])
        inputs = {item.name: item.type for item in self.session.get_inputs()}
        if inputs != contract.get("inputs") or inputs != {
                "input_ids": "tensor(int64)", "attention_mask": "tensor(int64)"}:
            raise Unavailable("unsupported_onnx_graph_inputs")
        outputs = [item.name for item in self.session.get_outputs()]
        if contract.get("logits_output") not in outputs:
            raise Unavailable("unsupported_onnx_graph_outputs")
        self.np = np
        self.tokenizer = Tokenizer.from_file(str(tokenizer_file))
        config = json.loads((self.path / "config.json").read_text(encoding="utf-8"))
        self.label_token = self.tokenizer.id_to_token(config["class_token_index"])
        self.sep_token = self.tokenizer.id_to_token(config["text_token_index"])
        self.logits_output = contract["logits_output"]
        self.identity["onnx_contract_sha256"] = file_hash(contract_path)
        self.identity["packages"] = {name: importlib.metadata.version(name)
                                    for name in ["onnxruntime", "tokenizers", "numpy"]}

    def _check_tokens(self, state, prompt, labels, question=None):
        if self.backend == "gliclass":
            prepared = self.pipeline.pipe.prepare_input(state, labels, prompt=prompt)
            count = len(self.tokenizer.encode(prepared, add_special_tokens=True))
        elif self.backend == "gliclass_onnx":
            prepared = "".join(self.label_token + label for label in labels) + self.sep_token + prompt + state
            encoded = self.tokenizer.encode(prepared)
            count = len(encoded.ids)
        elif self.backend == "gliner":
            ids, _ = ordered_labels(question)
            schema = self.model._classification_schema({"decision": {
                "labels": dict(zip(ids, labels)), "prompt": prompt}})
            prepared = self.model.processor.transform_record(state, schema, max_len=None)
            count = len(prepared.input_ids)
        elif self.backend == "laya":
            from laya.common import build_head, encode_text, render_options
            internal = self.model._to_internal(question)
            options = render_options(internal)
            option_lengths = [len(encode_text(self.tokenizer,
                " " + option.replace(self.tokenizer.mask_token, " "), add_special_tokens=False)["input_ids"])
                for option in options]
            if any(length > 48 for length in option_lengths):
                raise Unavailable("question_exceeds_model_option_token_limit")
            head_limit = self.model.cfg.get("head_max_len", 192)
            _, _, stats = build_head(self.tokenizer, internal, head_limit)
            instruction = f"{internal['t']} question: {internal['ins'].replace(self.tokenizer.mask_token, ' ')}"
            instruction_length = len(encode_text(self.tokenizer, instruction, add_special_tokens=False)["input_ids"])
            instruction_room = max(8, head_limit - sum(length + 1 for length in option_lengths))
            if stats["tokens_per_option"] is not None or instruction_length > instruction_room:
                raise Unavailable("question_exceeds_model_head_token_limit")
            prepared = self.model._encode_state(state, ["decision"], {"decision": internal},
                                                 max_len=self.limit, head_max_len=head_limit)[0]
            if prepared["state_stats"]["truncated"]:
                raise Unavailable("input_exceeds_pinned_context_budget")
            count = len(prepared["ids"])
        else:
            raise Unavailable("exact_token_preflight_unavailable")
        if count > self.limit:
            raise Unavailable("input_exceeds_pinned_context_budget")
        return count

    def predict(self, case):
        if case["task"] == "ranking":
            return self.rank(case)
        if self.backend == "gte":
            raise Unavailable("reranker_does_not_support_typed_decisions")
        question = case["question"]
        ids, labels = ordered_labels(question)
        state = case["state"]
        if not isinstance(state, str):
            state = json.dumps(state, ensure_ascii=False, separators=(",", ":"))
        prompt = question.get("instructions", "")
        token_count = self._check_tokens(state, prompt, labels, question)
        if self.backend == "gliclass":
            inputs = self.pipeline.pipe.prepare_inputs([state], labels, same_labels=True, prompt=prompt)
            with self.torch.inference_mode():
                output = self.model(**inputs, max_num_classes=len(labels))
            logits = output.logits[0][:len(labels)].float().tolist()
            probabilities = dict(zip(ids, softmax(logits)))
            result = {"prediction": max(probabilities, key=probabilities.get), "probabilities": probabilities,
                      "raw_scores": dict(zip(ids, logits)), "score_type": "categorical_softmax"}
        elif self.backend == "gliclass_onnx":
            prepared = "".join(self.label_token + label for label in labels) + self.sep_token + prompt + state
            encoded = self.tokenizer.encode(prepared)
            inputs = {"input_ids": self.np.asarray([encoded.ids], dtype=self.np.int64),
                      "attention_mask": self.np.asarray([encoded.attention_mask], dtype=self.np.int64)}
            logits = self.session.run([self.logits_output], inputs)[0].reshape(-1).tolist()
            if len(logits) != len(labels):
                raise Unavailable("onnx_output_label_count_mismatch")
            probabilities = dict(zip(ids, softmax(logits)))
            result = {"prediction": max(probabilities, key=probabilities.get), "probabilities": probabilities,
                      "raw_scores": dict(zip(ids, logits)), "score_type": "categorical_softmax"}
        elif self.backend == "gliner":
            criteria = dict(zip(ids, labels))
            with self.torch.inference_mode():
                output = self.model.classify_text(state, {"decision": {"labels": criteria, "prompt": prompt}},
                                                   include_confidence=True, max_len=self.limit)
            answer = output["decision"]
            if isinstance(answer, dict):
                prediction = answer.get("label")
                confidence = answer.get("confidence")
            else:
                prediction = answer
                confidence = None
            if prediction not in ids:
                raise Unavailable("model_returned_unknown_label")
            result = {"prediction": prediction, "probabilities": None, "confidence": confidence,
                      "score_type": "selected_label_only"}
        elif self.backend == "laya":
            output = self.model.predict(state, {"decision": question}, max_len=self.limit)
            answer = output["answers"]["decision"]
            if question["type"] == "noul":
                yes = float(answer["noul"])
                probabilities = {"false": 1 - yes, "true": yes}
            else:
                probabilities = {key: float(value) for key, value in answer["probabilities"].items()}
            if (set(probabilities) != set(ids) or abs(sum(probabilities.values()) - 1) > 1e-4
                    or any(not math.isfinite(value) or not 0 <= value <= 1
                           for value in probabilities.values())):
                raise Unavailable("invalid_probability_distribution")
            result = {"prediction": max(probabilities, key=probabilities.get), "probabilities": probabilities,
                      "score_type": "uncalibrated_typed_probability"}
        else:
            raise Unavailable("unsupported_decision_backend")
        result["input_tokens"] = token_count
        return result

    def rank(self, case):
        passages = case["passages"]
        query = case["query"]
        if self.backend == "gte":
            scores = []
            for passage in passages:
                encoded = self.tokenizer(query, passage["text"], truncation=False, return_tensors="pt")
                if encoded["input_ids"].shape[1] > self.limit:
                    raise Unavailable("input_exceeds_pinned_context_budget")
                with self.torch.inference_mode():
                    value = self.model(**encoded, return_dict=True).logits.view(-1).float()
                if value.numel() != 1:
                    raise Unavailable("invalid_reranker_output")
                scores.append(value.item())
            return {**checked_ranking(passages, scores), "score_type": "relevance_logit",
                    "probabilities": None}
        if self.backend in {"gliclass", "gliclass_onnx"}:
            scores = []
            for passage in passages:
                result = self.predict({"task": "relevance", "state": passage["text"], "question": {
                    "type": "noul", "instructions": "Does this passage provide relevant evidence for the query? " + query,
                    "criteria": {"false": "The passage is irrelevant.", "true": "The passage is relevant."}}})
                scores.append(result["probabilities"]["true"])
            return {**checked_ranking(passages, scores), "score_type": "decision_relevance_probability",
                    "probabilities": None, "limitations": ["Pairwise decision scorer; not the task-specific GTE reranker."]}
        raise Unavailable("ranking_capability_not_implemented_for_this_adapter")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model", required=True)
    parser.add_argument("--model-dir", type=Path, required=True)
    args = parser.parse_args()
    output = sys.stdout
    began = time.perf_counter()
    backend = None
    failure = None
    try:
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            backend = Backend(args.model, args.model_dir.resolve())
        startup = {"status": "ready", "identity": backend.identity, "limitations": backend.limitations,
                   "load_ms": (time.perf_counter() - began) * 1000, "peak_rss_bytes": peak_rss_bytes()}
    except (ImportError, importlib.metadata.PackageNotFoundError):
        failure = "optional_package_missing"
    except Unavailable as exc:
        failure = str(exc)
    except Exception:
        failure = "model_runtime_initialization_failed"
    if failure:
        startup = {"status": "unavailable", "reason": failure, "identity": {"model": args.model},
                   "load_ms": (time.perf_counter() - began) * 1000}
    output.write(json.dumps(startup, allow_nan=False) + "\n")
    output.flush()
    while True:
        line = sys.stdin.readline(4 * 1024 * 1024)
        if not line:
            break
        if not line.endswith("\n"):
            break
        case = {}
        before = time.perf_counter()
        try:
            case = json.loads(line)
            if failure:
                raise Unavailable(failure)
            with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                result = backend.predict(case)
            result.update({"case_id": case["id"], "status": "ok", "identity": backend.identity,
                           "latency_ms": (time.perf_counter() - before) * 1000,
                           "peak_rss_bytes": peak_rss_bytes(), "load_ms": startup["load_ms"]})
        except Unavailable as exc:
            result = {"case_id": case.get("id"), "status": "unavailable", "reason": str(exc)}
        except Exception:
            result = {"case_id": case.get("id"), "status": "error", "reason": "model_inference_failed"}
        output.write(json.dumps(result, ensure_ascii=False, allow_nan=False) + "\n")
        output.flush()


if __name__ == "__main__":
    main()
