"""Explicit model acquisition and isolated JSONL workers; importing this module is offline."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import selectors
import subprocess
import sys
import time
import urllib.request


DIRECTORY = Path(__file__).resolve().parent
REGISTRY = DIRECTORY / "models.json"
MAX_DOWNLOAD_BYTES = 3 * 1024**3


class LocalModelError(ValueError):
    pass


def registry():
    return json.loads(REGISTRY.read_text(encoding="utf-8"))["models"]


def model_directory(model_id, cache_root):
    model = registry()[model_id]
    return Path(cache_root) / model_id / model["revision"]


def file_hash(path):
    result = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            result.update(chunk)
    return result.hexdigest()


def git_blob_hash(path):
    path = Path(path)
    result = hashlib.sha1(f"blob {path.stat().st_size}\0".encode())
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            result.update(chunk)
    return result.hexdigest()


def model_status(model_id, cache_root):
    model = registry()[model_id]
    path = model_directory(model_id, cache_root)
    receipt_path = path / "artifact-receipt.json"
    if not receipt_path.is_file():
        return {"status": "unavailable", "reason": "artifact_not_downloaded", "model": model_id}
    try:
        receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
        if receipt.get("revision") != model["revision"] or receipt.get("repo") != model["repo"]:
            raise LocalModelError("artifact_identity_mismatch")
        for item in model["files"]:
            filename = item["path"]
            local = path / filename
            observed = receipt["files"][filename]
            if not local.is_file() or local.stat().st_size != item["size"]:
                raise LocalModelError("artifact_missing_or_changed")
            if file_hash(local) != observed["sha256"]:
                raise LocalModelError("artifact_hash_mismatch")
            if item.get("sha256") and observed["sha256"] != item["sha256"]:
                raise LocalModelError("artifact_hash_mismatch")
            if (not item.get("sha256") and item.get("git_blob_sha1")
                    and git_blob_hash(local) != item["git_blob_sha1"]):
                raise LocalModelError("artifact_hash_mismatch")
    except (OSError, ValueError, KeyError, TypeError):
        return {"status": "unavailable", "reason": "artifact_verification_failed", "model": model_id}
    return {"status": "available", "model": model_id, "path": str(path.resolve()),
            "identity": {"repo": model["repo"], "revision": model["revision"],
                         "precision": model["precision"], "files": receipt["files"]},
            "artifact_bytes": sum(item["size"] for item in model["files"])}


def fetch_models(model_ids, cache_root, budget_bytes=MAX_DOWNLOAD_BYTES, opener=None):
    """Download only registry files, reserving even failed attempts against one persistent ceiling."""
    if not 0 < budget_bytes <= MAX_DOWNLOAD_BYTES:
        raise LocalModelError("download_budget_exceeds_authorization")
    entries = registry()
    if len(set(model_ids)) != len(model_ids) or any(key not in entries for key in model_ids):
        raise LocalModelError("unknown_or_duplicate_model")
    root = Path(cache_root).resolve()
    root.mkdir(parents=True, exist_ok=True)
    lock_path = root / ".download.lock"
    try:
        lock = os.open(lock_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    except FileExistsError as exc:
        raise LocalModelError("download_already_running") from exc
    ledger_path = root / "download-ledger.json"
    opener = opener or urllib.request.urlopen
    try:
        if ledger_path.exists():
            ledger = json.loads(ledger_path.read_text(encoding="utf-8"))
        else:
            ledger = {"version": 1, "reserved_bytes": 0, "attempts": []}
        pending = []
        fetched = {}
        for key in model_ids:
            if model_status(key, root)["status"] == "available":
                continue
            partial_path = model_directory(key, root) / "artifact-receipt.json"
            if partial_path.is_file():
                partial = json.loads(partial_path.read_text(encoding="utf-8"))
                if partial.get("repo") != entries[key]["repo"] or partial.get("revision") != entries[key]["revision"]:
                    raise LocalModelError("artifact_identity_mismatch")
                fetched[key] = partial.get("files", {})
            for item in entries[key]["files"]:
                saved = fetched.get(key, {}).get(item["path"])
                local = model_directory(key, root) / item["path"]
                if (saved and local.is_file() and local.stat().st_size == item["size"]
                        and file_hash(local) == saved.get("sha256")
                        and (not item.get("sha256") or saved["sha256"] == item["sha256"])):
                    continue
                pending.append((key, item))
        reserve = sum(item["size"] for _, item in pending)
        if ledger["reserved_bytes"] + reserve > budget_bytes:
            raise LocalModelError("download_budget_exhausted")
        ledger["reserved_bytes"] += reserve
        ledger["attempts"].append({"models": model_ids, "reserved_bytes": reserve,
                                  "status": "reserved"})
        ledger_path.write_text(json.dumps(ledger, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        for key, item in pending:
            path = model_directory(key, root) / item["path"]
            path.parent.mkdir(parents=True, exist_ok=True)
            part = path.with_name(path.name + ".part")
            if path.exists():
                raise LocalModelError("refuse_overwrite_unverified_artifact")
            repo = item.get("repo", entries[key]["repo"])
            revision = item.get("revision", entries[key]["revision"])
            source = item.get("source", item["path"])
            url = f"https://huggingface.co/{repo}/resolve/{revision}/{source}"
            try:
                digest = hashlib.sha256()
                size = 0
                with opener(url, timeout=60) as response, part.open("xb") as output:
                    for chunk in iter(lambda: response.read(1024 * 1024), b""):
                        size += len(chunk)
                        if size > item["size"]:
                            raise LocalModelError("artifact_size_exceeds_manifest")
                        output.write(chunk)
                        digest.update(chunk)
                if (size != item["size"] or (item.get("sha256") and item["sha256"] != digest.hexdigest())
                        or (not item.get("sha256") and item.get("git_blob_sha1")
                            and git_blob_hash(part) != item["git_blob_sha1"])):
                    raise LocalModelError("artifact_verification_failed")
                part.replace(path)
                fetched.setdefault(key, {})[item["path"]] = {"size": size, "sha256": digest.hexdigest()}
                receipt = {"version": 1, "repo": entries[key]["repo"], "revision": entries[key]["revision"],
                           "files": fetched[key]}
                (model_directory(key, root) / "artifact-receipt.json").write_text(
                    json.dumps(receipt, sort_keys=True, indent=2) + "\n", encoding="utf-8")
            finally:
                part.unlink(missing_ok=True)
        for key, files in fetched.items():
            path = model_directory(key, root)
            receipt = {"version": 1, "repo": entries[key]["repo"], "revision": entries[key]["revision"],
                       "files": files}
            (path / "artifact-receipt.json").write_text(
                json.dumps(receipt, sort_keys=True, indent=2) + "\n", encoding="utf-8")
        ledger["attempts"][-1]["status"] = "complete"
        ledger_path.write_text(json.dumps(ledger, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        return {key: model_status(key, root) for key in model_ids}
    finally:
        os.close(lock)
        lock_path.unlink(missing_ok=True)


class LocalWorker:
    def __init__(self, model_id, model_dir, python=None, timeout=120):
        if model_id not in registry():
            raise LocalModelError("unknown_model")
        self.model_id = model_id
        self.model_dir = Path(model_dir).resolve()
        environment = registry()[model_id]["environment"]
        executable = "Scripts/python.exe" if os.name == "nt" else "bin/python"
        default_python = DIRECTORY.parents[1] / "build" / "evaluation-envs" / environment / executable
        self.python = str(python or default_python)
        self.timeout = timeout
        self.process = None
        self.startup = None
        self._buffer = bytearray()
        self.observed_peak_rss_bytes = 0

    def _check_memory(self):
        rss = None
        if sys.platform.startswith("linux"):
            try:
                pages = int(Path(f"/proc/{self.process.pid}/statm").read_text().split()[1])
                rss = pages * os.sysconf("SC_PAGE_SIZE")
            except (OSError, ValueError, IndexError):
                pass
        elif sys.platform == "darwin":
            sample = subprocess.run(["ps", "-p", str(self.process.pid), "-o", "rss="],
                                    capture_output=True, text=True, timeout=1, check=False)
            try:
                rss = int(sample.stdout.strip()) * 1024
            except ValueError:
                pass
        if rss is not None:
            self.observed_peak_rss_bytes = max(self.observed_peak_rss_bytes, rss)
            if rss > 24 * 1024**3:
                self.close()
                raise LocalModelError("worker_memory_budget_exceeded")

    def _read(self):
        if os.name == "nt":
            # communicate() is used by short-lived Windows smoke tests; persistent workers are
            # measured only where an interruptible pipe reader is available.
            raise LocalModelError("persistent_worker_pipe_wait_unmeasured_on_windows")
        deadline = time.monotonic() + self.timeout
        with selectors.DefaultSelector() as selector:
            selector.register(self.process.stdout, selectors.EVENT_READ)
            while True:
                if b"\n" in self._buffer:
                    line, _, rest = self._buffer.partition(b"\n")
                    self._buffer = bytearray(rest)
                    try:
                        result = json.loads(line.decode("utf-8"))
                        if not isinstance(result, dict):
                            raise ValueError("worker_record_must_be_object")
                    except (UnicodeError, ValueError):
                        self.close()
                        raise LocalModelError("worker_protocol_error") from None
                    if (result.get("peak_rss_bytes") or 0) > 24 * 1024**3:
                        self.close()
                        raise LocalModelError("worker_memory_budget_exceeded")
                    return result
                if len(self._buffer) >= 4 * 1024 * 1024:
                    self.close()
                    raise LocalModelError("worker_protocol_error")
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    self.close()
                    raise LocalModelError("worker_timeout")
                self._check_memory()
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    continue
                if not selector.select(min(remaining, 0.25)):
                    continue
                chunk = os.read(self.process.stdout.fileno(), 65536)
                if not chunk:
                    self.close()
                    raise LocalModelError("worker_protocol_error")
                self._buffer.extend(chunk)

    def start(self):
        if self.startup is not None:
            return self.startup
        if os.name == "nt":
            self.startup = {"status": "unavailable", "reason": "persistent_worker_measurement_unavailable_on_windows"}
            return self.startup
        if not Path(self.python).is_file():
            self.startup = {"status": "unavailable", "reason": "isolated_environment_not_installed"}
            return self.startup
        env = os.environ.copy()
        for key in list(env):
            if key.endswith(("_API_KEY", "_TOKEN")) or key.startswith("AWS_"):
                env.pop(key, None)
        env.update({"HF_HUB_OFFLINE": "1", "TRANSFORMERS_OFFLINE": "1", "USE_TF": "0",
                    "TOKENIZERS_PARALLELISM": "false", "OMP_NUM_THREADS": "4", "MKL_NUM_THREADS": "4",
                    "PYTHONDONTWRITEBYTECODE": "1"})
        before = time.perf_counter()
        self.process = subprocess.Popen(
            [self.python, str(DIRECTORY / "workers" / "inference.py"),
             "--model", self.model_id, "--model-dir", str(self.model_dir)],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
            bufsize=0, env=env,
        )
        self.startup = self._read()
        self.startup["process_start_ms"] = (time.perf_counter() - before) * 1000
        return self.startup

    def predict(self, case):
        startup = self.start()
        if startup.get("status") != "ready":
            return {"case_id": case["id"], "status": "unavailable", "reason": startup.get("reason"),
                    "identity": startup.get("identity", {}), "limitations": startup.get("limitations", [])}
        if self.process is None or self.process.poll() is not None:
            self.close()
            return {"case_id": case["id"], "status": "unavailable", "reason": "worker_closed",
                    "identity": startup.get("identity", {})}
        try:
            self.process.stdin.write((json.dumps(case, ensure_ascii=False) + "\n").encode("utf-8"))
            self.process.stdin.flush()
        except OSError:
            self.close()
            return {"case_id": case["id"], "status": "unavailable", "reason": "worker_closed",
                    "identity": startup.get("identity", {})}
        result = self._read()
        result["memory_guard"] = {"observed_peak_rss_bytes": self.observed_peak_rss_bytes,
                                  "budget_bytes": 24 * 1024**3, "sample_interval_ms": 250,
                                  "limitation": "RSS sampling can miss peaks between samples; returned high-water RSS is also checked."}
        return result

    def close(self):
        if self.process:
            if self.process.poll() is None:
                self.process.terminate()
                try:
                    self.process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    self.process.kill()
                    self.process.wait(timeout=5)
            for pipe in (self.process.stdin, self.process.stdout):
                if pipe:
                    pipe.close()
            self.process = None
            self._buffer.clear()
            if self.startup and self.startup.get("status") == "ready":
                self.startup = {"status": "unavailable", "reason": "worker_closed",
                                "identity": self.startup.get("identity", {})}

    def __enter__(self):
        self.start()
        return self

    def __exit__(self, *_):
        self.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["status", "fetch"])
    parser.add_argument("models", nargs="+")
    parser.add_argument("--cache", type=Path, default=DIRECTORY.parents[1] / "build" / "evaluation-models")
    args = parser.parse_args()
    try:
        if args.command == "fetch":
            result = fetch_models(args.models, args.cache)
        else:
            result = {key: model_status(key, args.cache) for key in args.models}
        print(json.dumps(result, sort_keys=True, indent=2))
    except (LocalModelError, KeyError) as exc:
        parser.exit(1, f"Local model operation failed: {type(exc).__name__}\n")


if __name__ == "__main__":
    main()
