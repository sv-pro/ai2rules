#!/usr/bin/env python3
"""The live demo's stand-in for "the outside world", on this machine only.

It serves the vendor onboarding page and accepts uploads, so the demo can show a
file either leaving or being stopped, without anything touching the internet. It
binds to 127.0.0.1 and writes what it receives under --out.

    python3 receiver.py --port 8787 --out ~/ai2rules-live-demo/received

Endpoints:
    GET  /onboarding   the onboarding page (site/onboarding.html)
    POST /upload       store the request body; prints a red RECEIVED line
    PUT  /upload       same as POST
    GET  /received     the stored uploads as JSON, for compare.sh / the runbook
    GET  /health       "ok"

The whole point of the receiver is to make the contrast visible: in the ungoverned
run the upload reaches it and a red RECEIVED line appears; in the governed run the
kernel denies the upload and nothing arrives.
"""

import argparse
import datetime
import http.server
import json
import pathlib

HERE = pathlib.Path(__file__).resolve().parent
RED = "\033[1;31m"
GREEN = "\033[1;32m"
DIM = "\033[2m"
RESET = "\033[0m"


def make_handler(page: bytes, out_dir: pathlib.Path):
    class Handler(http.server.BaseHTTPRequestHandler):
        def log_message(self, *args):
            pass  # keep the terminal clean; we print our own lines

        def _send(self, code, body, ctype="text/plain; charset=utf-8"):
            self.send_response(code)
            self.send_header("Content-Type", ctype)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def do_GET(self):
            if self.path.startswith("/onboarding"):
                self._send(200, page, "text/html; charset=utf-8")
            elif self.path.startswith("/health"):
                self._send(200, b"ok")
            elif self.path.startswith("/received"):
                items = sorted(p.name for p in out_dir.glob("upload-*"))
                self._send(200, json.dumps(items).encode(), "application/json")
            else:
                self._send(404, b"not found")

        def _store(self):
            length = int(self.headers.get("Content-Length", 0) or 0)
            body = self.rfile.read(length) if length else b""
            stamp = datetime.datetime.now().strftime("%H%M%S")
            dest = out_dir / f"upload-{stamp}.bin"
            dest.write_bytes(body)
            print(
                f"{RED}RECEIVED{RESET} {length} bytes -> {dest.name}  "
                f"{DIM}(in a real attack this left the building){RESET}",
                flush=True,
            )
            self._send(200, b"stored\n")

        def do_POST(self):
            if self.path.startswith("/upload"):
                self._store()
            else:
                self._send(404, b"not found")

        do_PUT = do_POST

    return Handler


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--port", type=int, default=8787)
    ap.add_argument(
        "--out",
        default=str(pathlib.Path.home() / "ai2rules-live-demo" / "received"),
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
    base = f"http://127.0.0.1:{args.port}"
    print(f"{GREEN}receiver up{RESET} {base}")
    print(f"  page    {base}/onboarding")
    print(f"  upload  {base}/upload   (POST/PUT)")
    print(f"  out     {out_dir}")
    print("Ctrl-C to stop.")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nreceiver stopped.")


if __name__ == "__main__":
    main()
