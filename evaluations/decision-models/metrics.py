"""Offline evaluation metrics; score meaning and calibration provenance stay explicit."""

from collections import defaultdict
import hashlib
import json
import math
import random


PROBABILITY_TYPES = {
    "probabilities", "probability", "categorical_probability", "categorical_probabilities",
    "binary_probability", "binary_probabilities", "ordinal_probability", "ordinal_probabilities",
    "noul", "choice", "score",
    "calibrated_probability", "calibrated_probabilities",
    "categorical_softmax", "uncalibrated_typed_probability",
}
CONTEXT_FIELDS = {"model", "transport", "upstream", "precision", "prompt_hash", "labels", "grouping"}
VOLATILE_FIELDS = {"generated_at", "timestamp", "created_at", "started_at", "finished_at"}
EPSILON = 1e-12


def canonical_json(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False)


def _semantic(value):
    if isinstance(value, dict):
        return {key: _semantic(item) for key, item in value.items() if key not in VOLATILE_FIELDS}
    if isinstance(value, (list, tuple)):
        return [_semantic(item) for item in value]
    return value


def semantic_digest(value):
    """Clock timestamps do not change the digest; measurements and ordered lists do."""
    return hashlib.sha256(canonical_json(_semantic(value)).encode()).hexdigest()


def _number(value, name):
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value):
        raise ValueError(f"{name} must be a finite number")
    return float(value)


def _ratio(numerator, denominator):
    return numerator / denominator if denominator else None


def _mean(values):
    return sum(values) / len(values) if values else None


def _harmonic(precision, recall):
    if precision is None or recall is None:
        return None
    return 2 * precision * recall / (precision + recall) if precision + recall else 0.0


def percentile(values, p):
    """Linear (Hyndman-Fan type 7) percentile, with p in [0,100]."""
    p = _number(p, "percentile")
    if not 0 <= p <= 100:
        raise ValueError("percentile must be in [0,100]")
    ordered = sorted(_number(value, "sample") for value in values)
    if not ordered:
        return None
    position = (len(ordered) - 1) * p / 100
    lower = math.floor(position)
    upper = math.ceil(position)
    return ordered[lower] + (ordered[upper] - ordered[lower]) * (position - lower)


def timing_metrics(values):
    values = [_number(value, "latency") for value in values]
    if any(value < 0 for value in values):
        raise ValueError("latencies cannot be negative")
    return {
        "n": len(values), "p50_ms": percentile(values, 50), "p95_ms": percentile(values, 95),
        "min_ms": min(values) if values else None, "max_ms": max(values) if values else None,
        "percentile_method": "linear_type_7",
    }


def ranking_metrics(ranked_ids, relevance_by_id, candidate_ids=None, k=5):
    """Graded nDCG and binary evidence recall; unanswerable queries are undefined."""
    if isinstance(k, bool) or not isinstance(k, int) or k < 1:
        raise ValueError("k must be a positive integer")
    ranked_ids = list(ranked_ids)
    if len(set(ranked_ids)) != len(ranked_ids):
        raise ValueError("ranked passage IDs must be unique")
    grades = {key: _number(value, "relevance grade") for key, value in relevance_by_id.items()}
    if any(value < 0 or value > 3 for value in grades.values()):
        raise ValueError("relevance grades must be in [0,3]")
    candidates = list(candidate_ids) if candidate_ids is not None else list(ranked_ids)
    if len(set(candidates)) != len(candidates):
        raise ValueError("candidate passage IDs must be unique")
    if not set(ranked_ids).issubset(candidates):
        raise ValueError("reranking cannot introduce passages outside its candidate set")
    relevant = {key for key, value in grades.items() if value > 0}
    selected = ranked_ids[:k]
    gain = lambda values: sum((2 ** grade - 1) / math.log2(index + 2) for index, grade in enumerate(values))
    ideal = gain(sorted(grades.values(), reverse=True)[:k])
    result = {
        "k": k, "answerable": bool(relevant), "relevant_count": len(relevant),
        "returned_count": len(ranked_ids), "candidate_count": len(candidates),
        "ndcg_at_k": gain([grades.get(key, 0) for key in selected]) / ideal if ideal else None,
        "recall_at_k": _ratio(len(relevant.intersection(selected)), len(relevant)),
        "candidate_recall": _ratio(len(relevant.intersection(candidates)), len(relevant)),
        "precision_at_k": len(relevant.intersection(selected)) / k if relevant else None,
    }
    result["ndcg"] = result["ndcg_at_k"]
    result["recall"] = result["recall_at_k"]
    return result


