#!/usr/bin/env python3
# server.py
import argparse, json, sys, threading, time
from http.server import ThreadingHTTPServer, BaseHTTPRequestHandler
from urllib.parse import parse_qs

LOCK = threading.Lock()

def now_iso():
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())

class LogHandler(BaseHTTPRequestHandler):
    server_version = "PostLogger/1.0"

    # CORS preflight (optional)
    def do_OPTIONS(self):
        # Status line must come before headers in BaseHTTPRequestHandler
        self.send_response(204)
        self._cors()
        self.end_headers()

    def do_GET(self):
        # Simple JSON API endpoints for local testing
        path = self.path.split("?")[0]
        if path == "/api/hello":
            payload = {
                "ok": True,
                "ts": now_iso(),
                "message": "Hello from the Rune example server",
            }
        elif path == "/api/ir-diff":
            # Minimal sample diff: replace text in the address bar InputBox
            # Using a widget target guarantees the mutation applies regardless of document snapshot timing.
            payload = {
                "type": "ir_diff",
                "ops": [
                    {
                        "op": "replace_text",
                        "target": "widget:InputBox",
                        "text": "Updated via /api/ir-diff",
                    }
                ],
            }
        else:
            payload = {"ok": True, "ts": now_iso(), "path": self.path}

        self.send_response(200)
        self._cors()
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.end_headers()
        self.wfile.write(json.dumps(payload).encode("utf-8"))

    def do_POST(self):
        max_bytes = self.server.max_bytes
        content_length = int(self.headers.get("Content-Length", "0") or 0)
        if content_length > max_bytes:
            self.send_response(413)  # Payload Too Large
            self._cors()
            self.end_headers()
            return

        body = self.rfile.read(content_length) if content_length else b""
        ctype = (self.headers.get("Content-Type") or "").split(";")[0].strip().lower()

        payload = None
        parse_error = None
        try:
            if ctype == "application/json":
                payload = json.loads(body.decode("utf-8") or "null")
            elif ctype == "application/x-www-form-urlencoded":
                payload = {k: v if len(v) > 1 else v[0] for k, v in parse_qs(body.decode("utf-8")).items()}
            else:
                # fall back to raw text; try utf-8 then base64ish safe repr
                try:
                    payload = {"text": body.decode("utf-8")}
                except UnicodeDecodeError:
                    payload = {"bytes_hex": body.hex()}
        except Exception as e:
            parse_error = str(e)

        record = {
            "ts": now_iso(),
            "remote_addr": self.client_address[0],
            "path": self.path,
            "content_type": ctype or None,
            "headers": {k: v for k, v in self.headers.items()},
            "payload": payload,
            "parse_error": parse_error,
        }

        line = json.dumps(record, ensure_ascii=False)
        with LOCK:
            with open(self.server.logfile, "a", encoding="utf-8") as f:
                f.write(line + "\n")

        self.send_response(200)
        self._cors()
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.end_headers()
        self.wfile.write(b'{"status":"ok"}')

    # common CORS headers (optional; safe to remove if not needed)
    def _cors(self):
        # Should be called after send_response(...)
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Headers", "Content-Type, Authorization")
        self.send_header("Access-Control-Allow-Methods", "POST, GET, OPTIONS")

def main():
    p = argparse.ArgumentParser(description="Minimal POST logger (no deps).")
    p.add_argument("--host", default="0.0.0.0", help="Bind host (default: 0.0.0.0)")
    p.add_argument("--port", type=int, default=3000, help="Port (default: 3000)")
    p.add_argument("--logfile", default="post_log.jsonl", help="Output JSONL file")
    p.add_argument("--max-bytes", type=int, default=5 * 1024 * 1024, help="Max payload size (bytes)")
    args = p.parse_args()

    httpd = ThreadingHTTPServer((args.host, args.port), LogHandler)
    httpd.logfile = args.logfile
    httpd.max_bytes = args.max_bytes

    print(f"Listening on http://{args.host}:{args.port} → logging to {args.logfile}")
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        print("\nShutting down…", file=sys.stderr)
        httpd.server_close()

if __name__ == "__main__":
    main()
