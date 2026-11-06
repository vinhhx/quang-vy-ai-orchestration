#!/usr/bin/env python3
"""Fake Google Sheet CSV exporter for agent-brain tests.

Usage: python3 -I tests/fakes/fake_gsheet.py STATE_DIR

Binds 127.0.0.1 on an ephemeral port and prints only the port number on the
first stdout line. Python standard library only; never contacts the network.

Files under STATE_DIR:
  sheet.csv      CSV content to return
  record.jsonl   one JSON object per request received (written by the fake)
  behavior.json  optional list of failure/delay rules:
                 {"times": N, "status": HTTP code, "body": text, "delay_ms": ms}

With FAKE_GSHEET_STDIN_WATCH=1 the fake exits when its stdin reaches EOF.
"""
from __future__ import annotations

import json
import os
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlsplit

STATE_DIR = ""
LOCK = threading.Lock()


def state_path(name: str) -> str:
    return os.path.join(STATE_DIR, name)


def write_atomic(path: str, text: str) -> None:
    tmp = f"{path}.{os.getpid()}.{threading.get_ident()}.tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        f.write(text)
        f.flush()
    os.replace(tmp, path)


def append_record(rec: dict) -> None:
    with LOCK:
        with open(state_path("record.jsonl"), "a", encoding="utf-8") as f:
            f.write(json.dumps(rec, ensure_ascii=False) + "\n")
            f.flush()


def take_rule() -> dict | None:
    path = state_path("behavior.json")
    with LOCK:
        try:
            with open(path, "r", encoding="utf-8") as f:
                rules = json.load(f)
        except (OSError, ValueError):
            return None
        if not isinstance(rules, list):
            return None
        for rule in rules:
            if not isinstance(rule, dict):
                continue
            times = rule.get("times", 0)
            if not isinstance(times, int) or times <= 0:
                continue
            rule["times"] = times - 1
            write_atomic(path, json.dumps(rules, ensure_ascii=False))
            return dict(rule)
    return None


def read_sheet_csv() -> bytes:
    with LOCK:
        path = state_path("sheet.csv")
        try:
            with open(path, "rb") as f:
                return f.read()
        except OSError:
            return b""


class Handler(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        pass

    def do_GET(self):
        parts = urlsplit(self.path)
        path = parts.path
        query = parse_qs(parts.query, keep_blank_values=True)

        append_record({
            "method": "GET",
            "path": path,
            "query": {k: v if len(v) > 1 else v[0] for k, v in query.items()},
        })

        rule = take_rule()
        if rule:
            delay_ms = rule.get("delay_ms", 0)
            if delay_ms > 0:
                time.sleep(delay_ms / 1000.0)
            status = rule.get("status", 500)
            body = rule.get("body", "Error from fake gsheet").encode("utf-8")
            self.send_response(status)
            self.send_header("Content-Type", "text/plain; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return

        csv_bytes = read_sheet_csv()
        self.send_response(200)
        self.send_header("Content-Type", "text/csv; charset=utf-8")
        self.send_header("Content-Length", str(len(csv_bytes)))
        self.end_headers()
        self.wfile.write(csv_bytes)


def watch_stdin():
    try:
        while sys.stdin.read(1):
            pass
    except Exception:
        pass
    os._exit(0)


def main():
    global STATE_DIR
    if len(sys.argv) < 2:
        sys.stderr.write("Usage: fake_gsheet.py STATE_DIR\n")
        sys.exit(2)

    STATE_DIR = sys.argv[1]
    os.makedirs(STATE_DIR, exist_ok=True)

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    port = server.server_address[1]

    sys.stdout.write(f"{port}\n")
    sys.stdout.flush()

    if os.environ.get("FAKE_GSHEET_STDIN_WATCH") == "1":
        t = threading.Thread(target=watch_stdin, daemon=True)
        t.start()

    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
