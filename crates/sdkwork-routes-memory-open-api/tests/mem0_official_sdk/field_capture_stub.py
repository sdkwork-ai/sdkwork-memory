"""Capture stub for the mem0 field-coverage probe.

Records every request an official client sends and answers a permissive 2xx, so
the *request* can be read off the wire without a working service behind it. The
audit question is "which keys does this client put on the wire", and that is a
property of the client, not of any server.

Usage:
    python field_capture_stub.py <listen_port> <log_path>

`/v1/ping/` answers with `status: "ok"` because the JavaScript client's `ping()`
requires it (see the top of `acceptance.mjs`); everything else gets a shallow
200 carrying the keys both clients read back.
"""
import json
import sys
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

PORT = int(sys.argv[1])
LOG = sys.argv[2]

PING = {
    "org_id": "probe-org",
    "project_id": "probe-project",
    "user_email": "probe@example.com",
    "status": "ok",
}
GENERIC = {
    "message": "probe-stub",
    "status": "ok",
    "count": 1,
    "next": None,
    "previous": None,
    "results": [
        {
            "id": "probe-memory-id",
            "memory": "probe memory",
            "event": "ADD",
            "user_id": "probe-user",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z",
        }
    ],
}

lock = threading.Lock()
seq = [0]


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *args):
        pass

    def _handle(self):
        length = int(self.headers.get("content-length") or 0)
        body = self.rfile.read(length) if length else b""
        with lock:
            seq[0] += 1
            record = {
                "seq": seq[0],
                "method": self.command,
                "path": self.path,
                "content_type": self.headers.get("content-type"),
                "body": body.decode("utf-8", "replace"),
            }
            with open(LOG, "a", encoding="utf-8") as handle:
                handle.write(json.dumps(record, ensure_ascii=False) + "\n")

        payload = json.dumps(
            PING if self.path.split("?")[0] == "/v1/ping/" else GENERIC,
            ensure_ascii=False,
        ).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(payload)

    do_GET = _handle
    do_POST = _handle
    do_PUT = _handle
    do_DELETE = _handle
    do_PATCH = _handle


if __name__ == "__main__":
    srv = ThreadingHTTPServer(("127.0.0.1", PORT), Handler)
    print(f"STUB_LISTENING 127.0.0.1:{PORT} -> {LOG}", flush=True)
    srv.serve_forever()
