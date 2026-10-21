#!/usr/bin/env python3
"""Drives a command under a real pty to check a secret prompt (Phase 05.3 plan 07).

Usage:
  python3 -I tests/fakes/pty_secret.py [--sigint-at-prompt] SECRET -- CMD ARGS...

The child runs CMD on a new pty (environment inherited). The parent reads the
pty until it sees `Token bot` (10 s at most). By default it then types SECRET
and a newline, keeps reading until it sees `Mở Telegram` or 10 s pass, and
sends SIGINT to the child. With --sigint-at-prompt it sends SIGINT while the
prompt is still waiting, without typing anything.

Prints one JSON line:
  {"output": everything read from the pty (decoded with replacement),
   "exit": exit code, or 128 + signal,
   "echo": whether the pty has ECHO on after the child exited (null if unknown)}

Python standard library only; never contacts the network.
"""
from __future__ import annotations

import json
import os
import pty
import select
import signal
import sys
import termios
import time


def usage() -> int:
    print(
        "usage: pty_secret.py [--sigint-at-prompt] SECRET -- CMD ARGS...",
        file=sys.stderr,
    )
    return 2


def main() -> int:
    args = sys.argv[1:]
    at_prompt = False
    if args and args[0] == "--sigint-at-prompt":
        at_prompt = True
        args = args[1:]
    if len(args) < 3 or args[1] != "--":
        return usage()
    secret = args[0]
    cmd = args[2:]

    pid, fd = pty.fork()
    if pid == 0:
        try:
            os.execvp(cmd[0], cmd)
        except OSError:
            pass
        os._exit(127)

    buf = bytearray()
    eof = False

    def pump(timeout: float) -> None:
        nonlocal eof
        if eof:
            return
        ready, _, _ = select.select([fd], [], [], max(0.0, timeout))
        if not ready:
            return
        try:
            data = os.read(fd, 4096)
        except OSError:
            data = b""
        if not data:
            eof = True
            return
        buf.extend(data)

    def read_until(marker: bytes, limit: float) -> bool:
        deadline = time.monotonic() + limit
        while marker not in buf and not eof:
            left = deadline - time.monotonic()
            if left <= 0:
                break
            pump(min(left, 0.1))
        return marker in buf

    if read_until("Token bot".encode("utf-8"), 10.0):
        if at_prompt:
            # Give the child time to block in read() with echo off.
            end = time.monotonic() + 0.3
            while time.monotonic() < end and not eof:
                pump(0.05)
        else:
            os.write(fd, secret.encode("utf-8") + b"\n")
            read_until("Mở Telegram".encode("utf-8"), 10.0)

    try:
        os.kill(pid, signal.SIGINT)
    except ProcessLookupError:
        pass

    status = None
    deadline = time.monotonic() + 10.0
    while time.monotonic() < deadline:
        done, st = os.waitpid(pid, os.WNOHANG)
        if done:
            status = st
            break
        pump(0.05)
    if status is None:
        os.kill(pid, signal.SIGKILL)
        _, status = os.waitpid(pid, 0)
    # Collect what is left in the pty buffer.
    end = time.monotonic() + 0.5
    while not eof and time.monotonic() < end:
        pump(0.05)

    if os.WIFSIGNALED(status):
        code = 128 + os.WTERMSIG(status)
    else:
        code = os.WEXITSTATUS(status)

    try:
        echo = bool(termios.tcgetattr(fd)[3] & termios.ECHO)
    except termios.error:
        echo = None
    os.close(fd)

    print(
        json.dumps(
            {"output": buf.decode("utf-8", "replace"), "exit": code, "echo": echo},
            ensure_ascii=False,
        ),
        flush=True,
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
