#!/usr/bin/env bash
# E-09 observation printer (D-02, D-34). Observation only: prints name=value lines, never a verdict, never a key.
#
#   bash spikes/e09/verify.sh DIR
#
# DIR holds e09-{a,a2,b,c}.jsonl (+ .stderr.txt, .exit, .file) written by spikes/e09/run.sh: one model directory
# (spikes/e09/raw/{name}/m{k}/). Probe b exists only in m1.
# Prints `<probe>.<name>=<value>`; probes a2 and c appear only when their files exist. Always exits 0.
# The key (the variable named by E09_KEY_ENV, default DEEPSEEK_API_KEY, if set) is only used to test presence with
# grep -qF; it is never printed.
set -u

DIR="${1:-}"
if [ -z "$DIR" ] || [ ! -d "$DIR" ]; then
  echo "usage: verify.sh DIR" >&2
  exit 0
fi

KEY_VAR="${E09_KEY_ENV:-DEEPSEEK_API_KEY}"
KEY=""
# indirect expansion of an arbitrary string is unsafe: accept only a plain variable name
if [[ "$KEY_VAR" =~ ^[A-Z_][A-Z0-9_]*$ ]]; then
  KEY="${!KEY_VAR:-}"
fi

for p in a a2 b c; do
  f="$DIR/e09-$p.jsonl"
  [ -f "$f" ] || continue

  python3 -I - "$f" "$p" <<'PY'
import json
import sys

path, prefix = sys.argv[1], sys.argv[2]


def out(name, value):
    print("%s.%s=%s" % (prefix, name, value))


def yn(flag):
    return "yes" if flag else "no"


def num(value):
    try:
        return float(value)
    except (TypeError, ValueError):
        return 0.0


with open(path, encoding="utf-8", errors="replace") as fh:
    lines = [ln for ln in fh.read().splitlines() if ln.strip()]

objs = []
parses = bool(lines)
for ln in lines:
    try:
        obj = json.loads(ln)
    except ValueError:
        parses = False
        continue
    if isinstance(obj, dict):
        objs.append(obj)
    else:
        parses = False

init = None
result = None
tool_use_write = False
tool_result = False
for obj in objs:
    kind = obj.get("type")
    if kind == "system" and obj.get("subtype") == "init" and init is None:
        init = obj
    elif kind == "result":
        result = obj
    message = obj.get("message")
    content = message.get("content") if isinstance(message, dict) else None
    if isinstance(content, list):
        for block in content:
            if not isinstance(block, dict):
                continue
            if kind == "assistant" and block.get("type") == "tool_use" and block.get("name") in ("Write", "Edit"):
                tool_use_write = True
            if kind == "user" and block.get("type") == "tool_result":
                tool_result = True


def flat(value, limit=None):
    text = " ".join(str(value).split())
    return text[:limit] if limit else text


out("stream_json_parses", yn(parses))
out("init_seen", yn(init is not None))
out("api_key_source", flat((init or {}).get("apiKeySource") or "none"))
out("init_model", flat((init or {}).get("model") or "none"))
out("tool_use_write", yn(tool_use_write))
out("tool_result", yn(tool_result))
out("result_seen", yn(result is not None))
if result is None:
    out("result_is_error", "none")
    out("api_error_status", "none")
    out("terminal_reason", "none")
    out("usage_input_gt0", "no")
    out("usage_output_gt0", "no")
    out("model_usage_keys", "none")
    out("total_cost_usd", "none")
    out("result_text_head", "none")
else:
    usage = result.get("usage")
    usage = usage if isinstance(usage, dict) else {}
    model_usage = result.get("modelUsage")
    keys = ",".join(sorted(str(k) for k in model_usage)) if isinstance(model_usage, dict) and model_usage else "none"
    cost = result.get("total_cost_usd")
    text = result.get("result")
    out("result_is_error", yn(result.get("is_error") is True))
    out("api_error_status", flat(result.get("api_error_status")) if result.get("api_error_status") is not None else "none")
    out("terminal_reason", flat(result.get("terminal_reason")) if result.get("terminal_reason") else "none")
    out("usage_input_gt0", yn(num(usage.get("input_tokens")) > 0))
    out("usage_output_gt0", yn(num(usage.get("output_tokens")) > 0))
    out("model_usage_keys", flat(keys))
    out("total_cost_usd", flat(cost) if cost is not None else "none")
    out("result_text_head", flat(text, 120) if isinstance(text, str) and text else "none")
PY

  # exit code written by run.sh (e09_call)
  ec="none"
  if [ -f "$DIR/e09-$p.exit" ]; then
    ec="$(sed -n 's/^exit_code=//p' "$DIR/e09-$p.exit" | head -n1)"
    [ -n "$ec" ] || ec="none"
  fi
  echo "$p.exit_code=$ec"

  # file check (probes that were asked to write e09.txt)
  if [ -f "$DIR/e09-$p.file" ]; then
    if grep -qx 'file=yes' "$DIR/e09-$p.file" && grep -qx 'content_ok=yes' "$DIR/e09-$p.file"; then
      echo "$p.file_ok=yes"
    else
      echo "$p.file_ok=no"
    fi
  fi

  # key presence: yes/no only, never the value
  if [ -z "$KEY" ]; then
    echo "$p.key_absent=unknown"
  else
    hit=0
    for g in "$f" "$DIR/e09-$p.stderr.txt"; do
      [ -f "$g" ] || continue
      if grep -qF -- "$KEY" "$g"; then hit=1; fi
    done
    if [ "$hit" -eq 0 ]; then echo "$p.key_absent=yes"; else echo "$p.key_absent=no"; fi
  fi
done

exit 0
