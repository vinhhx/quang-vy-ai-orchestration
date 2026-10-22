#!/usr/bin/env python3
"""Fake Telegram Bot API for agent-brain tests (Phase 05.3, D-17).

Usage: python3 -I tests/fakes/fake_telegram.py STATE_DIR

Binds 127.0.0.1 on an ephemeral port and prints only the port number on the
first stdout line. Python standard library only; never contacts the network.

Files under STATE_DIR:
  updates.jsonl  tests append Update objects (one JSON object per line)
  record.jsonl   one JSON object per request received (written by the fake)
  behavior.json  optional list of failure/delay rules:
                 {"method": NAME or "*", "times": N, "status": HTTP code,
                  "description": text, "retry_after": seconds, "delay_ms": ms}

With FAKE_TELEGRAM_STDIN_WATCH=1 the fake exits when its stdin reaches EOF
(the test driver keeps stdin open as a pipe; EOF means the test went away).
"""
from __future__ import annotations

import json
import os
import re
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlsplit

STATE_DIR = ""
LOCK = threading.Lock()
STATE = {"next_message_id": 100, "forgotten_below": 0}

PATH_RE = re.compile(r"^/bot([^/]+)/([A-Za-z]+)$")
PARAM_RE = re.compile(r';\s*([A-Za-z0-9_*-]+)\s*=\s*("(?:[^"\\]|\\.)*"|[^;]*)')


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


def take_rule(method: str) -> dict | None:
    """First rule matching the method with times > 0; decrement and rewrite."""
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
            if rule.get("method") not in (method, "*"):
                continue
            times = rule.get("times", 0)
            if not isinstance(times, int) or times <= 0:
                continue
            rule["times"] = times - 1
            write_atomic(path, json.dumps(rules, ensure_ascii=False))
            return dict(rule)
    return None


