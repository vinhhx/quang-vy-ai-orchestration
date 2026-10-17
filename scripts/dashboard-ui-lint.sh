#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -P "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -P "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

python3 - << 'EOF'
import sys
import re
from pathlib import Path

errors = []
total_interactive = 0

index_path = Path("agent-brain/assets/dashboard/index.html")
if not index_path.exists():
    print("Thiếu trong index.html: agent-brain/assets/dashboard/index.html")
    sys.exit(1)

index_text = index_path.read_text(encoding="utf-8")
for sub in [
    '<html lang="vi" class="scheme-light dark:scheme-dark">',
    '<meta name="color-scheme" content="light dark">',
    '<main id="screen"',
]:
    if sub not in index_text:
        errors.append(f"Thiếu trong index.html: {sub}")

tag_re = re.compile(r'<(a|button|input|select|textarea)\b[^>]*>', re.DOTALL)
class_re = re.compile(r'(?<![:\w-])class="([^"]*)"')

screens_dir = Path("agent-brain/assets/dashboard/screens")
screen_files = sorted(screens_dir.glob("*.html")) if screens_dir.exists() else []
html_files = [index_path] + screen_files

for p in html_files:
    content = p.read_text(encoding="utf-8")
    for m in tag_re.finditer(content):
        total_interactive += 1
        tag_str = m.group(0)
        line_no = content[:m.start()].count("\n") + 1
        cm = class_re.search(tag_str)
        has_focus = False
        if cm and "focus-visible:outline-2" in cm.group(1):
            has_focus = True
        if not has_focus:
            collapsed = " ".join(tag_str.split())[:100]
            errors.append(f"Thiếu FOCUS: {p}:{line_no}: {collapsed}")

if errors:
    for e in errors:
        print(e)
    sys.exit(1)

print(f"UI lint OK: {total_interactive} thẻ tương tác")
EOF
