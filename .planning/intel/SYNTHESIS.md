# Synthesis Summary

Mode: new. Precedence: ADR > SPEC > PRD > DOC. Entry point for gsd-roadmapper.

## Doc counts by type
Total 7 (ADR 0, SPEC 5, PRD 0, DOC 2, UNKNOWN 0). All paths under /mnt/data/IDE/QuangVy-Orchestration/files/
- DOC (2): 00-ROADMAP.md (medium confidence), 01-phase0-kiem-chung.md (low confidence)
- SPEC (5): 02-phase1-giao-thuc-mvp.md (medium), 03-phase2-loi-rust.md (medium), 04-phase3-toi-uu-session.md (medium), 05-phase4-dashboard.md (medium), 06-phase5-da-doi-hinh-media.md (low)

## Decisions locked
0. No ADRs in the set. See decisions.md.

## Requirements extracted
0 REQ-* entries (no PRDs). PRD-like goals, scope and acceptance tables (42 test cases: P1-01 to P1-10, P2-01 to P2-09, P3-01 to P3-06, P4-01 to P4-10, P5-01 to P5-07) are preserved verbatim in context.md under "Phase goals and scope" and "Acceptance test tables". The roadmapper should derive phase success criteria from those tables and from the phase gates.

## Constraints
24 entries in constraints.md: schema 8, api-contract 2, protocol 11, nfr 3.

## Context topics
14 topic entries in context.md (vision, principles, phase table, dependencies, estimates, KPIs, open questions, Phase 0 assumptions, Phase 0 spikes, Phase 0 acceptance, phase goals and scope, acceptance test tables, risk registers, Phase 4 UI rationale).

## Phase structure (from sources)
Phase 0 verify assumptions -> Phase 1 protocol MVP (scripts) -> Phase 2 Rust CLI core -> then Phase 3 session/token optimization, Phase 4 embedded dashboard, Phase 5 multi-engine + media branch (3, 4, 5 independent after 2; order decided after Phase 2). Details in context.md.

## Conflicts
0 blockers, 1 warning (low-confidence classifications of Phase 0 and Phase 5 docs), 5 info. Detail: /mnt/data/IDE/QuangVy-Orchestration/.planning/INGEST-CONFLICTS.md

## Intel files
- /mnt/data/IDE/QuangVy-Orchestration/.planning/intel/decisions.md
- /mnt/data/IDE/QuangVy-Orchestration/.planning/intel/requirements.md
- /mnt/data/IDE/QuangVy-Orchestration/.planning/intel/constraints.md
- /mnt/data/IDE/QuangVy-Orchestration/.planning/intel/context.md
