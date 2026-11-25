#!/usr/bin/env python3
"""Serve built Showcase assets on loopback only, without stale browser caching."""

import argparse
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


class LocalAssets(SimpleHTTPRequestHandler):
    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        super().end_headers()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, default=8788)
    args = parser.parse_args()
    if not 1 <= args.port <= 65535:
        parser.error("port must be in 1..65535")
    assets = Path(__file__).resolve().parents[2] / "target" / "web-showcase"
    required = ("index.html", "halcyon_showcase.js", "halcyon_showcase_bg.wasm")
    missing = [name for name in required if not (assets / name).is_file()]
    if missing:
        parser.error("build the Showcase assets first; missing: " + ", ".join(missing))
    handler = partial(LocalAssets, directory=str(assets))
    with ThreadingHTTPServer(("127.0.0.1", args.port), handler) as server:
        print(f"Halcyon Web Showcase: http://127.0.0.1:{args.port}/", flush=True)
        server.serve_forever()


if __name__ == "__main__":
    main()
