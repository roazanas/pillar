#!/usr/bin/env python3
"""Loopback-only playground. Every compiler/execution request uses a fresh container."""
import argparse
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import subprocess
import threading
import uuid

ROOT = Path(__file__).resolve().parent
IMAGE = "pillar-playground:local"
SLOTS = threading.BoundedSemaphore(2)

class SandboxUnavailable(RuntimeError):
    pass

def sandbox(payload, image=IMAGE):
    name = "pillar-playground-" + uuid.uuid4().hex
    command = [
        "docker", "run", "--rm", "--name", name, "--interactive",
        "--network=none", "--memory=512m", "--memory-swap=512m", "--cpus=1",
        "--pids-limit=32", "--read-only", "--cap-drop=ALL",
        "--security-opt=no-new-privileges", "--user=65534:65534",
        "--ulimit", "fsize=1048576:1048576", "--ulimit", "nofile=64:64",
        "--tmpfs", "/work:rw,exec,nosuid,nodev,size=67108864,mode=1777",
        "--tmpfs", "/tmp:rw,noexec,nosuid,nodev,size=16777216,mode=1777",
        image,
    ]
    try:
        result = subprocess.run(command, input=json.dumps(payload).encode(), capture_output=True, timeout=15)
        if result.returncode:
            if result.returncode == 137:
                return {"status": 137, "stdout": "", "stderr": "Execution stopped at the memory limit.", "phase": "sandbox"}
            raise SandboxUnavailable("Unable to start the sandbox. Start Docker and build the playground image.")
        if len(result.stdout) > 800000:
            raise SandboxUnavailable("The sandbox response exceeded the output limit.")
        return json.loads(result.stdout)
    except subprocess.TimeoutExpired:
        return {"status": 124, "stdout": "", "stderr": "Execution stopped at the time limit.", "timed_out": True, "phase": "sandbox"}
    except (FileNotFoundError, json.JSONDecodeError) as error:
        raise SandboxUnavailable("Unable to start the sandbox. Start Docker and build the playground image.") from error
    finally:
        try:
            subprocess.run(["docker", "rm", "--force", name], capture_output=True, timeout=3)
        except (OSError, subprocess.TimeoutExpired):
            pass

class Handler(BaseHTTPRequestHandler):
    server_version = "PillarPlayground"
    def send_headers(self, status, content_type, length):
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(length))
        self.send_header("X-Content-Type-Options", "nosniff")
        self.send_header("Content-Security-Policy", "default-src 'self'; script-src 'self'; style-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'")
        self.send_header("Cache-Control", "no-store")
        self.send_header("Connection", "close")
        self.end_headers()
    def json(self, status, value):
        data = json.dumps(value).encode()
        self.send_headers(status, "application/json; charset=utf-8", len(data))
        self.wfile.write(data)
    def origin_allowed(self):
        port = self.server.server_port
        hosts = {f"localhost:{port}", f"127.0.0.1:{port}"}
        host = self.headers.get("Host", "")
        origin = self.headers.get("Origin")
        return host in hosts and (origin is None or origin == "http://" + host)
    def do_GET(self):
        if not self.origin_allowed():
            self.json(403, {"error": "Use the local playground address."})
            return
        assets = {"/": (ROOT / "index.html", "text/html"), "/app.js": (ROOT / "app.js", "text/javascript"), "/style.css": (ROOT / "style.css", "text/css"), "/columns.js": (ROOT.parent / "editor/vscode/columns.js", "text/javascript")}
        if self.path not in assets:
            self.json(404, {"error": "Not found"})
            return
        path, content_type = assets[self.path]
        data = path.read_bytes()
        self.send_headers(200, content_type + "; charset=utf-8", len(data))
        self.wfile.write(data)
    def do_POST(self):
        if not self.origin_allowed() or self.path != "/api" or self.headers.get("Content-Type", "").split(";")[0] != "application/json":
            self.json(403, {"error": "Use the playground controls to submit code."})
            return
        try:
            length = int(self.headers.get("Content-Length", "0"))
        except ValueError:
            length = 0
        if not 0 < length <= 524288:
            self.json(413, {"error": "Use at most 64 KiB of source and 16 KiB of input."})
            return
        try:
            payload = json.loads(self.rfile.read(length))
            if not isinstance(payload, dict) or payload.get("action") not in {"check", "fmt", "run"} or not isinstance(payload.get("source"), str) or len(payload["source"].encode()) > 65536 or not isinstance(payload.get("stdin", ""), str) or len(payload.get("stdin", "").encode()) > 16384:
                raise ValueError("Invalid request")
        except (ValueError, UnicodeError, RecursionError):
            self.json(400, {"error": "Submit valid source and input within the displayed limits."})
            return
        if self.server.preview_only:
            self.json(503, {"error": "Preview mode: start Docker and restart without --preview-only to check or run code."})
            return
        if not SLOTS.acquire(blocking=False):
            self.json(429, {"error": "Two programs are already running. Try again shortly."})
            return
        try:
            self.json(200, sandbox(payload))
        except SandboxUnavailable as error:
            self.json(503, {"error": str(error)})
        finally:
            SLOTS.release()
    def setup(self):
        super().setup()
        self.connection.settimeout(20)

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, default=8765)
    parser.add_argument("--preview-only", action="store_true", help="Serve the editor without compiling or running code")
    args = parser.parse_args()
    if not args.preview_only:
        try:
            subprocess.run(["docker", "info"], check=True, capture_output=True, timeout=10)
        except (OSError, subprocess.SubprocessError):
            parser.error("Start Docker first, or use --preview-only to inspect the editor.")
    server = ThreadingHTTPServer(("127.0.0.1", args.port), Handler)
    server.daemon_threads = True
    server.preview_only = args.preview_only
    print(f"Pillar playground: http://127.0.0.1:{server.server_port}", flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
