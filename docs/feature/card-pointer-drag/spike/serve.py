#!/usr/bin/env python3
"""Serve the spike probes and collect ?beacon=1 log lines POSTed to /log into probe-log.txt."""
import http.server, sys, time

LOG = "probe-log.txt"

class Handler(http.server.SimpleHTTPRequestHandler):
    def do_POST(self):
        if self.path != "/log":
            self.send_error(404); return
        body = self.rfile.read(int(self.headers.get("Content-Length", 0))).decode("utf-8", "replace")
        ua = self.headers.get("User-Agent", "")
        with open(LOG, "a") as f:
            f.write(f"{time.strftime('%H:%M:%S')} {self.client_address[0]} {body}\n")
        self.send_response(204); self.end_headers()
    def end_headers(self):
        self.send_header("Cache-Control", "no-store"); super().end_headers()
    def log_message(self, *a): pass

port = int(sys.argv[1]) if len(sys.argv) > 1 else 8000
http.server.ThreadingHTTPServer(("0.0.0.0", port), Handler).serve_forever()