def validate_probabilities(probabilities, labels=None):
    if not isinstance(probabilities, dict) or not probabilities:
        raise ValueError("probabilities must be a nonempty label distribution")
    if any(not isinstance(label, str) or not label for label in probabilities):
        raise ValueError("probability labels must be nonempty strings")
    result = {label: _number(value, "probability") for label, value in probabilities.items()}
    if any(not 0 <= value <= 1 for value in result.values()):
        raise ValueError("probabilities must be in [0,1]")
    if not math.isclose(sum(result.values()), 1, rel_tol=0, abs_tol=1e-6):
        raise ValueError("probabilities must sum to one")
    if labels is not None and set(result) != set(labels):
        raise ValueError("probability label set differs from the frozen configuration")
    return result


def _distribution(row, required=False):
    score_type = row.get("score_type", row.get("raw_score_type", "probabilities"))
    if score_type not in PROBABILITY_TYPES or row.get("probabilities") is None:
        if required:
            raise ValueError("calibration requires probabilities, not confidence or relevance logits")
        return None
    distribution = validate_probabilities(row["probabilities"])
    if row.get("expected") not in distribution:
        raise ValueError("expected label is absent from the probability distribution")
    return distribution


def _choice(distribution):
    return max(sorted(distribution), key=distribution.get)


def classification_metrics(rows, bins=10):
    """Abstentions reduce recall/coverage; categorical Brier is an unnormalised sum."""
    rows = list(rows)
    if isinstance(bins, bool) or not isinstance(bins, int) or bins < 1:
        raise ValueError("ECE bins must be a positive integer")
    labels = set()
    accepted_count = correct_count = 0
    scored = []
    for row in rows:
        expected = row.get("expected")
        prediction = row.get("prediction")
        if not isinstance(expected, str) or not expected:
            raise ValueError("expected labels must be nonempty strings")
        if prediction is not None and (not isinstance(prediction, str) or not prediction):
            raise ValueError("predictions must be labels or null abstentions")
        labels.add(expected)
        accepted = row.get("accepted", prediction is not None) and prediction is not None
        if accepted:
            labels.add(prediction)
            accepted_count += 1
            correct_count += prediction == expected
        distribution = _distribution(row)
        if distribution is not None:
            scored.append((row, distribution))
    per_class = {}
    for label in sorted(labels):
        tp = fp = fn = support = 0
        for row in rows:
            prediction = row.get("prediction") if row.get("accepted", row.get("prediction") is not None) else None
            support += row["expected"] == label
            tp += row["expected"] == label and prediction == label
            fp += row["expected"] != label and prediction == label
            fn += row["expected"] == label and prediction != label
        precision = tp / (tp + fp) if tp + fp else 0.0
        recall = tp / (tp + fn) if tp + fn else 0.0
        per_class[label] = {
            "support": support, "tp": tp, "fp": fp, "fn": fn,
            "precision": precision, "recall": recall, "f1": _harmonic(precision, recall),
        }
    tp = sum(value["tp"] for value in per_class.values())
    fp = sum(value["fp"] for value in per_class.values())
    fn = sum(value["fn"] for value in per_class.values())
    micro_precision = _ratio(tp, tp + fp)
    micro_recall = _ratio(tp, tp + fn)
    brier_values = []
    binary_values = []
    bin_values = [[] for _ in range(bins)]
    for row, distribution in scored:
        expected = row["expected"]
        brier_values.append(sum((value - (label == expected)) ** 2 for label, value in distribution.items()))
        if set(distribution) == {"false", "true"}:
            binary_values.append((distribution["true"] - (expected == "true")) ** 2)
        choice = _choice(distribution)
        confidence = distribution[choice]
        bin_values[min(bins - 1, int(confidence * bins))].append((confidence, float(choice == expected)))
    ece = 0.0
    ece_bins = []
    for index, values in enumerate(bin_values):
        confidence = _mean([value[0] for value in values])
        accuracy = _mean([value[1] for value in values])
        if values:
            ece += len(values) / len(scored) * abs(confidence - accuracy)
        ece_bins.append({
            "lower": index / bins, "upper": (index + 1) / bins,
            "count": len(values), "confidence": confidence, "accuracy": accuracy,
        })
    result = {
        "count": len(rows), "support": len(rows), "accepted_count": accepted_count,
        "correct_count": correct_count, "accuracy": _ratio(correct_count, len(rows)),
        "coverage": _ratio(accepted_count, len(rows)),
        "selective_accuracy": _ratio(correct_count, accepted_count),
        "selective_precision": _ratio(correct_count, accepted_count),
        "macro_precision": _mean([value["precision"] for value in per_class.values()]),
        "macro_recall": _mean([value["recall"] for value in per_class.values()]),
        "macro_f1": _mean([value["f1"] for value in per_class.values()]),
        "micro_precision": micro_precision, "micro_recall": micro_recall,
        "micro_f1": _harmonic(micro_precision, micro_recall), "per_class": per_class,
        "probability_count": len(scored), "brier": _mean(brier_values),
        "brier_definition": "mean(sum_k((p_k - indicator(y=k))^2)); range [0,2]",
        "binary_brier": _mean(binary_values),
        "binary_brier_definition": "mean((p_true - indicator(y=true))^2); range [0,1]",
        "ece": ece if scored else None, "ece_bins": ece_bins,
        "ece_definition": f"top-label, {bins} equal-width bins, all probability-bearing cases",
    }
    result["precision"] = result["macro_precision"]
    result["recall"] = result["macro_recall"]
    result["f1"] = result["macro_f1"]
    result["positive_precision"] = per_class.get("true", {}).get("precision")
    result["positive_recall"] = per_class.get("true", {}).get("recall")
    return result


