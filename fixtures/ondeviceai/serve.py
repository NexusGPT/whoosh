#!/usr/bin/env python3
"""A small file server for the install-step check: GET and HEAD, byte ranges, an optional rate limit.

    python3 fixtures/ondeviceai/serve.py <folder> [--port N] [--rate BYTES_PER_SECOND] [--log FILE]

Prints `PORT <n>` once it listens. The check starts it in the background, reads the pid, and stops it by that pid.
Standard library only; it binds 127.0.0.1 and nothing else.
"""
import argparse, os, re, sys, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ap = argparse.ArgumentParser()
ap.add_argument("folder"); ap.add_argument("--port", type=int, default=0); ap.add_argument("--rate", type=int, default=0); ap.add_argument("--log", default="")
A = ap.parse_args()
ROOT = os.path.realpath(A.folder)


class H(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *a): pass

    def _path(self):
        p = os.path.realpath(os.path.join(ROOT, self.path.split("?")[0].lstrip("/")))
        return p if p.startswith(ROOT + os.sep) and os.path.isfile(p) else None

    def _head(self, body=False):
        p = self._path()
        if A.log and body:
            open(A.log, "a").write("%s %s\n" % (self.path, self.headers.get("Range", "-")))
        if not p:
            self.send_response(404); self.send_header("Content-Length", "0"); self.end_headers(); return
        size = os.path.getsize(p); start, end, code = 0, size - 1, 200
        m = re.match(r"bytes=(\d+)-(\d*)$", self.headers.get("Range", ""))
        if m:
            start = int(m.group(1)); end = int(m.group(2)) if m.group(2) else size - 1
            if start >= size:
                self.send_response(416); self.send_header("Content-Range", f"bytes */{size}"); self.send_header("Content-Length", "0"); self.end_headers(); return
            code = 206
        self.send_response(code)
        self.send_header("Content-Length", str(end - start + 1)); self.send_header("Accept-Ranges", "bytes")
        if code == 206: self.send_header("Content-Range", f"bytes {start}-{end}/{size}")
        self.end_headers()
        if not body: return
        with open(p, "rb") as f:
            f.seek(start); left = end - start + 1; chunk = 64 * 1024
            while left > 0:
                data = f.read(min(chunk, left))
                try: self.wfile.write(data)
                except (BrokenPipeError, ConnectionResetError): return
                left -= len(data)
                if A.rate: time.sleep(len(data) / A.rate)

    def do_HEAD(self): self._head(False)
    def do_GET(self): self._head(True)


srv = ThreadingHTTPServer(("127.0.0.1", A.port), H)
print("PORT", srv.server_address[1], flush=True)
srv.serve_forever()
