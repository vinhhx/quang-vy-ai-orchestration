# Requirements (from PRD-classified docs)

No PRD-classified documents in the ingest set. No REQ-* entries extracted; none were inferred.

Note for downstream consumers: every SPEC doc (Phase 1 to Phase 5) contains a goals section ("Mục tiêu"), scope section ("Phạm vi") and an acceptance-test table (P1-xx to P5-xx). These are PRD-like, but the classifier chose SPEC and this synthesizer does not reclassify. The goals and acceptance tables are preserved verbatim in `context.md` under "Phase goals and scope" and "Acceptance test tables". If the user wants them treated as requirements, re-tag the docs as PRD via `--manifest` and re-run ingest.