def selective_curve(rows):
    """Threshold the returned class distribution, never the vendor confidence field."""
    rows = list(rows)
    scored = []
    for row in rows:
        distribution = _distribution(row)
        if distribution is not None:
            prediction = _choice(distribution)
            scored.append((distribution[prediction], prediction == row["expected"]))
    result = []
    for threshold in sorted({confidence for confidence, _ in scored}, reverse=True):
        accepted = [correct for confidence, correct in scored if confidence >= threshold]
        result.append({
            "threshold": threshold, "accepted_count": len(accepted), "correct_count": sum(accepted),
            "coverage": _ratio(len(accepted), len(rows)),
            "selective_accuracy": _ratio(sum(accepted), len(accepted)),
            "selective_precision": _ratio(sum(accepted), len(accepted)),
        })
    return result


def _require_calibration(rows):
    if not rows or any(row.get("split") != "calibration" for row in rows):
        raise ValueError("fitting and threshold selection require only calibration-split rows")


def choose_threshold(rows, target_precision=.98):
    rows = list(rows)
    _require_calibration(rows)
    target_precision = _number(target_precision, "target precision")
    if not 0 < target_precision <= 1:
        raise ValueError("target precision must be in (0,1]")
    choices = [point for point in selective_curve(rows) if point["selective_precision"] >= target_precision]
    if not choices:
        return {
            "threshold": None, "accepted_count": 0, "correct_count": 0, "coverage": 0.0,
            "selective_accuracy": None, "selective_precision": None,
        }
    return max(choices, key=lambda point: (point["coverage"], -point["threshold"]))


def _context_hash(context):
    if not isinstance(context, dict) or CONTEXT_FIELDS.difference(context):
        raise ValueError("calibration context requires model, transport, upstream, precision, prompt_hash, labels and grouping")
    labels = context["labels"]
    if not isinstance(labels, list) or len(labels) < 2 or len(set(labels)) != len(labels):
        raise ValueError("frozen labels must be an ordered list of at least two distinct IDs")
    if any(not isinstance(label, str) or not label for label in labels):
        raise ValueError("frozen label IDs must be nonempty strings")
    if any(context[key] is None or context[key] == "" for key in CONTEXT_FIELDS - {"labels"}):
        raise ValueError("calibration identity fields cannot be missing")
    return hashlib.sha256(canonical_json(context).encode()).hexdigest()


def _temperature(distribution, temperature):
    logits = {label: math.log(max(value, EPSILON)) / temperature for label, value in distribution.items()}
    maximum = max(logits.values())
    weights = {label: math.exp(value - maximum) for label, value in logits.items()}
    total = sum(weights.values())
    return {label: value / total for label, value in weights.items()}


