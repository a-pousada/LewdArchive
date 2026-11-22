"""Local WD image classification with bounded memory use and immutable weights."""
import csv
import ctypes
import gc
import hashlib
import io
import json
import logging
import math
import os
from pathlib import Path
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, HTTPServer
from urllib.request import urlopen
import warnings

import numpy as np
import onnxruntime as ort
from PIL import Image, ImageOps, UnidentifiedImageError

MODEL = "SmilingWolf/wd-swinv2-tagger-v3"
REVISION = "627aef95638667ddcaa3ac8ae625e88ea5b02f51"
FILES = {
    "model.onnx": "e6774bff34d43bd49f75a47db4ef217dce701c9847b546523eb85ff6dbba1db1",
    "selected_tags.csv": "298633d94d0031d2081c0893f29c82eab7f0df00b08483ba8f29d1e979441217",
}
MAX_BYTES = 5 * 1024 * 1024
Image.MAX_IMAGE_PIXELS = 16_000_000
warnings.simplefilter("error", Image.DecompressionBombWarning)
ort.disable_telemetry_events()


def setting(name, default, minimum, maximum, cast=float):
    value = cast(os.environ.get(name, default))
    if not math.isfinite(value) or not minimum <= value <= maximum:
        raise ValueError(f"{name} must be between {minimum} and {maximum}")
    return value


def device_setting():
    value = os.environ.get("WD_DEVICE", "cpu").strip().lower()
    if value not in {"cpu", "cuda", "auto"}:
        raise ValueError("WD_DEVICE must be one of: cpu, cuda, auto")
    return value


def _trim_memory():
    gc.collect()
    try:
        ctypes.CDLL("libc.so.6").malloc_trim(0)
    except Exception:
        pass


def digest(path):
    with path.open("rb") as file:
        return hashlib.file_digest(file, "sha256").hexdigest()


def model_files():
    cache = Path(os.environ.get("MODEL_CACHE", "/models")) / REVISION
    cache.mkdir(parents=True, exist_ok=True)
    for name, expected in FILES.items():
        target = cache / name
        if target.exists() and (expected is None or digest(target) == expected):
            continue
        logging.info("Downloading pinned WD artifact: %s", name)
        # Atomic replacement allows restart after an interrupted download.
        temporary = None
        try:
            with tempfile.NamedTemporaryFile(dir=cache, delete=False) as out:
                temporary = Path(out.name)
                with urlopen(
                    f"https://huggingface.co/{MODEL}/resolve/{REVISION}/{name}",
                    timeout=60,
                ) as response:
                    total = 0
                    limit = 500 * 1024 * 1024 if name.endswith(".onnx") else 1024 * 1024
                    while chunk := response.read(1024 * 1024):
                        total += len(chunk)
                        if total > limit:
                            raise ValueError("Model artifact exceeds its size limit")
                        out.write(chunk)
            if expected is not None and digest(temporary) != expected:
                raise ValueError(f"Checksum mismatch for {name}")
            temporary.replace(target)
        finally:
            if temporary is not None:
                temporary.unlink(missing_ok=True)
    return cache


