#!/usr/bin/env python3
import json
import sys
import tempfile
import threading
import unittest
import urllib.parse
import urllib.request
from http.server import ThreadingHTTPServer
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import local_server


class LocalSandboxServerTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tempdir = tempfile.TemporaryDirectory()
        local_server.SandboxHandler.workdir = self.tempdir.name
        self.server = ThreadingHTTPServer(("127.0.0.1", 0), local_server.SandboxHandler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.base_url = f"http://127.0.0.1:{self.server.server_port}"

    def tearDown(self) -> None:
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=5)
        self.tempdir.cleanup()

    def request(self, method: str, path: str, body: bytes | None = None) -> urllib.request.addinfourl:
        request = urllib.request.Request(f"{self.base_url}{path}", data=body, method=method)
        return urllib.request.urlopen(request, timeout=10)

    def test_tensorlake_compatible_endpoints(self) -> None:
        health_path = "/api/v1/files/list?path=/"
        with self.request("GET", health_path) as response:
            self.assertEqual(response.status, 200)
            payload = json.loads(response.read().decode("utf-8"))
        self.assertIn("entries", payload)

        file_path = "/workspace/demo.txt"
        encoded_path = urllib.parse.quote(file_path, safe="")
        with self.request(
            "PUT",
            f"/api/v1/files?path={encoded_path}",
            b"hello from tensorlake-compatible local sandbox",
        ) as response:
            self.assertEqual(response.status, 200)

        with self.request("GET", f"/api/v1/files?path={encoded_path}") as response:
            self.assertEqual(response.status, 200)
            self.assertEqual(
                response.read().decode("utf-8"),
                "hello from tensorlake-compatible local sandbox",
            )

        generated_file = Path(self.tempdir.name) / "workspace" / "generated.txt"
        command = {
            "command": "/bin/bash",
            "args": ["-c", "echo local-process > /workspace/generated.txt"],
        }
        with self.request(
            "POST",
            "/api/v1/processes",
            json.dumps(command).encode("utf-8"),
        ) as response:
            self.assertEqual(response.status, 200)

        self.assertEqual(generated_file.read_text(encoding="utf-8").strip(), "local-process")


if __name__ == "__main__":
    unittest.main()
