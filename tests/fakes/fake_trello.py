#!/usr/bin/env python3
"""Fake Trello REST API for agent-brain tests.

Usage: python3 -I tests/fakes/fake_trello.py STATE_DIR

Binds 127.0.0.1 on an ephemeral port and prints only the port number on the
first stdout line. Python standard library only; never contacts the network.

Files under STATE_DIR:
  data.json      initial or current state (lists, cards, comments)
  record.jsonl   one JSON object per request received (written by the fake)
  behavior.json  optional list of failure/delay rules:
                 {"path": PATTERN or "*", "times": N, "status": HTTP code,
                  "body": text, "delay_ms": ms}

With FAKE_TRELLO_STDIN_WATCH=1 the fake exits when its stdin reaches EOF.
"""
from __future__ import annotations

import json
import os
import re
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, unquote_plus, urlsplit

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


def take_rule(path_str: str) -> dict | None:
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
            target_path = rule.get("path", "*")
            if target_path != "*" and target_path not in path_str:
                continue
            times = rule.get("times", 0)
            if not isinstance(times, int) or times <= 0:
                continue
            rule["times"] = times - 1
            write_atomic(path, json.dumps(rules, ensure_ascii=False))
            return dict(rule)
    return None


def read_data() -> dict:
    with LOCK:
        path = state_path("data.json")
        try:
            with open(path, "r", encoding="utf-8") as f:
                return json.load(f)
        except (OSError, ValueError):
            return {"lists": [], "cards": [], "comments": {}}


def save_data(data: dict) -> None:
    with LOCK:
        write_atomic(state_path("data.json"), json.dumps(data, ensure_ascii=False, indent=2))


class Handler(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        pass

    def do_GET(self):
        self._handle("GET")

    def do_POST(self):
        self._handle("POST")

    def do_PUT(self):
        self._handle("PUT")

    def _handle(self, method: str):
        parts = urlsplit(self.path)
        path = parts.path
        query = parse_qs(parts.query, keep_blank_values=True)

        body = ""
        length = int(self.headers.get("Content-Length") or 0)
        if length > 0:
            body = self.rfile.read(length).decode("utf-8", errors="replace")

        append_record({
            "method": method,
            "path": path,
            "query": {k: v if len(v) > 1 else v[0] for k, v in query.items()},
            "body": body,
        })

        rule = take_rule(path)
        if rule:
            delay_ms = rule.get("delay_ms", 0)
            if delay_ms > 0:
                time.sleep(delay_ms / 1000.0)
            status = rule.get("status", 500)
            rule_body = rule.get("body", "Error from fake")
            resp_bytes = rule_body.encode("utf-8")
            self.send_response(status)
            self.send_header("Content-Type", "text/plain; charset=utf-8")
            self.send_header("Content-Length", str(len(resp_bytes)))
            self.end_headers()
            self.wfile.write(resp_bytes)
            return

        data = read_data()

        # Route matching
        # 1. GET /1/boards/{board}/lists
        m_lists = re.match(r"^/1/boards/([^/]+)/lists$", path)
        if method == "GET" and m_lists:
            resp = json.dumps(data.get("lists", [])).encode("utf-8")
            self._send_json(200, resp)
            return

        # 2. GET /1/boards/{board}/cards
        m_cards = re.match(r"^/1/boards/([^/]+)/cards$", path)
        if method == "GET" and m_cards:
            resp = json.dumps(data.get("cards", [])).encode("utf-8")
            self._send_json(200, resp)
            return

        # 3. GET /1/cards/{id}/actions
        m_actions = re.match(r"^/1/cards/([^/]+)/actions$", path)
        if method == "GET" and m_actions:
            card_id = m_actions.group(1)
            raw_comments = data.get("comments", {}).get(card_id, [])
            actions = []
            for c in raw_comments:
                actions.append({
                    "id": f"act_{len(actions)+1}",
                    "date": c.get("date", "2026-11-06T12:00:00.000Z"),
                    "memberCreator": {"fullName": c.get("author", "Tester")},
                    "data": {"text": c.get("text", "")},
                })
            resp = json.dumps(actions).encode("utf-8")
            self._send_json(200, resp)
            return

        # 4. POST /1/cards/{id}/actions/comments
        m_comment_post = re.match(r"^/1/cards/([^/]+)/actions/comments$", path)
        if method == "POST" and m_comment_post:
            card_id = m_comment_post.group(1)
            # parse body form: text=...
            form_params = parse_qs(body)
            comment_text = form_params.get("text", [""])[0]
            if not comment_text and "=" in body:
                comment_text = unquote_plus(body.split("=", 1)[1])

            comments = data.setdefault("comments", {})
            card_comments = comments.setdefault(card_id, [])
            card_comments.append({
                "date": time.strftime("%Y-%m-%dT%H:%M:%S.000Z", time.gmtime()),
                "author": "agent-brain",
                "text": comment_text,
            })
            save_data(data)
            resp = json.dumps({"id": f"comm_{len(card_comments)}", "text": comment_text}).encode("utf-8")
            self._send_json(200, resp)
            return

        # 5. PUT /1/cards/{id}
        m_card_put = re.match(r"^/1/cards/([^/]+)$", path)
        if method == "PUT" and m_card_put:
            card_id = m_card_put.group(1)
            new_list = query.get("idList", [""])[0]
            if not new_list and "idList=" in body:
                new_list = parse_qs(body).get("idList", [""])[0]

            cards = data.get("cards", [])
            for c in cards:
                if c.get("id") == card_id:
                    c["idList"] = new_list
            save_data(data)
            resp = json.dumps({"id": card_id, "idList": new_list}).encode("utf-8")
            self._send_json(200, resp)
            return

        # 6. GET /1/cards/{id}
        if method == "GET" and m_card_put:
            card_id = m_card_put.group(1)
            cards = data.get("cards", [])
            card = next((c for c in cards if c.get("id") == card_id), None)
            if card:
                resp = json.dumps({
                    "id": card["id"],
                    "idList": card.get("idList", ""),
                    "name": card.get("name", ""),
                    "closed": card.get("closed", False),
                }).encode("utf-8")
                self._send_json(200, resp)
            else:
                self._send_json(404, b'{"message": "Card not found"}')
            return

        # 7. GET /1/lists/{id}
        m_list_get = re.match(r"^/1/lists/([^/]+)$", path)
        if method == "GET" and m_list_get:
            list_id = m_list_get.group(1)
            lists = data.get("lists", [])
            l_obj = next((l for l in lists if l.get("id") == list_id), None)
            if l_obj:
                resp = json.dumps({
                    "id": l_obj["id"],
                    "name": l_obj.get("name", ""),
                    "closed": l_obj.get("closed", False),
                }).encode("utf-8")
                self._send_json(200, resp)
            else:
                self._send_json(404, b'{"message": "List not found"}')
            return

        self._send_json(404, b'{"message": "Not found in fake trello"}')

    def _send_json(self, status: int, body_bytes: bytes):
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body_bytes)))
        self.end_headers()
        self.wfile.write(body_bytes)


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
        sys.stderr.write("Usage: fake_trello.py STATE_DIR\n")
        sys.exit(2)

    STATE_DIR = sys.argv[1]
    os.makedirs(STATE_DIR, exist_ok=True)

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    port = server.server_address[1]

    # First line printed to stdout must be only the port number
    sys.stdout.write(f"{port}\n")
    sys.stdout.flush()

    if os.environ.get("FAKE_TRELLO_STDIN_WATCH") == "1":
        t = threading.Thread(target=watch_stdin, daemon=True)
        t.start()

    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
