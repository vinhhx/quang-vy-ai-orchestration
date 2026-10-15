from __future__ import annotations

import json
import posixpath
import re
import unicodedata
from dataclasses import dataclass
from pathlib import Path

PLAN_SECTIONS = ("Mục tiêu", "Thay đổi", "Không được đụng tới", "Cách kiểm tra", "Vướng mắc")
QA_SECTIONS = ("Thay đổi có khớp plan không (từng mục)", "Sửa ngoài plan (nếu có)", "Rủi ro", "Kết luận")
QA_VERDICTS = ("Đạt", "Cần sửa")


@dataclass
class Change:
    file: str
    where: str
    what: str
    why: str


@dataclass
class Plan:
    title: str
    meta: dict | None
    sections: dict[str, str]
    changes: list[Change]


def _format_meta_line(meta: dict) -> str:
    dumped = json.dumps(meta, ensure_ascii=False, sort_keys=True)
    escaped = dumped.replace(">", "\\u003e")
    return f"<!-- brain-meta: {escaped} -->"


def _normalize_path(p: str) -> str:
    p = p.strip().strip("`").strip("'\"").strip()
    if not p:
        return ""
    p = unicodedata.normalize("NFC", p)
    p = posixpath.normpath(p)
    if p.startswith("./"):
        p = p[2:]
    return p


def render_plan_skeleton(task_title: str, meta: dict) -> str:
    template_path = Path(__file__).resolve().parent / "templates" / "plan.md"
    template_content = template_path.read_text(encoding="utf-8")
    title_first_line = task_title.splitlines()[0][:100] if task_title.splitlines() else task_title[:100]
    title_first_line = unicodedata.normalize("NFC", title_first_line)
    lines = template_content.splitlines()
    first_line = lines[0].replace("<tên task>", title_first_line)
    meta_line = _format_meta_line(meta)
    new_lines = [first_line, meta_line] + lines[1:]
    return "\n".join(new_lines) + "\n"


def render_qa_skeleton(task_title: str, meta: dict) -> str:
    template_path = Path(__file__).resolve().parent / "templates" / "qa-report.md"
    template_content = template_path.read_text(encoding="utf-8")
    title_first_line = task_title.splitlines()[0][:100] if task_title.splitlines() else task_title[:100]
    title_first_line = unicodedata.normalize("NFC", title_first_line)
    lines = template_content.splitlines()
    first_line = lines[0].replace("<tên task>", title_first_line)
    meta_line = _format_meta_line(meta)
    new_lines = [first_line, meta_line] + lines[1:]
    return "\n".join(new_lines) + "\n"


def read_meta(text: str) -> dict | None:
    match = re.search(r"<!--\s*brain-meta:\s*(.*?)\s*-->", text, flags=re.DOTALL)
    if not match:
        return None
    try:
        return json.loads(match.group(1))
    except Exception:
        return None


def set_meta(text: str, meta: dict) -> str:
    meta_line = _format_meta_line(meta)
    if re.search(r"<!--\s*brain-meta:\s*.*?-->", text, flags=re.DOTALL):
        return re.sub(r"<!--\s*brain-meta:\s*.*?-->", lambda _: meta_line, text, count=1, flags=re.DOTALL)
    lines = text.splitlines(keepends=True)
    if not lines:
        return meta_line + "\n"
    first_line = lines[0]
    if not first_line.endswith("\n"):
        first_line += "\n"
    rest = "".join(lines[1:])
    return first_line + meta_line + "\n" + rest


def section_text(text: str, name: str) -> str:
    norm_name = unicodedata.normalize("NFC", name).strip()
    lines = text.splitlines()
    in_section = False
    section_lines = []
    for line in lines:
        if line.startswith("## ") or line.startswith("# "):
            heading = unicodedata.normalize("NFC", line.lstrip("#").strip())
            if in_section:
                break
            if heading.startswith(norm_name):
                in_section = True
                continue
        elif in_section:
            section_lines.append(line)
    if not in_section:
        return ""
    return "\n".join(section_lines).strip()


def parse_plan(text: str) -> Plan:
    meta = read_meta(text)

    # Title
    title = ""
    for line in text.splitlines():
        line_clean = line.strip()
        if line_clean.lower().startswith("# plan:"):
            title = unicodedata.normalize("NFC", line_clean[7:].strip())
            break
        elif line_clean.startswith("# ") and not line_clean.lower().startswith("# qa:"):
            title = unicodedata.normalize("NFC", line_clean[2:].strip())
            break

    # Sections
    sections = {}
    lines = text.splitlines()
    for line in lines:
        if line.startswith("## "):
            heading = unicodedata.normalize("NFC", line[3:].strip())
            matched = False
            for sec in PLAN_SECTIONS:
                if heading.startswith(sec):
                    sections[sec] = section_text(text, sec)
                    matched = True
                    break
            if not matched:
                sections[heading] = section_text(text, heading)

    # Changes from Thay đổi
    changes = []
    thay_doi_text = sections.get("Thay đổi", "")
    for line in thay_doi_text.splitlines():
        s = line.strip()
        if not s.startswith("|") or s.count("|") < 2:
            continue
        if s.startswith("|"):
            s = s[1:]
        if s.endswith("|"):
            s = s[:-1]
        cells = [c.replace(r"\|", "|").strip() for c in re.split(r"(?<!\\)\|", s)]
        if len(cells) >= 2 and cells[0] == "#" and cells[1].lower() == "file":
            continue
        if all(re.match(r"^:?-+:?$", c) for c in cells):
            continue

        if len(cells) < 5:
            continue

        file_cell = cells[1].strip()
        where_cell = cells[2]
        what_cell = cells[3]
        why_cell = cells[4]

        if not file_cell:
            changes.append(Change(file="", where=where_cell, what=what_cell, why=why_cell))
        else:
            sub_files = [f.strip() for f in file_cell.split(",")]
            for sub_f in sub_files:
                norm_f = _normalize_path(sub_f)
                changes.append(Change(file=norm_f, where=where_cell, what=what_cell, why=why_cell))

    return Plan(title=title, meta=meta, sections=sections, changes=changes)


