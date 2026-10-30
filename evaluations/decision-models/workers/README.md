# Optional local evaluation workers

These environments are independent of Pennel and of ordinary CI. The parent harness uses only Python's standard library. Workers read local, verified artifacts and set Hugging Face/Transformers offline mode. They never download a missing model during inference.

## Setup and explicit acquisition

From the repository root, using Python 3.11 and `uv`:

```sh
uv venv --python 3.11 build/evaluation-envs/decisions
uv pip sync --python build/evaluation-envs/decisions/bin/python --require-hashes evaluations/decision-models/workers/envs/decisions.lock
uv venv --python 3.11 build/evaluation-envs/legacy
uv pip sync --python build/evaluation-envs/legacy/bin/python --require-hashes evaluations/decision-models/workers/envs/legacy.lock
uv venv --python 3.11 build/evaluation-envs/onnx
uv pip sync --python build/evaluation-envs/onnx/bin/python --require-hashes evaluations/decision-models/workers/envs/onnx.lock
```

On Windows the interpreter is `Scripts/python.exe`. Persistent-worker latency collection is currently unavailable on Windows; offline contract tests still run there. Linux CPU-only Torch installation may use a separately resolved CPU-wheel lock; do not label a different lock as this Mac configuration.

Model acquisition is separate, explicit and constrained to the committed file/revision manifest. Run only after freezing the protocol and corpus. The 3 GiB ceiling includes tokenizer/config/custom-code files, reservations for failed attempts and earlier acquisitions in the same cache:

```sh
python3 evaluations/decision-models/local_models.py fetch gliclass-multilingual gliner-multilingual gte-reranker gliclass-v4-q8
python3 evaluations/decision-models/local_models.py status gliclass-multilingual gliner-multilingual gte-reranker gliclass-v4-q8
```

The four primary artifacts total 2,770,266,421 bytes (about 2.58 GiB), based on upstream file metadata. This is not measured RAM. Laya adds 678,201,774 bytes and exceeds the first-run ceiling when all four primary artifacts have been acquired. Keep it deferred unless already available within the same verified budget or selected instead of another candidate.

Start one CPU worker at a time through `local_models.LocalWorker(model_id, model_dir, python=...)`. The startup JSON record includes process/model load time, package versions and artifact fingerprints. Each request receives a JSON record with scores, identity, latency and peak process RSS. Cold startup is a fresh owned process, not a guarantee of cold operating-system file caches. RSS includes the worker/runtime, is a high-water mark for the process lifetime, and excludes unrelated services. No native Pennel binary is created or measured by these workers.

## Model-specific boundaries

- GLiClass multilingual uses the owner's 0.1.21 uni-encoder preparation and direct model logits. Label IDs/descriptions and task instructions remain in the pinned input. Single-label softmax returns a complete option distribution; it is not a task-calibration guarantee.
- GLiNER2.5 multilingual uses `AutoExtractor.classify_text` with named labels and prompt. The adapter returns the actual selected label and optional top-label confidence. It does not manufacture a complete distribution from that confidence; Brier/ECE remain unavailable.
- GTE scores query/passage pairs with relevance logits. It is a reranker, with no answerability/claim-support probability contract. Its model config references `Alibaba-NLP/new-impl`; the worker instead imports two reviewed local source files pinned at revision `40ced75c3017eb27626c9d4ea981bde21a2662f4`, bypassing unpinned remote-code discovery.
- GLiClass v4 Q8 inference stays unavailable until a graph/preprocessing/reference-parity receipt has been recorded. An ONNX file on disk does not prove preprocessing or quantization parity. `onnx-contract.json` must identify the graph/tokenizer SHA-256, exact graph inputs/output, `preprocessing: "gliclass-0.1.21-uni-encoder"`, and a reference-parity record with `passed: true` and an exact reference revision. The current adapter supports only `input_ids`/`attention_mask` int64 inputs and a single logits output of the expected label count; other graphs fail explicitly. Do not create a passing receipt without a real reference comparison.
- Laya is a secondary local comparison with the owner's documented calibration and order/option sensitivity. Its automatic multilingual Router is not used, so it cannot silently load a second checkpoint.

No runner enrolls voices, opens production stores, mutates installed model-server settings or substitutes another checkpoint. Model text and library diagnostics are suppressed from logs; synthetic predictions are returned only through the evaluation JSONL channel.
