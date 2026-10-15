#!/usr/bin/env python3
"""Redact sensitive data from spike output, in place.

Usage: python3 -I spikes/lib/scrub.py PATH [PATH...]   (files or directories)

stdlib only. .json/.jsonl files are handled JSON-aware (every string value is walked and the
line is re-serialised with ensure_ascii=False, so fixtures stay valid JSON); a line that is not
valid JSON is scrubbed as plain text and never crashes the script. Everything else is plain text.
Prints only per-file replacement counts, never a value.
"""
import json
import os
import re
import sys

HOME = os.environ.get("HOME", "")

# (compiled pattern, replacement). Order matters: secrets first, then emails, then paths.
RULES = [
    (re.compile(r"Bearer\s+[A-Za-z0-9._~+/=-]+"), "Bearer <redacted>"),
    (re.compile(r"eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+"), "<redacted-jwt>"),
    (re.compile(r"sk-ant-[A-Za-z0-9_-]+"), "<redacted-key>"),
    (re.compile(r"sk-[A-Za-z0-9_-]{16,}"), "<redacted-key>"),
    (re.compile(r"ghp_[A-Za-z0-9]{20,}"), "<redacted-gh>"),
    (re.compile(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}"), "<email>"),
]
# key=value / "key": "value" in plain text, where the key looks sensitive and the value is a long blob
KV_RULE = re.compile(
    r"""((?:token|key|secret|password)[A-Za-z_]*["']?\s*[:=]\s*["']?)[A-Za-z0-9+/=_-]{32,}""",
    re.IGNORECASE,
)
SENSITIVE_KEY = re.compile(r"token|key|secret|password", re.IGNORECASE)
BLOB = re.compile(r"[A-Za-z0-9+/=_-]{32,}")
HOME_OTHER = re.compile(r"/home/[A-Za-z0-9._-]+")
SKIP_DIRS = {"work", "scratch", "__pycache__", ".git"}

count = 0


def scrub_text(s):
    """Return s with all plain-text rules applied; bumps the global counter."""
    global count
    out = s
    for pat, repl in RULES:
        out, n = pat.subn(repl, out)
        count += n
    out, n = KV_RULE.subn(lambda m: m.group(1) + "<redacted>", out)
    count += n
    if HOME and HOME != "/":
        n = out.count(HOME)
        if n:
            out = out.replace(HOME, "~")
            count += n
    # any other /home/<user>; "/home/USER" maps to itself so reruns are stable
    def other(m):
        global count
        if m.group(0) == "/home/USER":
            return m.group(0)
        count += 1
        return "/home/USER"
    out = HOME_OTHER.sub(other, out)
    return out


def walk(obj, key=None):
    global count
    if isinstance(obj, str):
        if key is not None and SENSITIVE_KEY.search(key) and BLOB.fullmatch(obj):
            count += 1
            return "<redacted>"
        return scrub_text(obj)
    if isinstance(obj, list):
        return [walk(x, key) for x in obj]
    if isinstance(obj, dict):
        return {scrub_text(k) if isinstance(k, str) else k: walk(v, k) for k, v in obj.items()}
    return obj


def dumps_line(obj):
    return json.dumps(obj, ensure_ascii=False, separators=(",", ":"))


def scrub_json_line(line):
    """Scrub one line; JSON-aware when it parses, plain text otherwise. Keeps the original
    bytes when nothing changed so fixtures are not reformatted needlessly."""
    global count
    stripped = line.strip()
    if not stripped:
        return line
    before = count
    try:
        obj = json.loads(stripped)
    except ValueError:
        return scrub_text(line)
    new = walk(obj)
    if count == before:
        return line
    nl = "\n" if line.endswith("\n") else ""
    return dumps_line(new) + nl


def scrub_file(path):
    global count
    count = 0
    with open(path, "rb") as fh:
        raw = fh.read()
    text = raw.decode("utf-8", errors="surrogateescape")
    low = path.lower()
    if low.endswith(".json") and "\n" in text.strip():
        # a pretty-printed whole-file document?
        try:
            obj = json.loads(text)
        except ValueError:
            obj = None
        if obj is not None:
            before = count
            new = walk(obj)
            out = text if count == before else json.dumps(new, ensure_ascii=False, indent=2) + "\n"
        else:
            out = "".join(scrub_json_line(ln) for ln in text.splitlines(keepends=True))
    elif low.endswith(".json") or low.endswith(".jsonl"):
        out = "".join(scrub_json_line(ln) for ln in text.splitlines(keepends=True))
    else:
        out = "".join(scrub_text(ln) for ln in text.splitlines(keepends=True))
    if out != text:
        with open(path, "wb") as fh:
            fh.write(out.encode("utf-8", errors="surrogateescape"))
    return count


def iter_files(arg):
    if os.path.isfile(arg):
        yield arg
    elif os.path.isdir(arg):
        for root, dirs, files in os.walk(arg):
            dirs[:] = [d for d in dirs if d not in SKIP_DIRS]
            for name in sorted(files):
                yield os.path.join(root, name)
    else:
        print("scrub: skip (not found): " + arg, file=sys.stderr)


def main(argv):
    if not argv:
        print("usage: scrub.py PATH...", file=sys.stderr)
        return 2
    for arg in argv:
        for path in iter_files(arg):
            try:
                n = scrub_file(path)
            except OSError as exc:
                print("scrub: cannot process %s: %s" % (path, exc.__class__.__name__), file=sys.stderr)
                continue
            print("scrub %s: %d replacement(s)" % (path, n))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