def read_updates() -> list[dict]:
    out = []
    try:
        with open(state_path("updates.jsonl"), "r", encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                try:
                    obj = json.loads(line)
                except ValueError:
                    continue
                if isinstance(obj, dict) and isinstance(obj.get("update_id"), int):
                    out.append(obj)
    except OSError:
        pass
    return out


def next_message_id() -> int:
    with LOCK:
        mid = STATE["next_message_id"]
        STATE["next_message_id"] = mid + 1
        return mid


def unquote_param(v: str) -> str:
    v = v.strip()
    if len(v) >= 2 and v[0] == '"' and v[-1] == '"':
        return re.sub(r"\\(.)", r"\1", v[1:-1])
    return v


def parse_multipart(body: bytes, content_type: str) -> dict:
    m = re.search(r'boundary=("?)([^";]+)\1', content_type)
    if not m:
        return {}
    boundary = b"--" + m.group(2).encode("latin-1")
    result: dict = {}
    chunks = body.split(boundary)
    for chunk in chunks[1:]:
        if chunk.startswith(b"--"):
            break
        if chunk.startswith(b"\r\n"):
            chunk = chunk[2:]
        if chunk.endswith(b"\r\n"):
            chunk = chunk[:-2]
        head, sep, content = chunk.partition(b"\r\n\r\n")
        if not sep:
            continue
        headers = {}
        for line in head.decode("utf-8", "replace").split("\r\n"):
            k, _, v = line.partition(":")
            headers[k.strip().lower()] = v.strip()
        disp = headers.get("content-disposition", "")
        params = {k.lower(): unquote_param(v) for k, v in PARAM_RE.findall(disp)}
        name = params.get("name")
        if name is None:
            continue
        if "filename" in params:
            result[name] = {
                "filename": params["filename"],
                "content_type": headers.get("content-type", ""),
                "content": content.decode("utf-8", "replace"),
            }
        else:
            result[name] = content.decode("utf-8", "replace")
    return result


def get_updates(params: dict) -> list[dict]:
    try:
        offset = int(params.get("offset", 0) or 0)
    except (TypeError, ValueError):
        offset = 0
    try:
        timeout = float(params.get("timeout", 0) or 0)
    except (TypeError, ValueError):
        timeout = 0.0
    allowed = params.get("allowed_updates")
    if not isinstance(allowed, list):
        allowed = None

    def wanted(u: dict) -> bool:
        if allowed is None:
            return True
        return any(k in u for k in allowed)

    if offset < 0:
        ups = read_updates()
        if not ups:
            return []
        newest = max(ups, key=lambda u: u["update_id"])
        with LOCK:
            STATE["forgotten_below"] = newest["update_id"]
        return [newest] if wanted(newest) else []
    if offset > 0:
        with LOCK:
            STATE["forgotten_below"] = max(STATE["forgotten_below"], offset)

    deadline = time.time() + max(0.0, min(timeout, 1.0))
    while True:
        with LOCK:
            below = STATE["forgotten_below"]
        found = [u for u in read_updates() if u["update_id"] >= below and wanted(u)]
        if found or time.time() >= deadline:
            return sorted(found, key=lambda u: u["update_id"])
        time.sleep(0.05)


def chat_of(params: dict) -> dict:
    cid = params.get("chat_id")
    try:
        cid = int(cid)
    except (TypeError, ValueError):
        pass
    return {"id": cid, "type": "private"}


def result_for(method: str, params: dict):
    """Returns (http_status, response_dict)."""
    if method == "getMe":
        return 200, {
            "ok": True,
            "result": {
                "id": 424242,
                "is_bot": True,
                "first_name": "AB fake",
                "username": "ab_fake_bot",
            },
        }
    if method == "getUpdates":
        return 200, {"ok": True, "result": get_updates(params)}
    if method == "sendMessage":
        return 200, {
            "ok": True,
            "result": {
                "message_id": next_message_id(),
                "date": int(time.time()),
                "chat": chat_of(params),
                "text": params.get("text", ""),
            },
        }
    if method == "sendDocument":
        doc = params.get("document")
        fname = doc.get("filename") if isinstance(doc, dict) else None
        return 200, {
            "ok": True,
            "result": {
                "message_id": next_message_id(),
                "date": int(time.time()),
                "chat": chat_of(params),
                "document": {"file_name": fname},
            },
        }
    if method == "editMessageText":
        return 200, {
            "ok": True,
            "result": {
                "message_id": params.get("message_id"),
                "chat": chat_of(params),
                "text": params.get("text", ""),
            },
        }
    if method == "editMessageReplyMarkup":
        return 200, {
            "ok": True,
            "result": {"message_id": params.get("message_id"), "chat": chat_of(params)},
        }
    if method in ("answerCallbackQuery", "deleteWebhook", "setMyCommands"):
        return 200, {"ok": True, "result": True}
    return 404, {"ok": False, "error_code": 404, "description": "Not Found"}


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, fmt, *args):  # quiet
        pass

    def send_json(self, status: int, obj: dict) -> None:
        data = json.dumps(obj, ensure_ascii=False).encode("utf-8")
        try:
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)
            self.wfile.flush()
        except (BrokenPipeError, ConnectionResetError, OSError):
            pass

    def handle_any(self) -> None:
        parts = urlsplit(self.path)
        length = int(self.headers.get("Content-Length") or 0)
        body = self.rfile.read(length) if length > 0 else b""
        m = PATH_RE.match(parts.path)
        if not m:
            self.send_json(404, {"ok": False, "error_code": 404, "description": "Not Found"})
            return
        token, method = m.group(1), m.group(2)
        ctype = self.headers.get("Content-Type") or ""
        json_body = None
        form_body = None
        if ctype.startswith("application/json"):
            try:
                json_body = json.loads(body.decode("utf-8") or "{}")
            except ValueError:
                json_body = None
        elif ctype.startswith("multipart/form-data"):
            form_body = parse_multipart(body, ctype)
        elif parts.query:
            json_body = {k: v[-1] for k, v in parse_qs(parts.query).items()}

        params = json_body if isinstance(json_body, dict) else (form_body or {})

        rule = take_rule(method)
        if rule and rule.get("delay_ms"):
            time.sleep(float(rule["delay_ms"]) / 1000.0)
        if rule and rule.get("status"):
            status = int(rule["status"])
            resp = {
                "ok": False,
                "error_code": status,
                "description": rule.get("description", "Error"),
            }
            if rule.get("retry_after") is not None:
                resp["parameters"] = {"retry_after": rule["retry_after"]}
        else:
            status, resp = result_for(method, params)

        append_record(
            {
                "t": time.time(),
                "method": method,
                "token": token,
                "json": json_body,
                "form": form_body,
                "status": status,
                "response": resp,
            }
        )
        self.send_json(status, resp)

    def do_POST(self):
        self.handle_any()

    def do_GET(self):
        self.handle_any()


def watch_stdin() -> None:
    try:
        while sys.stdin.buffer.read(4096):
            pass
    except Exception:
        pass
    os._exit(0)


def main() -> int:
    global STATE_DIR
    if len(sys.argv) != 2:
        print("usage: fake_telegram.py STATE_DIR", file=sys.stderr)
        return 2
    STATE_DIR = os.path.abspath(sys.argv[1])
    os.makedirs(STATE_DIR, exist_ok=True)
    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    server.daemon_threads = True
    print(server.server_address[1], flush=True)
    if os.environ.get("FAKE_TELEGRAM_STDIN_WATCH") == "1":
        threading.Thread(target=watch_stdin, daemon=True).start()
    try:
        server.serve_forever(poll_interval=0.1)
    except KeyboardInterrupt:
        pass
    return 0


if __name__ == "__main__":
    sys.exit(main())
