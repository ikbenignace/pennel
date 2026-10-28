"""Fit frozen calibration contexts and score held-out predictions without refitting."""

from collections import Counter, defaultdict
import hashlib

import metrics


MINIMUM_SAMPLES = 10
TRANSIENT_IDENTITY_FIELDS = metrics.VOLATILE_FIELDS | {
    "id", "request_id", "upstream_request_id", "request_sha256", "usage", "cost_usd",
    "billed_cost_usd", "latency_ms", "load_time_ms", "peak_rss_bytes", "attempts",
    "catalog_sha256", "endpoint_sha256",
}
TARGET_PRECISION = {
    "answerability": .98, "claim_support": .98, "events": .95,
    "templates": .95, "speaker_names": .98, "cleanup": 1.0,
}


def _stable_identity(value):
    if isinstance(value, dict):
        return {key: _stable_identity(item) for key, item in value.items() if key not in TRANSIENT_IDENTITY_FIELDS}
    if isinstance(value, list):
        return [_stable_identity(item) for item in value]
    return value


def _digest(value):
    return hashlib.sha256(metrics.canonical_json(value).encode()).hexdigest()


def prediction_context(row):
    """State and request IDs vary by case; model, route and authored question do not."""
    identity = _stable_identity(row.get("identity", {}))
    if not identity:
        raise ValueError("missing_model_identity")
    transport = identity.get("transport", row.get("transport", "local" if identity.get("revision") else None))
    model = identity.get("returned_model", identity.get("model", identity.get("model_id")))
    if not model or not transport:
        raise ValueError("missing_actual_model_identity")
    question = row.get("question")
    if not isinstance(question, dict) and not row.get("prompt_hash"):
        raise ValueError("missing_authored_question")
    labels = row.get("label_order")
    if labels is None and question and question.get("type") == "noul":
        labels = ["false", "true"]
    if not isinstance(labels, list) or any(not isinstance(label, str) for label in labels):
        raise ValueError("missing_ordered_labels")
    criteria = question.get("criteria", {}) if question else {}
    if isinstance(criteria, dict):
        criteria = [[label, criteria.get(label)] for label in labels]
    prompt = {"type": question.get("type"), "instructions": question.get("instructions"), "criteria": criteria} if question else None
    upstream = identity.get("provider") or identity.get("configured_provider") or identity.get("upstream") or identity.get("endpoint")
    if not upstream and transport == "local":
        upstream = "owned_local_worker"
    if not upstream:
        raise ValueError("missing_upstream_identity")
    context = {
        "model": model, "transport": transport, "upstream": upstream,
        "precision": identity.get("precision") or identity.get("quantization") or "unreported",
        "prompt_hash": row.get("prompt_hash") or _digest(prompt), "labels": labels,
        "grouping": row.get("grouping") or identity.get("question_grouping") or "one_question_per_request",
        "identity": identity, "task": row.get("task"), "subtask": row.get("subtask"),
        "language": row.get("language"), "runner_id": row.get("runner_id"),
    }
    metrics._context_hash(context)
    return context


def _case_id(row):
    return row.get("id", row.get("case_id"))


def _manifest_hash(rows, supplied):
    hashes = {row["run_manifest_hash"] for row in rows if row.get("run_manifest_hash")}
    if supplied:
        hashes.add(supplied)
    if len(hashes) > 1:
        raise ValueError("predictions from different frozen run manifests cannot be combined")
    return next(iter(hashes)) if hashes else None


def _score_type(row):
    return row.get("score_type", row.get("raw_score_type", "probabilities"))


def fit_predictions(rows, run_manifest_hash=None, min_samples=MINIMUM_SAMPLES):
    """Ignore other splits entirely; only calibration labels may influence fitting."""
    rows = list(rows)
    if isinstance(min_samples, bool) or not isinstance(min_samples, int) or min_samples < MINIMUM_SAMPLES:
        raise ValueError("minimum calibration sample count cannot weaken the frozen ten-case minimum")
    manifest_hash = _manifest_hash(rows, run_manifest_hash)
    calibration_rows = [row for row in rows if row.get("split") == "calibration"]
    groups = defaultdict(list)
    contexts = {}
    uncalibratable = []
    seen = set()
    for row in calibration_rows:
        if row.get("probabilities") is None or _score_type(row) not in metrics.PROBABILITY_TYPES:
            uncalibratable.append({"case_id": _case_id(row), "runner_id": row.get("runner_id"), "reason": "unsupported_score_type"})
            continue
        try:
            context = prediction_context(row)
        except ValueError as error:
            uncalibratable.append({"case_id": _case_id(row), "runner_id": row.get("runner_id"), "reason": str(error)})
            continue
        key = metrics._context_hash(context)
        metrics.validate_probabilities(row["probabilities"], context["labels"])
        case_id = _case_id(row)
        if case_id is not None:
            unique = (row.get("runner_id"), key, case_id)
            if unique in seen:
                raise ValueError("repeated calibration case IDs cannot inflate sample size")
            seen.add(unique)
        groups[key].append(row)
        contexts[key] = context
    fits = {}
    for key, group in sorted(groups.items()):
        context = contexts[key]
        classes = {row["expected"] for row in group}
        meeting_groups = {row.get("group_id", row.get("meeting_id")) for row in group}
        meeting_groups.discard(None)
        status = "fitted"
        if len(group) < min_samples:
            status = "insufficient_calibration_samples"
        elif len(classes) < 2:
            status = "insufficient_observed_classes"
        elif len(meeting_groups) < 2:
            status = "insufficient_meeting_groups"
        fit = None
        if status == "fitted":
            question_type = group[0].get("question", {}).get("type")
            binary = question_type == "noul" or set(context["labels"]) == {"false", "true"}
            target = TARGET_PRECISION.get(context["task"], .98)
            fit = metrics.fit_calibration(group, context, kind="binary" if binary else "temperature", target_precision=target)
        fits[key] = {
            "context": context, "status": status, "fit": fit, "sample_count": len(group),
            "observed_class_count": len(classes), "meeting_group_count": len(meeting_groups),
            "threshold_target_precision": TARGET_PRECISION.get(context["task"], .98),
        }
    gates = [] if manifest_hash else ["run_manifest_unlinked"]
    if not any(value["status"] == "fitted" for value in fits.values()):
        gates.append("no_fitted_calibration_contexts")
    return {
        "format_version": 1, "run_manifest_hash": manifest_hash,
        "protocol_sha256": _protocol_hash(), "fit_split": "calibration", "evaluation_split": "heldout",
        "minimum_calibration_samples": min_samples, "minimum_observed_classes": 2,
        "minimum_meeting_groups": 2, "calibration_rows": len(calibration_rows),
        "ignored_splits": dict(sorted(Counter(row.get("split", "unknown") for row in rows if row.get("split") != "calibration").items())),
        "fits": fits, "uncalibratable": uncalibratable,
        "gates": gates,
    }