def prepare(data, size):
    """Match WD's white square padding, bicubic resize and NHWC BGR input."""
    if not data or len(data) > MAX_BYTES:
        raise ValueError("Image must contain between 1 byte and 5 MiB")
    try:
        with Image.open(io.BytesIO(data)) as original:
            if original.format not in {"JPEG", "PNG", "WEBP", "GIF"}:
                raise ValueError("Use JPEG, PNG, WebP or GIF images")
            # Bound square-padding memory too, including pathological panoramas.
            if max(original.size) > 4096 or original.width * original.height > Image.MAX_IMAGE_PIXELS:
                raise ValueError("Image exceeds 4096 pixels per side or 16 megapixels")
            original.seek(0)
            rgba = ImageOps.exif_transpose(original).convert("RGBA")
    except (UnidentifiedImageError, OSError, Image.DecompressionBombWarning,
            Image.DecompressionBombError) as error:
        raise ValueError("Invalid or oversized image") from error
    background = Image.new("RGBA", rgba.size, "white")
    background.alpha_composite(rgba)
    rgb = background.convert("RGB")
    side = max(rgb.size)
    canvas = Image.new("RGB", (side, side), "white")
    canvas.paste(rgb, ((side - rgb.width) // 2, (side - rgb.height) // 2))
    resized = canvas.resize((size, size), Image.Resampling.BICUBIC)
    return np.ascontiguousarray(np.asarray(resized, dtype=np.float32)[None, :, :, ::-1])


def select_tags(scores, labels, general, character):
    if len(scores) != len(labels) or not np.isfinite(scores).all() or np.any((scores < 0) | (scores > 1)):
        raise ValueError("Invalid model output")
    found = []
    for score, (name, category) in zip(scores, labels):
        threshold = general if category == 0 else character if category == 4 else None
        if threshold is None or score < threshold:
            continue  # Ratings are not tags or channel-routing decisions.
        name = "_".join(name.lower().split())
        # The application accepts only this bounded, mention-free vocabulary.
        if not name or len(name) > 48 or not all(c.isalnum() or c in "_-()" for c in name):
            continue
        found.append({"name": name, "score": float(score)})
    return sorted(found, key=lambda tag: (-tag["score"], tag["name"]))[:30]


class Classifier:
    def __init__(self):
        self.general = setting("WD_GENERAL_THRESHOLD", "0.35", 0, 1)
        self.character = setting("WD_CHARACTER_THRESHOLD", "0.85", 0, 1)
        self.threads = setting("WD_CPU_THREADS", "2", 1, 32, int)
        self.ttl = setting("WD_MODEL_TTL", "300", 0, 86400, float)
        self.device = device_setting()
        self.gpu_mem_limit_mb = setting("WD_GPU_MEM_LIMIT_MB", "1536", 256, 32768, int)
        self.cache = model_files()
        with (self.cache / "selected_tags.csv").open(newline="", encoding="utf-8") as file:
            self.labels = [(row["name"], int(row["category"])) for row in csv.DictReader(file)]
        if not self.labels:
            raise ValueError("Model vocabulary is empty")
        self.size = 448
        self.session = None
        self.input = None
        self.active_provider = None
        self._lock = threading.Lock()
        self._last_used = 0.0
        self._stop = threading.Event()
        self._watchdog = None
        if self.ttl == 0:
            with self._lock:
                self._load_session()
        else:
            self._watchdog = threading.Thread(target=self._idle_loop, daemon=True)
            self._watchdog.start()
            logging.info(
                "WD ready (lazy): %s, device=%s, threads=%s, ttl=%ss, labels=%s",
                MODEL, self.device, self.threads, self.ttl, len(self.labels),
            )

    @property
    def loaded(self):
        return self.session is not None

    def _resolve_providers(self):
        if self.device in {"cuda", "auto"}:
            if hasattr(ort, "preload_dlls"):
                try:
                    ort.preload_dlls()
                except Exception as exc:
                    logging.warning("ONNX preload_dlls failed: %s", exc)
            available = ort.get_available_providers()
            if "CUDAExecutionProvider" in available:
                return [
                    (
                        "CUDAExecutionProvider",
                        {
                            "arena_extend_strategy": "kSameAsRequested",
                            "gpu_mem_limit": self.gpu_mem_limit_mb * 1024 * 1024,
                        },
                    ),
                    "CPUExecutionProvider",
                ]
            if self.device == "cuda":
                raise RuntimeError(
                    f"WD_DEVICE=cuda requested, but available providers are {available}"
                )
        return ["CPUExecutionProvider"]

    def _load_session(self):
        options = ort.SessionOptions()
        options.intra_op_num_threads = self.threads
        options.inter_op_num_threads = 1
        options.execution_mode = ort.ExecutionMode.ORT_SEQUENTIAL
        options.add_session_config_entry("session.intra_op.allow_spinning", "0")
        if self.ttl > 0:
            # Disable CPU memory arena when TTL unload is active so RAM returns to OS cleanly.
            options.enable_cpu_mem_arena = False
        providers = self._resolve_providers()
        session = ort.InferenceSession(
            str(self.cache / "model.onnx"), sess_options=options,
            providers=providers,
        )
        tensor = session.get_inputs()[0]
        if tensor.shape[1:] != [448, 448, 3] or tensor.type != "tensor(float)":
            raise ValueError("Unexpected WD input shape")
        if session.get_outputs()[0].shape[-1] != len(self.labels):
            raise ValueError("Model and label vocabulary do not match")
        self.session = session
        self.input = tensor.name
        self.size = tensor.shape[1]
        self.active_provider = session.get_providers()[0]
        self._last_used = time.monotonic()
        logging.info(
            "WD model loaded: %s, provider=%s, threads=%s, ttl=%ss",
            MODEL, self.active_provider, self.threads, self.ttl,
        )

    def unload(self):
        with self._lock:
            if self.session is not None:
                self.session = None
                self.input = None
                self.active_provider = None
                _trim_memory()
                logging.info("Unloaded idle WD model")

    def _idle_loop(self):
        interval = min(15.0, max(0.05, self.ttl / 4.0))
        while not self._stop.wait(interval):
            with self._lock:
                if self.session is not None and (time.monotonic() - self._last_used) >= self.ttl:
                    self.session = None
                    self.input = None
                    self.active_provider = None
                    _trim_memory()
                    logging.info("Unloaded idle WD model after %.1fs", self.ttl)

    def close(self):
        self._stop.set()
        self.unload()
        if self._watchdog is not None and self._watchdog.is_alive():
            self._watchdog.join(timeout=2)

    def classify(self, data):
        tensor = prepare(data, self.size)
        with self._lock:
            if self.session is None:
                self._load_session()
            self._last_used = time.monotonic()
            scores = self.session.run(None, {self.input: tensor})[0][0]
            self._last_used = time.monotonic()
        return {
            "model": MODEL, "revision": REVISION,
            "tags": select_tags(scores, self.labels, self.general, self.character),
        }


class Server(HTTPServer):
    # A single inference at a time, with a bounded TCP backlog.
    request_queue_size = 4

    def __init__(self, address, classifier):
        self.classifier = classifier
        super().__init__(address, Handler)


class Handler(BaseHTTPRequestHandler):
    def setup(self):
        super().setup()
        self.connection.settimeout(30)

    def reply(self, status, body):
        encoded = json.dumps(body, allow_nan=False).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(encoded)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(encoded)

    def do_GET(self):
        if self.path == "/health":
            self.reply(200, {
                "ready": True,
                "model": MODEL,
                "revision": REVISION,
                "loaded": getattr(self.server.classifier, "loaded", True),
                "device": getattr(self.server.classifier, "active_provider", None),
            })
        else:
            self.reply(404, {"error": "Not found"})

    def do_POST(self):
        if self.path != "/v1/tag":
            self.reply(404, {"error": "Not found"})
            return
        if self.headers.get("Transfer-Encoding"):
            self.reply(400, {"error": "Chunked requests are not supported"})
            return
        try:
            length = int(self.headers.get("Content-Length", "0"))
        except ValueError:
            length = 0
        if not 0 < length <= MAX_BYTES:
            self.reply(413, {"error": "Image must contain between 1 byte and 5 MiB"})
            return
        try:
            data = self.rfile.read(length)
            if len(data) != length:
                raise ValueError("Incomplete image")
            result = self.server.classifier.classify(data)
        except ValueError as error:
            self.reply(422, {"error": str(error)})
            return
        except Exception:
            logging.exception("WD inference failed")
            self.reply(500, {"error": "Image inference failed"})
            return
        self.reply(200, result)

    def log_message(self, format, *args):
        logging.info(format, *args)


if __name__ == "__main__":
    logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(message)s")
    Server((os.environ.get("WD_BIND_ADDRESS", "127.0.0.1"), 5000), Classifier()).serve_forever()
