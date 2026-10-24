#!/usr/bin/env python3
"""The live demo's stand-in for "the outside world", on this machine only.

It serves the vendor onboarding page and accepts uploads, so the demo can show a
file either leaving or being stopped, without anything touching the internet. It
binds to 127.0.0.1, writes what it receives under --out, and never forwards
anything anywhere: it is the destination, not a proxy.

    python3 receiver.py --port 8787 --out ~/northwind-inbox

The demo addresses it by name, so the audience sees ordinary-looking sites rather
than 127.0.0.1. Both names live under `.test`, which is reserved (RFC 6761) and
never resolves on the internet, and point here through the hosts file (hosts.txt
holds the two lines). Without those lines the names do not resolve at all, so a
missing entry fails closed. 127.0.0.1 keeps working either way.

    portal.northwind.test   the site the assistant reads (GET /onboarding)
    files.northwind.test    where the data goes (POST /upload)

Endpoints, on any of those names:
    GET  /onboarding   the onboarding page (site/onboarding.html)
    POST /upload       store the request body; prints a red RECEIVED line
    PUT  /upload       same as POST
    GET  /received     the stored uploads as JSON
    GET  /health       "ok" (preflight.sh uses it)

Every request is logged with the name it was addressed to, so the terminal is the
record of which sites each window actually reached: in the ungoverned run the
upload arrives and a red RECEIVED line appears; in the governed run the kernel
denies it and the upload never shows up.
"""

import argparse
import datetime
import http.server
import json
import pathlib
import re

HERE = pathlib.Path(__file__).resolve().parent
PORTAL = "portal.northwind.test"
FILES = "files.northwind.test"
RED = "\033[1;31m"
GREEN = "\033[1;32m"
DIM = "\033[2m"
RESET = "\033[0m"


def make_handler(page: bytes, out_dir: pathlib.Path):
    class Handler(http.server.BaseHTTPRequestHandler):
        def log_message(self, *args):
            pass  # keep the terminal clean; we print our own lines

        def _host(self):
            # The name the client addressed, without the port.
            return (self.headers.get("Host") or "-").rsplit(":", 1)[0]

        def _log(self, what, colour=""):
            now = datetime.datetime.now().strftime("%H:%M:%S")
            target = f"{self._host()}{self.path}"
            line = f"{now}  {self.command:<4} {target:<40} {what}"
            print(f"{colour}{line}{RESET}" if colour else line, flush=True)

        def _send(self, code, body, ctype="text/plain; charset=utf-8"):
            self.send_response(code)
            self.send_header("Content-Type", ctype)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def do_GET(self):
            if self.path.startswith("/onboarding"):
                self._log("page served")
                self._send(200, page, "text/html; charset=utf-8")
            elif self.path.startswith("/health"):
                self._log("ok", DIM)
                self._send(200, b"ok")
            elif self.path.startswith("/received"):
                items = sorted(p.name for p in out_dir.glob("upload-*"))
                self._log(f"{len(items)} upload(s) listed", DIM)
                self._send(200, json.dumps(items).encode(), "application/json")
            else:
                self._log("not found", DIM)
                self._send(404, b"not found")

        def _store(self):
            length = int(self.headers.get("Content-Length", 0) or 0)
            body = self.rfile.read(length) if length else b""
            stamp = datetime.datetime.now().strftime("%H%M%S")
            host = re.sub(r"[^A-Za-z0-9.-]", "_", self._host())
            dest = out_dir / f"upload-{stamp}-{host}.bin"
            dest.write_bytes(body)
            self._log(
                f"RECEIVED {length} bytes -> {dest.name}  "
                f"(in a real attack this left the building)",
                RED,
            )
            self._send(200, b"stored\n")

        def do_POST(self):
            if self.path.startswith("/upload"):
                self._store()
            else:
                self._log("not found", DIM)
                self._send(404, b"not found")

        do_PUT = do_POST

    return Handler


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--port", type=int, default=8787)
    ap.add_argument(
        "--out",
        default=str(pathlib.Path.home() / "northwind-inbox"),
        help="where received uploads are written",
    )
    ap.add_argument(
        "--page",
        default=str(HERE / "site" / "onboarding.html"),
        help="the onboarding page to serve",
    )
    args = ap.parse_args()

    out_dir = pathlib.Path(args.out).expanduser()
    out_dir.mkdir(parents=True, exist_ok=True)
    page = pathlib.Path(args.page).expanduser().read_bytes()

    handler = make_handler(page, out_dir)
    # 127.0.0.1 only: the receiver must never be reachable off this machine.
    server = http.server.HTTPServer(("127.0.0.1", args.port), handler)
    port = args.port
    print(f"{GREEN}receiver up{RESET} on 127.0.0.1:{port}, forwarding nothing")
    print(f"  page    http://{PORTAL}:{port}/onboarding")
    print(f"  upload  http://{FILES}:{port}/upload   (POST/PUT)")
    print(f"  out     {out_dir}")
    print("Every request is logged below, with the name it was sent to. Ctrl-C to stop.")
    print()
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nreceiver stopped.")


if __name__ == "__main__":
    main()