def _protocol_hash():
    from pathlib import Path
    return hashlib.sha256(Path(__file__).with_name("protocol.json").read_bytes()).hexdigest()


def score_predictions(rows, artifact, run_manifest_hash=None):
    """Apply pre-existing fits to held-out cases; unknown contexts remain abstentions."""
    rows = list(rows)
    manifest_hash = _manifest_hash(rows, run_manifest_hash)
    frozen_hash = artifact.get("run_manifest_hash")
    if manifest_hash and frozen_hash and manifest_hash != frozen_hash:
        raise ValueError("calibration and held-out prediction run manifests differ")
    if artifact.get("protocol_sha256") != _protocol_hash():
        raise ValueError("frozen protocol changed; calibration requires revalidation")
    heldout = [row for row in rows if row.get("split") == "heldout"]
    groups = defaultdict(list)
    seen = set()
    for row in heldout:
        group_key = (row.get("runner_id"), row.get("language"), row.get("task"), row.get("subtask") or "")
        case_id = _case_id(row)
        if case_id is not None:
            unique = (*group_key, case_id)
            if unique in seen:
                raise ValueError("repeated held-out case IDs cannot inflate quality metrics")
            seen.add(unique)
        groups[group_key].append(row)
    results = []
    for (runner, language, task, subtask), group in sorted(groups.items()):
        calibrated = []
        statuses = Counter()
        context_counts = Counter()
        for row in group:
            probability = row.get("probabilities")
            status = "calibrated"
            fit = None
            context = None
            key = None
            if probability is None or _score_type(row) not in metrics.PROBABILITY_TYPES:
                status = "unsupported_score_type"
            else:
                try:
                    context = prediction_context(row)
                    key = metrics._context_hash(context)
                    matched = artifact["fits"].get(key)
                    if matched is None:
                        status = "unfitted_context"
                    elif matched["status"] != "fitted":
                        status = matched["status"]
                    else:
                        fit = matched["fit"]
                except ValueError:
                    status = "invalid_context"
            if key:
                context_counts[key] += 1
            if fit is not None:
                probability = metrics.apply_calibration(probability, fit, context)
                selected = max(sorted(probability), key=probability.get)
                threshold = fit["threshold"]["threshold"]
                accepted = threshold is not None and probability[selected] >= threshold
                calibrated.append({**row, "prediction": selected, "probabilities": probability,
                                   "score_type": "calibrated_probabilities", "accepted": accepted})
            else:
                calibrated.append({**row, "prediction": None, "probabilities": None, "accepted": False})
            statuses[status] += 1
        uncalibrated_count = len(group) - statuses["calibrated"]
        results.append({
            "runner_id": runner, "language": language, "task": task, "subtask": subtask or None,
            "raw": metrics.classification_metrics(group), "calibrated": metrics.classification_metrics(calibrated),
            "raw_selective_curve": metrics.selective_curve(group),
            "calibrated_selective_curve": metrics.selective_curve(calibrated),
            "calibrated_count": statuses["calibrated"], "uncalibrated_count": uncalibrated_count,
            "statuses": dict(sorted(statuses.items())), "context_counts": dict(sorted(context_counts.items())),
            "gates": ["uncalibrated_heldout_cases"] if uncalibrated_count else [],
        })
    manifest_verified = bool(manifest_hash and frozen_hash and manifest_hash == frozen_hash)
    gates = [] if manifest_verified else ["run_manifest_unlinked"]
    if not heldout:
        gates.append("no_heldout_predictions")
    return {
        "format_version": 1, "run_manifest_hash": frozen_hash, "manifest_verified": manifest_verified,
        "calibration_artifact_sha256": _digest(artifact), "evaluation_split": "heldout",
        "heldout_rows": len(heldout),
        "ignored_splits": dict(sorted(Counter(row.get("split", "unknown") for row in rows if row.get("split") != "heldout").items())),
        "groups": results, "gates": gates,
        "interpretation": "Raw and calibrated metrics use the full task/language denominator. Missing, failed and unmatched contexts remain abstentions; probability metrics disclose the smaller probability-bearing count.",
    }
