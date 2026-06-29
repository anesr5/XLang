"""Minimal LSP smoke test for xlang-language-server."""
import subprocess
import sys
import time
from pathlib import Path

SERVER = Path(__file__).resolve().parents[1] / "target" / "debug" / "xlang-language-server.exe"
if not SERVER.exists():
    SERVER = Path(__file__).resolve().parents[1] / "target" / "release" / "xlang-language-server.exe"
if not SERVER.exists():
    print("error: build xlang-language-server first", file=sys.stderr)
    sys.exit(1)

SOURCE = """module main

i32 add(i32 a, i32 b) {
    return a + b;
}

i32 main() {
    i32 x = add(40, 2);
    return x;
}
"""


def send(proc, body: str) -> None:
    data = body.encode("utf-8")
    header = f"Content-Length: {len(data)}\r\n\r\n".encode("ascii")
    proc.stdin.write(header)
    proc.stdin.write(data)
    proc.stdin.flush()


proc = subprocess.Popen(
    [str(SERVER)],
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,
)

send(
    proc,
    '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{},"rootUri":null,"processId":null}}',
)
time.sleep(0.5)
send(proc, '{"jsonrpc":"2.0","method":"initialized","params":{}}')
time.sleep(0.2)
import json

open_msg = {
    "jsonrpc": "2.0",
    "method": "textDocument/didOpen",
    "params": {
        "textDocument": {
            "uri": "file:///main.x",
            "languageId": "xlang",
            "version": 1,
            "text": SOURCE,
        }
    },
}
send(proc, json.dumps(open_msg))
time.sleep(0.8)

proc.stdin.close()
out = proc.stdout.read(8192).decode("utf-8", errors="replace")
proc.kill()

checks = [
    ("capabilities", "capabilities" in out),
    ("semanticTokensProvider", "semanticTokensProvider" in out),
    ("xlang server name", "xlang-language-server" in out),
    ("publishDiagnostics", "publishDiagnostics" in out or "textDocument/publishDiagnostics" in out),
]

failed = [name for name, ok in checks if not ok]
if failed:
    print("SMOKE FAIL missing:", ", ".join(failed))
    print(out[:1000])
    sys.exit(1)

print("SMOKE OK — LSP initialize + didOpen + diagnostics")
