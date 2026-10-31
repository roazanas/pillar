# Local Pillar playground

Build the container from the repository root, then start the loopback server:

```sh
docker build -f playground/Dockerfile -t pillar-playground:local .
python3 playground/server.py
```

Open http://127.0.0.1:8765. Python 3.11+ and a running Docker engine are required.
`python3 playground/server.py --preview-only` opens the editor without executing
code. It does not substitute host execution when Docker is unavailable.

The editor has vertical insertion/deletion, column insert/remove/move, undo/redo,
horizontal import/preview, source download, input and output. Enter starts a new
column in downward mode; Tab moves focus. Source is sent only to the local server.

Every check, format and run request uses a new unprivileged container. Constraints:
no network or host mounts, read-only root, all capabilities dropped, no new
privileges, 512 MiB RAM/swap cap, one CPU, 32 processes, bounded tmpfs/file sizes,
15-second host deadline, 8-second compile/format deadline and 2-second program
limit. Output is limited to 64 KiB per stream; only two requests execute concurrently.
The server validates Host/Origin, limits JSON bodies and serves a fixed asset list.

This is a local development sandbox, not a multi-tenant hosted execution service.
A public service needs a separate threat model, authentication, scheduling,
operational isolation and deployment review. The server binds only to loopback.

Run HTTP/constraint unit tests with:

```sh
python3 -m unittest discover -s playground -p 'test_*.py'
python3 playground/smoke.py  # requires the built image and engine
```