def _sigmoid(value):
    if value >= 0:
        return 1 / (1 + math.exp(-value))
    exponential = math.exp(value)
    return exponential / (1 + exponential)


def _logit(probability):
    probability = min(1 - EPSILON, max(EPSILON, probability))
    return math.log(probability / (1 - probability))


def apply_calibration(probabilities, fit, context):
    if _context_hash(context) != fit.get("context_hash"):
        raise ValueError("model, route or evaluation configuration changed; refit calibration")
    distribution = validate_probabilities(probabilities, context["labels"])
    parameters = fit["parameters"]
    if fit["kind"] == "temperature":
        temperature = _number(parameters["temperature"], "temperature")
        if temperature <= 0:
            raise ValueError("temperature must be positive")
        return _temperature(distribution, temperature)
    if fit["kind"] == "binary":
        positive = parameters["positive_label"]
        if len(distribution) != 2 or positive not in distribution:
            raise ValueError("binary calibration requires the frozen positive label")
        slope = _number(parameters["slope"], "slope")
        intercept = _number(parameters["intercept"], "intercept")
        if slope <= 0:
            raise ValueError("binary calibration must preserve score ordering")
        p = _sigmoid(slope * _logit(distribution[positive]) + intercept)
        return {label: p if label == positive else 1 - p for label in distribution}
    raise ValueError("unsupported calibration kind")


def _golden_minimum(function, lower, upper):
    ratio = (math.sqrt(5) - 1) / 2
    left = upper - ratio * (upper - lower)
    right = lower + ratio * (upper - lower)
    left_value, right_value = function(left), function(right)
    for _ in range(70):
        if left_value < right_value:
            upper, right, right_value = right, left, left_value
            left = upper - ratio * (upper - lower)
            left_value = function(left)
        else:
            lower, left, left_value = left, right, right_value
            right = lower + ratio * (upper - lower)
            right_value = function(right)
    return (lower + upper) / 2


