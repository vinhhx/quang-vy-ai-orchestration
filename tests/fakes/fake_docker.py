#!/usr/bin/env python3
"""Fake of the `docker` client, only the three subcommands agent-brain's dsh engine uses. Stdlib only.

  ps       the readiness probe of plan 06-26:
           `docker ps --filter name=^C$ --format {{.Status}}`. Whatever the filter names, one
           container is emulated; its status line is FAKE_DOCKER_STATUS (default `Up 1 minute`).
           FAKE_DOCKER_STATUS set to the empty string prints nothing (no such container). Exit 2
           when --format is missing: the fake emulates only the probe shape.
  inspect  `docker inspect --format {{json .Mounts}} C` (--format before or after the container).
           Prints FAKE_DOCKER_MOUNTS (JSON; default one same-path read-write bind of `/`). Exit 2
           without --format: a bare inspect would print the container environment.
  exec     `docker exec [-w DIR] C CMD...`
             CMD `ps`    runs the local ps and relays its output and exit code, minus the rows of
                         this fake itself (they are the host side, a container cannot see them)
             CMD `kill`  parses -TERM / -KILL / -INT and PIDs and signals each (a missing PID is
                         ignored)
             any other   runs CMD detached from the client: the client forks, the child calls
                         setsid() and forks again, the grandchild runs CMD in DIR as its own child
                         and reports the exit code through a pipe. The client relays the output
                         and exits with that code. It installs no SIGTERM handler, so a host-side
                         kill ends only the client and leaves CMD running, exactly as S6 (4a)
                         observed on the real container.

Environment: FAKE_DOCKER_RECORD (path; one JSON line per call: {argv, cwd, env_names}),
FAKE_DOCKER_STATUS, FAKE_DOCKER_MOUNTS.
"""
from __future__ import annotations

import json
import os
import signal
import subprocess
import sys

DEFAULT_STATUS = "Up 1 minute"
DEFAULT_MOUNTS = [
    {"Type": "bind", "Source": "/", "Destination": "/", "Mode": "", "RW": True, "Propagation": "rprivate"}
]
SELF = os.path.abspath(__file__)

SIGNALS = {
    "TERM": signal.SIGTERM,
    "KILL": signal.SIGKILL,
    "INT": signal.SIGINT,
    "HUP": signal.SIGHUP,
}


def record(argv: list[str]) -> None:
    path = os.environ.get("FAKE_DOCKER_RECORD")
    if not path:
        return
    entry = {"argv": argv, "cwd": os.getcwd(), "env_names": sorted(os.environ.keys())}
    with open(path, "a", encoding="utf-8") as f:
        f.write(json.dumps(entry) + "\n")


def fail(msg: str, code: int = 2) -> None:
    sys.stderr.write(msg + "\n")
    sys.exit(code)


def option_value(args: list[str], name: str) -> str | None:
    for i, a in enumerate(args):
        if a == name and i + 1 < len(args):
            return args[i + 1]
        if a.startswith(name + "="):
            return a.split("=", 1)[1]
    return None


def cmd_ps(args: list[str]) -> None:
    fmt = option_value(args, "--format")
    if fmt is None:
        fail("fake docker ps: --format is required (the fake emulates only the readiness probe)")
    status = os.environ.get("FAKE_DOCKER_STATUS", DEFAULT_STATUS)
    if status != "":
        sys.stdout.write(status + "\n")
    sys.stdout.flush()
    sys.exit(0)


def cmd_inspect(args: list[str]) -> None:
    fmt = option_value(args, "--format")
    if fmt is None:
        fail("fake docker inspect: --format is required")
    if ".Mounts" in fmt:
        raw = os.environ.get("FAKE_DOCKER_MOUNTS")
        sys.stdout.write((raw if raw is not None else json.dumps(DEFAULT_MOUNTS)) + "\n")
    sys.stdout.flush()
    sys.exit(0)


def parse_exec(args: list[str]) -> tuple[str | None, str, list[str]]:
    workdir = None
    i = 0
    while i < len(args):
        a = args[i]
        if a in ("-w", "--workdir"):
            workdir = args[i + 1] if i + 1 < len(args) else None
            i += 2
        elif a in ("-u", "--user", "-e", "--env"):
            i += 2
        elif a.startswith("-"):
            i += 1
        else:
            break
    if i >= len(args):
        fail("fake docker exec: container missing")
    return workdir, args[i], args[i + 1 :]


def exec_ps(cmd: list[str]) -> None:
    res = subprocess.run(cmd, stdin=subprocess.DEVNULL, capture_output=True, text=True, errors="replace")
    out = []
    for line in res.stdout.splitlines():
        if SELF in line:
            continue  # host-side docker clients are not visible inside the container
        out.append(line)
    if out:
        sys.stdout.write("\n".join(out) + "\n")
    sys.stdout.flush()
    sys.stderr.write(res.stderr)
    sys.exit(res.returncode)


def exec_kill(cmd: list[str]) -> None:
    sig = signal.SIGTERM
    pids: list[int] = []
    for a in cmd[1:]:
        name = a[1:].removeprefix("SIG") if a.startswith("-") else ""
        if name in SIGNALS:
            sig = SIGNALS[name]
        elif a.isdigit():
            pids.append(int(a))
    for pid in pids:
        try:
            os.kill(pid, sig)
        except (ProcessLookupError, PermissionError):
            pass
    sys.exit(0)


def exec_detached(workdir: str | None, cmd: list[str]) -> None:
    out_r, out_w = os.pipe()
    st_r, st_w = os.pipe()
    first = os.fork()
    if first == 0:
        os.setsid()
        if os.fork() != 0:
            os._exit(0)
        # grandchild: own session, re-parented away from the client
        os.close(out_r)
        os.close(st_r)
        try:
            devnull = os.open(os.devnull, os.O_RDONLY)
            proc = subprocess.Popen(cmd, cwd=workdir, stdin=devnull, stdout=out_w)
            os.close(out_w)
            rc = proc.wait()
        except OSError as e:
            sys.stderr.write(f"fake docker exec: {e}\n")
            rc = 127
        code = rc if rc >= 0 else 128 - rc
        try:
            os.write(st_w, str(code).encode())
        except OSError:
            pass
        os._exit(0)
    os.waitpid(first, 0)
    os.close(out_w)
    os.close(st_w)
    while True:
        chunk = os.read(out_r, 65536)
        if not chunk:
            break
        sys.stdout.buffer.write(chunk)
        sys.stdout.buffer.flush()
    raw = b""
    while True:
        chunk = os.read(st_r, 64)
        if not chunk:
            break
        raw += chunk
    try:
        code = int(raw.decode().strip())
    except ValueError:
        code = 1
    sys.exit(code)


def cmd_exec(args: list[str]) -> None:
    workdir, _container, cmd = parse_exec(args)
    if not cmd:
        fail("fake docker exec: command missing")
    if cmd[0] == "ps":
        exec_ps(cmd)
    elif cmd[0] == "kill":
        exec_kill(cmd)
    else:
        exec_detached(workdir, cmd)


def main() -> None:
    argv = sys.argv[1:]
    record(argv)
    if not argv:
        fail("usage: fake_docker.py ps|inspect|exec ...")
    sub, rest = argv[0], argv[1:]
    if sub == "ps":
        cmd_ps(rest)
    elif sub == "inspect":
        cmd_inspect(rest)
    elif sub == "exec":
        cmd_exec(rest)
    else:
        fail(f"fake docker: unsupported subcommand {sub}")


if __name__ == "__main__":
    main()
