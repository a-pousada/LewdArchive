# CPU tagger selection — 2026-10-20

Selected **SmilingWolf/wd-swinv2-tagger-v3** as the quality/latency balance for this CPU deployment. This is not a claim that it is the newest or universally most accurate classifier.

The author's comparable v3 evaluations report macro-F1 **0.4541 for SwinV2** and **0.4419 for ConvNeXt**. Both use the same dataset generation. The chosen SwinV2 revision contains the updated v2.0 weights within the v3 repository and a vocabulary of 10,861 outputs. Its training data cutoff is February 2024, so recognition of newer characters is limited. Sources: [SwinV2 model card](https://huggingface.co/SmilingWolf/wd-swinv2-tagger-v3), [ConvNeXt model card](https://huggingface.co/SmilingWolf/wd-convnext-tagger-v3).

Newer alternatives exist. [PixAI Tagger v1.0](https://huggingface.co/pixai-labs/pixai-tagger-v1.0) uses 486.3 million parameters and 1008×1008 inputs with a May 2026 cutoff. Those characteristics suggest a much higher resource cost; its published performance examples use GPU hardware. It was not benchmarked locally, so no CPU speed comparison or accuracy equivalence is claimed. Other models should be evaluated on representative images before replacing this default.

## Local timing

Measured on an AMD Ryzen 5 5500, Linux x86-64, Docker, ONNX Runtime 1.31.0, CPUExecutionProvider, sequential execution, spinning disabled. Batch size 1, 448×448 float32 input, one warmup followed by eight measured runs. Synthetic constant input avoids using private images. Timing excludes model loading, image fetching and preprocessing; it measures speed, not accuracy.

| Model | Threads | Median per image | Observed range |
| --- | ---: | ---: | ---: |
| WD ConvNeXt v3 | 2 | 1,070.6 ms | 1,035.0–1,110.6 ms |
| WD ConvNeXt v3 | 4 | 804.6 ms | 785.0–848.9 ms |
| WD SwinV2 v3 | 2 | 1,268.3 ms | 1,223.3–1,366.2 ms |
| WD SwinV2 v3 | 4 | 970.6 ms | 955.3–998.6 ms |

Two threads are the default to leave capacity for RSSHub, Postgres and the application. Four threads are optional for faster inference. These are observations on one machine under its existing workload, not performance guarantees. Thread controls follow the [ONNX Runtime documentation](https://onnxruntime.ai/docs/performance/tune-performance/threading.html).

Reproduce with the installed Python dependencies:

```sh
python tagger/benchmark.py /path/to/model.onnx --threads 2
python tagger/benchmark.py /path/to/model.onnx --threads 4
```

## Reproducibility

The service pins SwinV2 revision `627aef95638667ddcaa3ac8ae625e88ea5b02f51`. `tagger/service.py` contains SHA-256 checksums for the ONNX weights and CSV labels. Both are checked before use; interrupted downloads are replaced atomically. Weights occupy 467,460,978 bytes on disk. The benchmark used ConvNeXt revision `d39e46de298d27340111b64965e20b8185c407e6`.

Images receive EXIF orientation correction, white alpha compositing and square padding, followed by bicubic resize and BGR float32 conversion. General and character thresholds default to 0.35 and 0.85; they are operational starting points, not calibrated guarantees. No quantization was applied because its accuracy impact on this library has not been evaluated.

## Idle unload and optional GPU runtime

By default (`WD_MODEL_TTL=300`), the tagger starts without allocating the ONNX Runtime session, loads it lazily on the first `/v1/tag` request, and unloads it back to OS memory after 5 minutes of inactivity (`0` disables unloading). For hosts with an NVIDIA GPU, build with `--build-arg WD_RUNTIME=cuda` (uses `tagger/requirements-cuda.txt` with `onnxruntime-gpu[cuda,cudnn]==1.31.0`) and set `WD_DEVICE=cuda` (or `auto`) alongside `WD_GPU_MEM_LIMIT_MB` (default `1536`).

Tests without model downloads:

```sh
pip install -r tagger/requirements.txt
python -m unittest discover -s tagger -v
```

Real model/HTTP smoke test (downloads the pinned model if absent):

```sh
WD_MODEL_SMOKE=1 MODEL_CACHE=.test-runtime/wd-cache \
  python -m unittest discover -s tagger -v
```

See [deployment and queue behavior](categories-and-ai.md) for environment variables, image limits and migration from Ollama.
