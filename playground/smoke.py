"""Real container regression, run in Linux CI after building the image."""
from pathlib import Path
import subprocess
import server

def vertical(text):
    rows = text.splitlines()
    return "\n".join(" ".join(row[i] if i < len(row) else " " for row in rows) for i in range(max(map(len, rows), default=0))) + "\n"

for action in ["check", "fmt", "run"]:
    result = server.sandbox({"action": action, "source": vertical('FN main(): int { println("sandbox works")~ RET 0~ }')})
    assert result["status"] == 0, result
    if action == "run":
        assert result["stdout"] == "sandbox works\n", result
result = server.sandbox({"action": "run", "source": vertical("FN main(): int { WH true {} }")})
assert result["timed_out"], result
result = server.sandbox({"action": "run", "source": vertical('FN main(): int { println(read_file("/host/Cargo.toml"))~ RET 0~ }')})
assert result["status"] != 0 and "cannot open" in result["stderr"], result
print("Container execution, timeout, formatting and filesystem isolation: OK")
