from __future__ import annotations

import re
import unicodedata


class RoutingError(Exception):
    def __init__(self, kind: str, query: str, candidates: list[dict]):
        super().__init__(f"Routing error ({kind}): query='{query}' candidates={len(candidates)}")
        self.kind = kind
        self.query = query
        self.candidates = candidates


def normalize(text: str) -> str:
    s = text.replace("đ", "d").replace("Đ", "d")
    s = unicodedata.normalize("NFD", s)
    s = "".join(c for c in s if unicodedata.category(c) != "Mn")
    s = re.sub(r"[\s_\-]+", " ", s.lower()).strip()
    return s


def slugify(text: str, max_len: int = 48) -> str:
    norm = normalize(text)
    if not norm:
        return ""
    slug = norm.replace(" ", "-")
    slug = re.sub(r"[^a-z0-9\-]", "", slug)
    slug = re.sub(r"-+", "-", slug).strip("-")
    if len(slug) <= max_len:
        return slug
    truncated = slug[:max_len]
    last_hyphen = truncated.rfind("-")
    if last_hyphen > 0:
        truncated = truncated[:last_hyphen]
    return truncated.strip("-")


def resolve(query: str, projects: list[dict]) -> dict:
    stripped_query = query.strip() if query else ""
    if not stripped_query:
        raise RoutingError(kind="none", query=query, candidates=list(projects))

    for p in projects:
        if p.get("id", "").strip() == stripped_query:
            return p

    norm_q = normalize(stripped_query)
    matches: list[dict] = []
    for p in projects:
        matched = False
        if normalize(p.get("id", "")) == norm_q:
            matched = True
        elif normalize(p.get("name_vi", "")) == norm_q:
            matched = True
        else:
            for alias in p.get("aliases", []):
                if normalize(alias) == norm_q:
                    matched = True
                    break
        if matched:
            matches.append(p)

    if len(matches) == 1:
        return matches[0]
    if len(matches) == 0:
        raise RoutingError(kind="none", query=query, candidates=list(projects))
    raise RoutingError(kind="ambiguous", query=query, candidates=matches)
