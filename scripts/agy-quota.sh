#!/usr/bin/env bash
# Print agy quota remaining per model group (from `agy -p /usage`, no model turn used).
# Usage: scripts/agy-quota.sh [group-substring]
#   With a group substring (e.g. "Gemini Models"), prints only the lowest remaining
#   fraction (weekly or 5h) of that group as a number 0..1, for scripting.
set -euo pipefail
out=$(mktemp)
trap 'rm -f "$out"' EXIT
timeout 60 agy -p /usage --output-format stream-json --print-timeout 50s </dev/null >"$out" 2>/dev/null
python3 - "$out" "${1:-}" <<'EOF'
import json, sys
path, want = sys.argv[1], sys.argv[2]
for line in open(path):
    try:
        d = json.loads(line)
    except ValueError:
        continue
    if d.get("event") != "command_result":
        continue
    for g in d["command"]["data"]["groups"]:
        fr = {b["window"]: b["remaining_fraction"] for b in g.get("buckets", [])}
        if want:
            if want.lower() in g["name"].lower():
                print(round(min(fr.values()), 3))
                sys.exit(0)
        else:
            print(g["name"], {k: round(v, 3) for k, v in fr.items()})
if want:
    sys.exit("group not found: " + want)
EOF
