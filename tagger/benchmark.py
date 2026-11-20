"""Inference-only timing: python benchmark.py /path/to/model.onnx --threads 2."""
import argparse
import json
import statistics
import time

import numpy as np
import onnxruntime as ort

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("model")
parser.add_argument("--threads", type=int, default=2)
args = parser.parse_args()
ort.disable_telemetry_events()
options = ort.SessionOptions()
options.intra_op_num_threads = args.threads
options.inter_op_num_threads = 1
options.execution_mode = ort.ExecutionMode.ORT_SEQUENTIAL
options.add_session_config_entry("session.intra_op.allow_spinning", "0")
session = ort.InferenceSession(args.model, sess_options=options, providers=["CPUExecutionProvider"])
name = session.get_inputs()[0].name
tensor = np.full((1, 448, 448, 3), 127, dtype=np.float32)
session.run(None, {name: tensor})
times = []
for _ in range(8):
    start = time.perf_counter()
    session.run(None, {name: tensor})
    times.append((time.perf_counter() - start) * 1000)
print(json.dumps({
    "model": args.model, "threads": args.threads, "runs": len(times),
    "median_ms": round(statistics.median(times), 1),
    "min_ms": round(min(times), 1), "max_ms": round(max(times), 1),
}))