def fit_calibration(rows, context, kind="temperature", target_precision=.98):
    """Fit only calibration probabilities; model/route/prompt/labels are bound by hash."""
    rows = list(rows)
    _require_calibration(rows)
    context_hash = _context_hash(context)
    if kind not in {"temperature", "binary"}:
        raise ValueError("calibration kind must be temperature or binary")
    distributions = [_distribution(row, required=True) for row in rows]
    for distribution in distributions:
        validate_probabilities(distribution, context["labels"])
    observations = list(zip(rows, distributions))
    warnings = []
    groups = sorted({row["group_id"] for row in rows if "group_id" in row})
    if len(rows) < 30:
        warnings.append("Fewer than 30 calibration observations; fitted estimates are preliminary.")
    classes = {row["expected"] for row in rows}
    status = "fitted" if len(classes) > 1 else "identity_insufficient_classes"
    if status != "fitted":
        warnings.append("Only one observed class; retain identity calibration.")
    fit = {
        "format_version": 1, "kind": kind, "status": status,
        "context_hash": context_hash, "context": context,
        "calibration_split": "calibration", "sample_count": len(rows),
        "calibration_case_ids": [row.get("id", row.get("case_id", str(index))) for index, row in enumerate(rows)],
        "calibration_group_ids": groups, "warnings": warnings,
    }
    if kind == "temperature":
        def loss(log_temperature):
            temperature = math.exp(log_temperature)
            nll = -sum(math.log(max(_temperature(distribution, temperature)[row["expected"]], EPSILON)) for row, distribution in observations) / len(rows)
            return nll + .001 * log_temperature ** 2
        before = loss(0)
        best = _golden_minimum(loss, math.log(.05), math.log(20)) if status == "fitted" else 0
        if loss(best) > before:
            best = 0
        fit.update({
            "parameters": {"temperature": math.exp(best)}, "objective_before": before,
            "objective_after": loss(best), "objective": "mean_nll + 0.001 * log(T)^2; T in [0.05,20]",
        })
    else:
        positive = context.get("positive_label", "true" if "true" in context["labels"] else None)
        if len(context["labels"]) != 2 or positive not in context["labels"]:
            raise ValueError("binary calibration requires two labels and an explicit positive_label if not true/false")
        inputs = [(_logit(distribution[positive]), float(row["expected"] == positive)) for row, distribution in observations]
        def loss(slope, intercept):
            total = 0.0
            for value, expected in inputs:
                predicted = min(1 - EPSILON, max(EPSILON, _sigmoid(slope * value + intercept)))
                total -= expected * math.log(predicted) + (1 - expected) * math.log(1 - predicted)
            return total / len(inputs) + .01 * ((slope - 1) ** 2 + intercept ** 2)
        slope, intercept = 1.0, 0.0
        before = loss(slope, intercept)
        for _ in range(80 if status == "fitted" else 0):
            ga, gb = .02 * (slope - 1), .02 * intercept
            haa, hab, hbb = .02, 0.0, .02
            for value, expected in inputs:
                p = _sigmoid(slope * value + intercept)
                error = (p - expected) / len(inputs)
                variance = p * (1 - p) / len(inputs)
                ga += error * value
                gb += error
                haa += variance * value * value
                hab += variance * value
                hbb += variance
            determinant = haa * hbb - hab * hab
            da = (hbb * ga - hab * gb) / determinant
            db = (haa * gb - hab * ga) / determinant
            step = 1.0
            current = loss(slope, intercept)
            while step > 1e-8:
                next_slope = min(20.0, max(.001, slope - step * da))
                next_intercept = min(20.0, max(-20.0, intercept - step * db))
                if loss(next_slope, next_intercept) <= current:
                    break
                step /= 2
            if step <= 1e-8:
                break
            slope, intercept = next_slope, next_intercept
            if abs(step * da) + abs(step * db) < 1e-10:
                break
        fit.update({
            "parameters": {"slope": slope, "intercept": intercept, "positive_label": positive},
            "objective_before": before, "objective_after": loss(slope, intercept),
            "objective": "mean_binary_nll + 0.01 * ((slope-1)^2 + intercept^2); slope in [0.001,20]",
        })
    calibrated = []
    for row, distribution in observations:
        probability = apply_calibration(distribution, fit, context)
        calibrated.append({**row, "probabilities": probability, "prediction": _choice(probability)})
    fit["threshold"] = choose_threshold(calibrated, target_precision)
    return fit


def paired_bootstrap(rows, baseline_key="baseline", candidate_key="candidate", group_key="group_id", seed=53, iterations=2000):
    """Equal-weight meeting means, paired and resampled as complete meeting groups."""
    rows = list(rows)
    if isinstance(iterations, bool) or not isinstance(iterations, int) or iterations < 1:
        raise ValueError("bootstrap iterations must be a positive integer")
    if any(len({row[key] for row in rows if row.get(key) is not None}) > 1 for key in ("language", "split")):
        raise ValueError("bootstrap comparisons must stay within one language and split")
    grouped = defaultdict(list)
    for row in rows:
        if not row.get(group_key):
            raise ValueError("bootstrap requires meeting-group identifiers")
        difference = _number(row[candidate_key], "candidate metric") - _number(row[baseline_key], "baseline metric")
        grouped[row[group_key]].append(difference)
    values = [_mean(grouped[key]) for key in sorted(grouped)]
    result = {
        "case_count": len(rows), "group_count": len(grouped), "mean_difference": _mean(values),
        "confidence": .95, "lower": None, "upper": None, "iterations": iterations, "seed": seed,
        "unit": "meeting_group", "method": "paired percentile bootstrap, equal group weight",
    }
    if values:
        rng = random.Random(seed)
        samples = [sum(rng.choice(values) for _ in values) / len(values) for _ in range(iterations)]
        result.update({"lower": percentile(samples, 2.5), "upper": percentile(samples, 97.5)})
    return result


def render_report(report):
    """Render the supplied measurement artifact without adding clock-dependent text."""
    report = _semantic(report)
    title = report.get("title", "Decision-model evaluation")
    lines = [f"# {title}", ""]
    for key, value in sorted(report.items()):
        if key == "title":
            continue
        lines.extend([f"## {key.replace('_', ' ').capitalize()}", ""])
        if isinstance(value, (dict, list)):
            lines.extend(["```json", json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2, allow_nan=False), "```", ""])
        else:
            lines.extend([str(value), ""])
    return "\n".join(lines)