def validate_plan(plan: Plan, root: Path | None = None) -> list[str]:
    errors = []

    # 1. missing_title
    if not plan.title or not plan.title.strip():
        errors.append("missing_title")

    # 2. missing_section
    for sec in PLAN_SECTIONS:
        if sec not in plan.sections:
            errors.append(f"missing_section:{sec}")

    # Check table rows in Thay đổi
    thay_doi_text = plan.sections.get("Thay đổi", "")
    data_rows_count = 0
    bad_rows = []
    empty_cells = []

    for line in thay_doi_text.splitlines():
        s = line.strip()
        if not s.startswith("|") or s.count("|") < 2:
            continue
        if s.startswith("|"):
            s = s[1:]
        if s.endswith("|"):
            s = s[:-1]
        cells = [c.replace(r"\|", "|").strip() for c in re.split(r"(?<!\\)\|", s)]
        if len(cells) >= 2 and cells[0] == "#" and cells[1].lower() == "file":
            continue
        if all(re.match(r"^:?-+:?$", c) for c in cells):
            continue

        data_rows_count += 1
        if len(cells) < 5:
            bad_rows.append(data_rows_count)
            continue

        file_cell = cells[1].strip()
        if not file_cell:
            empty_cells.append(data_rows_count)
        else:
            sub_files = [f.strip() for f in file_cell.split(",")]
            for sub_f in sub_files:
                if not _normalize_path(sub_f):
                    empty_cells.append(data_rows_count)
                    break

    for r in bad_rows:
        errors.append(f"bad_row:{r}")
    for r in empty_cells:
        errors.append(f"empty_file_cell:{r}")

    # 3. no_changes
    if data_rows_count == 0 or (not plan.changes and not bad_rows and not empty_cells):
        errors.append("no_changes")

    # 4. File path errors for each change
    seen_outside_root = set()
    for change in plan.changes:
        f = change.file
        if not f:
            continue
        if f.startswith("/"):
            errors.append(f"absolute_path:{f}")
        elif f == "~" or f.startswith("~/"):
            errors.append(f"home_path:{f}")
        elif f == ".." or f.startswith("../") or "/../" in f or f.endswith("/.."):
            errors.append(f"parent_traversal:{f}")
        elif f == ".git" or f.startswith(".git/") or "/.git/" in f or f.endswith("/.git"):
            errors.append(f"git_dir:{f}")
        elif f in (".plan.md", ".qa-report.md") or f.endswith("/.plan.md") or f.endswith("/.qa-report.md"):
            errors.append(f"protocol_file:{f}")
        elif root is not None:
            root_resolved = Path(root).resolve()
            target_resolved = (Path(root) / f).resolve()
            try:
                target_resolved.relative_to(root_resolved)
            except ValueError:
                if f not in seen_outside_root:
                    seen_outside_root.add(f)
                    errors.append(f"outside_root:{f}")

    return errors


def plan_files(plan: Plan) -> list[str]:
    seen = set()
    result = []
    for c in plan.changes:
        if c.file and c.file not in seen:
            seen.add(c.file)
            result.append(c.file)
    return result


def has_blockers(plan: Plan) -> bool:
    body = plan.sections.get("Vướng mắc", "")
    body_no_comments = re.sub(r"<!--.*?-->", "", body, flags=re.DOTALL)
    return bool(body_no_comments.strip())


def parse_qa_verdict(text: str) -> str | None:
    kt_body = section_text(text, "Kết luận")
    if not kt_body:
        return None
    found_verdicts = set()
    for raw_line in kt_body.splitlines():
        line = raw_line.strip()
        if not line:
            continue
        line = re.sub(r"^[>\s*+-]+(?:\d+\.)?\s*", "", line)
        line = re.sub(r"^(?:kết luận|ket luan)\s*:\s*", "", line, flags=re.IGNORECASE)
        line = line.strip("`*\"'_ \t")
        norm = unicodedata.normalize("NFC", line).casefold()
        if norm == "đạt".casefold():
            found_verdicts.add("Đạt")
        elif norm == "cần sửa".casefold():
            found_verdicts.add("Cần sửa")
    if len(found_verdicts) == 1:
        return next(iter(found_verdicts))
    return None
