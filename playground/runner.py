"""Container entry point. Never invoke this runner on the host."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

LIMIT = 65536

def phase(command, timeout, stdin=""):
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        try:
            result = subprocess.run(command, input=stdin.encode(), stdout=stdout, stderr=stderr, timeout=timeout)
            status, timed_out = result.returncode, False
        except subprocess.TimeoutExpired:
            status, timed_out = 124, True
        stdout.seek(0)
        stderr.seek(0)
        out, err = stdout.read(LIMIT + 1), stderr.read(LIMIT + 1)
        return {"status": status, "stdout": out[:LIMIT].decode("utf-8", "replace"), "stderr": err[:LIMIT].decode("utf-8", "replace"), "truncated": len(out) > LIMIT or len(err) > LIMIT, "timed_out": timed_out}

def main():
    if os.environ.get("PILLAR_SANDBOX") != "1":
        raise SystemExit("This runner requires the constrained playground container.")
    payload = json.loads(sys.stdin.buffer.read(524289))
    source = payload.get("source", "")
    action = payload.get("action")
    stdin = payload.get("stdin", "")
    if not isinstance(source, str) or len(source.encode()) > LIMIT or action not in {"check", "fmt", "run"} or not isinstance(stdin, str) or len(stdin.encode()) > 16384:
        raise SystemExit("Invalid request")
    Path("/work/main.plr").write_text(source)
    command = ["plrc", "--color", "never", "--diagnostic-format", "json"]
    if action in {"check", "fmt"}:
        result = phase(command + [action, "/work/main.plr"], 8)
        result["phase"] = action
    else:
        result = phase(command + ["build", "/work/main.plr", "-o", "/work/program"], 8)
        result["phase"] = "build"
        if result["status"] == 0:
            result = phase(["/work/program"], 2, stdin)
            result["phase"] = "run"
    print(json.dumps(result))

if __name__ == "__main__":
    main()
