import http.client
import json
from pathlib import Path
import subprocess
import threading
import unittest
from unittest import mock
import server

class SandboxTests(unittest.TestCase):
    def test_constraints_and_cleanup(self):
        completed = subprocess.CompletedProcess([], 0, b'{"status":0,"stdout":"ok"}', b'')
        with mock.patch.object(server.subprocess, "run", return_value=completed) as run:
            result = server.sandbox({"action": "run", "source": "source"})
            self.assertEqual(result["stdout"], "ok")
            flags = run.call_args_list[0].args[0]
            for value in ["--network=none", "--read-only", "--cap-drop=ALL", "--user=65534:65534", "--memory=512m", "--pids-limit=32"]:
                self.assertIn(value, flags)
            self.assertNotIn("--privileged", flags)
            self.assertNotIn("--volume", flags)
            self.assertEqual(run.call_args_list[-1].args[0][:3], ["docker", "rm", "--force"])
    def test_timeout_cleans_container(self):
        with mock.patch.object(server.subprocess, "run", side_effect=[subprocess.TimeoutExpired("docker", 15), subprocess.CompletedProcess([], 0)]) as run:
            self.assertTrue(server.sandbox({})["timed_out"])
            self.assertEqual(run.call_count, 2)

class HTTPTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.http = server.ThreadingHTTPServer(("127.0.0.1", 0), server.Handler)
        cls.http.preview_only = True
        cls.thread = threading.Thread(target=cls.http.serve_forever, daemon=True)
        cls.thread.start()
    @classmethod
    def tearDownClass(cls):
        cls.http.shutdown()
        cls.http.server_close()
        cls.thread.join()
    def request(self, method, path, value=None, headers=None):
        connection = http.client.HTTPConnection("127.0.0.1", self.http.server_port, timeout=3)
        headers = headers or {}
        if value is not None:
            headers.setdefault("Content-Type", "application/json")
        connection.request(method, path, json.dumps(value) if value is not None else None, headers)
        response = connection.getresponse()
        body = response.read()
        connection.close()
        return response, body
    def test_static_assets_and_security_headers(self):
        for path in ["/", "/app.js", "/style.css", "/columns.js"]:
            response, body = self.request("GET", path)
            self.assertEqual(response.status, 200)
            self.assertTrue(body)
            self.assertIn("frame-ancestors 'none'", response.headers["Content-Security-Policy"])
        self.assertEqual(self.request("GET", "/../Cargo.toml")[0].status, 404)
    def test_no_execution_in_preview_mode(self):
        with mock.patch.object(server, "sandbox") as run:
            response, _ = self.request("POST", "/api", {"action": "run", "source": ""})
            self.assertEqual(response.status, 503)
            run.assert_not_called()
    def test_request_validation(self):
        value = {"action": "check", "source": ""}
        self.assertEqual(self.request("POST", "/api", value, {"Origin": "https://example.com"})[0].status, 403)
        self.assertEqual(self.request("POST", "/api", value, {"Host": "attacker.example"})[0].status, 403)
        self.assertEqual(self.request("POST", "/api", {"action": "shell", "source": ""})[0].status, 400)
        self.assertEqual(self.request("POST", "/api", {"action": "run", "source": "x" * 70000})[0].status, 400)
        self.assertEqual(self.request("POST", "/api", None, {"Content-Type": "text/plain"})[0].status, 403)

if __name__ == "__main__":
    unittest.main()
