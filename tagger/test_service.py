import http.client
import io
import json
import os
from pathlib import Path
import tempfile
import threading
import unittest
from unittest.mock import patch

import numpy as np
from PIL import Image

import service


def image_bytes(mode="RGB", color="red", size=(32, 16), format="PNG"):
    data = io.BytesIO()
    Image.new(mode, size, color).save(data, format=format)
    return data.getvalue()


class PreprocessingTests(unittest.TestCase):
    def test_padding_bgr_float32_and_alpha(self):
        tensor = service.prepare(image_bytes(), 448)
        self.assertEqual(tensor.shape, (1, 448, 448, 3))
        self.assertEqual(tensor.dtype, np.float32)
        self.assertTrue(tensor.flags.c_contiguous)
        np.testing.assert_array_equal(tensor[0, 224, 224], [0, 0, 255])
        np.testing.assert_array_equal(tensor[0, 0, 0], [255, 255, 255])
        transparent = service.prepare(image_bytes("RGBA", (255, 0, 0, 0)), 448)
        self.assertTrue(np.all(transparent == 255))

    def test_formats_and_limits(self):
        for format in ["PNG", "JPEG", "WEBP"]:
            self.assertEqual(service.prepare(image_bytes(format=format), 448).shape[0], 1)
        for data in [
            b"", b"not an image", b"x" * (service.MAX_BYTES + 1),
            image_bytes(format="GIF"), image_bytes(size=(4097, 1)),
            image_bytes(size=(4096, 4096)),
        ]:
            with self.assertRaises(ValueError):
                service.prepare(data, 448)

    def test_thresholds_vocabulary_and_ratings(self):
        labels = [
            ("general", 9), ("Blue Sky", 0), ("character_(series)", 4),
            ("low_character", 4), ("@everyone", 0), ("low_score", 0),
        ]
        found = service.select_tags(np.array([.99, .8, .9, .84, .99, .34]), labels, .35, .85)
        self.assertEqual([tag["name"] for tag in found], ["character_(series)", "blue_sky"])
        with self.assertRaises(ValueError):
            service.select_tags(np.array([np.nan]), [("sky", 0)], .35, .85)
        with self.assertRaises(ValueError):
            service.select_tags(np.array([]), labels, .35, .85)

    def test_model_artifact_repair_and_checksum_failure(self):
        payload = b"test weights"
        with tempfile.TemporaryDirectory() as temp:
            with patch.dict(os.environ, {"MODEL_CACHE": temp}), \
                 patch.object(service, "FILES", {"model.onnx": service.hashlib.sha256(payload).hexdigest()}), \
                 patch.object(service, "urlopen", side_effect=lambda *a, **kw: io.BytesIO(payload)) as download:
                cache = service.model_files()
                self.assertEqual((cache / "model.onnx").read_bytes(), payload)
                service.model_files()
                self.assertEqual(download.call_count, 1)
                (cache / "model.onnx").write_bytes(b"broken")
                service.model_files()
                self.assertEqual(download.call_count, 2)
                self.assertEqual((cache / "model.onnx").read_bytes(), payload)
            with patch.dict(os.environ, {"MODEL_CACHE": temp}), \
                 patch.object(service, "FILES", {"model.onnx": "0" * 64}), \
                 patch.object(service, "urlopen", side_effect=lambda *a, **kw: io.BytesIO(payload)):
                with self.assertRaisesRegex(ValueError, "Checksum mismatch"):
                    service.model_files()
                self.assertEqual([p.name for p in Path(cache).iterdir()], ["model.onnx"])

    def test_invalid_runtime_settings(self):
        for value in ["nan", "-1", "1.1"]:
            with patch.dict(os.environ, {"WD_GENERAL_THRESHOLD": value}):
                with self.assertRaises(ValueError):
                    service.setting("WD_GENERAL_THRESHOLD", ".35", 0, 1)


class StubClassifier:
    def classify(self, data):
        service.prepare(data, 448)
        return {"tags": [{"name": "red_background", "score": .9}]}


class HttpTests(unittest.TestCase):
    def setUp(self):
        self.server = service.Server(("127.0.0.1", 0), StubClassifier())
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()

    def tearDown(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join()

    def request(self, method, path, body=None, headers=None):
        connection = http.client.HTTPConnection(*self.server.server_address, timeout=5)
        try:
            connection.request(method, path, body, headers or {})
            response = connection.getresponse()
            return response.status, json.loads(response.read())
        finally:
            connection.close()

    def test_http_contract(self):
        self.assertEqual(self.request("GET", "/health")[0], 200)
        status, result = self.request("POST", "/v1/tag", image_bytes())
        self.assertEqual(status, 200)
        self.assertEqual(result["tags"][0]["name"], "red_background")
        self.assertEqual(self.request("POST", "/v1/tag", b"broken")[0], 422)
        self.assertEqual(self.request("POST", "/v1/tag")[0], 413)
        self.assertEqual(self.request("POST", "/v1/tag", headers={"Content-Length": str(service.MAX_BYTES + 1)})[0], 413)
        self.assertEqual(self.request("GET", "/missing")[0], 404)


@unittest.skipUnless(os.environ.get("WD_MODEL_SMOKE") == "1", "Opt-in real model smoke test")
class RealModelTests(unittest.TestCase):
    def test_real_model_and_http(self):
        classifier = service.Classifier()
        server = service.Server(("127.0.0.1", 0), classifier)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            connection = http.client.HTTPConnection(*server.server_address, timeout=120)
            connection.request("POST", "/v1/tag", image_bytes())
            response = connection.getresponse()
            result = json.loads(response.read())
            connection.close()
            self.assertEqual(response.status, 200)
            self.assertEqual(result["model"], service.MODEL)
            self.assertGreater(len(result["tags"]), 0)
            self.assertLessEqual(len(result["tags"]), 30)
            self.assertTrue(all(0 <= tag["score"] <= 1 for tag in result["tags"]))
        finally:
            server.shutdown()
            server.server_close()
            thread.join()


if __name__ == "__main__":
    unittest.main()
