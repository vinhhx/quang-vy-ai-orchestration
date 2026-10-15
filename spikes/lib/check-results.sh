#!/usr/bin/env bash
# spikes/lib/check-results.sh - validator for spikes/spike-results.md (SPK-09, D-01, D-02, D-13).
#
#   bash spikes/lib/check-results.sh               structure: headings S1..S8 + SPK-10, every field label present
#   bash spikes/lib/check-results.sh --section S4  one section is complete: all fields non-empty, Kết quả has
#                                                  exactly one of Đạt / Không đạt / Một phần plus the pending
#                                                  label, Một phần / Không đạt need Phương án thay thế and
#                                                  Phase bị ảnh hưởng, every Bằng chứng path exists
#   bash spikes/lib/check-results.sh --final       all sections complete, no pending label anywhere,
#                                                  S1 and S4 have "Cách làm đã kiểm chứng (D-13)", SPK-10 decided
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO="$(cd "$SCRIPT_DIR/../.." && pwd -P)"
FILE="${SPIKE_RESULTS_FILE:-$REPO/spikes/spike-results.md}"
MODE="structure"
ARG=""
case "${1:-}" in
  "") ;;
  --final) MODE="final" ;;
  --section) MODE="section"; ARG="${2:-}"; [ -n "$ARG" ] || { echo "check-results: --section needs S1..S8 or SPK-10" >&2; exit 2; } ;;
  *) echo "check-results: unknown argument: $1" >&2; exit 2 ;;
esac
[ -f "$FILE" ] || { echo "check-results: missing $FILE" >&2; exit 1; }

exec python3 -I - "$MODE" "$ARG" "$FILE" "$REPO" <<'PY'
import glob
import os
import re
import sys

mode, arg, path, repo = sys.argv[1:5]
PENDING = "(chờ duyệt)"
SPIKES = ["S%d" % i for i in range(1, 9)]
FIELDS = ["Ngày thử", "Phiên bản", "Lệnh", "Kết quả", "Quan sát", "Ảnh hưởng tới thiết kế",
          "Phương án thay thế", "Phase bị ảnh hưởng", "Bằng chứng"]
EXTRA = {"S1": ["Cách làm đã kiểm chứng (D-13)"], "S4": ["Cách làm đã kiểm chứng (D-13)"]}
SPK10_FIELDS = ["Câu hỏi chính sách", "Quyết định", "Người xác nhận / ngày"]
VERDICTS = ["Không đạt", "Một phần", "Đạt"]
SPK10_PENDING = "chờ người dùng xác nhận"

text = open(path, encoding="utf-8").read()
errors = []


def err(msg):
    errors.append(msg)


# split into "## " sections
sections = {}
order = []
cur = None
for line in text.splitlines():
    m = re.match(r"^## (S[1-8]|SPK-10)\b", line)
    if m:
        cur = m.group(1)
        sections[cur] = []
        order.append(cur)
        continue
    if line.startswith("## "):
        cur = None
        continue
    if cur is not None:
        sections[cur].append(line)


def parse_fields(lines):
    """Top-level '- Label: value' items; indented lines belong to the preceding field."""
    fields = {}
    label = None
    for ln in lines:
        m = re.match(r"^- ([^:]+?):\s*(.*)$", ln)
        if m:
            label = m.group(1).strip()
            fields[label] = [m.group(2)]
        elif label is not None:
            fields[label].append(ln)
    return {k: "\n".join(v).strip() for k, v in fields.items()}


def required_fields(name):
    if name == "SPK-10":
        return SPK10_FIELDS
    return FIELDS + EXTRA.get(name, [])


def check_structure():
    for name in SPIKES + ["SPK-10"]:
        if name not in sections:
            err("missing section heading: " + name)
            continue
        fields = parse_fields(sections[name])
        for f in required_fields(name):
            if f not in fields:
                err("%s: missing field label: %s" % (name, f))
    if "SPK-10" in sections:
        if not any(l.startswith("|") and "organizationType" in l for l in sections["SPK-10"]):
            err("SPK-10: missing profile table (Profile | Thư mục | organizationType | seatTier | organizationRole)")


def verdict_of(value):
    found = [v for v in VERDICTS if v in value]
    # "Không đạt" contains lowercase "đạt", never the capitalised "Đạt", so no overlap
    return found


def check_section(name, need_label):
    if name not in sections:
        err("missing section: " + name)
        return
    fields = parse_fields(sections[name])
    for f in required_fields(name):
        if f not in fields:
            err("%s: missing field label: %s" % (name, f))
        elif not fields[f].strip():
            err("%s: empty field: %s" % (name, f))
    if name == "SPK-10":
        dec = fields.get("Quyết định", "")
        if need_label is False and SPK10_PENDING in dec:
            err("SPK-10: Quyết định is still pending")
        return
    kq = fields.get("Kết quả", "")
    found = verdict_of(kq)
    if len(found) != 1:
        err("%s: Kết quả must hold exactly one of Đạt / Không đạt / Một phần (found %d)" % (name, len(found)))
    if need_label and PENDING not in kq:
        err("%s: Kết quả is a proposal and must carry %s" % (name, PENDING))
    if found and found[0] in ("Không đạt", "Một phần"):
        for f in ("Phương án thay thế", "Phase bị ảnh hưởng"):
            v = fields.get(f, "").strip()
            if not v or v.lower().startswith("không cần"):
                err("%s: %s needs a real alternative/phase when Kết quả is %s" % (name, f, found[0]))
    for p in re.findall(r"`([^`]+)`", fields.get("Bằng chứng", "")):
        full = p if os.path.isabs(p) else os.path.join(repo, p)
        if not glob.glob(full):
            err("%s: Bằng chứng path does not exist: %s" % (name, p))


if mode == "structure":
    check_structure()
elif mode == "section":
    if arg not in SPIKES + ["SPK-10"]:
        err("unknown section: " + arg)
    else:
        check_section(arg, need_label=True)
else:  # final
    check_structure()
    for name in SPIKES + ["SPK-10"]:
        check_section(name, need_label=False)
    n = text.count(PENDING)
    if n:
        err("%d pending label(s) %s remain" % (n, PENDING))
    for name in ("S1", "S4"):
        f = parse_fields(sections.get(name, []))
        if not f.get("Cách làm đã kiểm chứng (D-13)", "").strip():
            err("%s: Cách làm đã kiểm chứng (D-13) is empty" % name)

if errors:
    for e in errors:
        print("FAIL: " + e)
    print("check-results (%s): %d problem(s)" % (mode if mode != "section" else "section " + arg, len(errors)))
    sys.exit(1)
print("check-results (%s): OK" % (mode if mode != "section" else "section " + arg))
PY
