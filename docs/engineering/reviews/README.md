# reviews

Audit and review records for `sdkwork-memory`, governed by `DOCUMENTATION_SPEC.md` section 2.

A review record states what was assessed, the evidence that was actually executed, and each
finding's disposition. It is a point-in-time record: it must not be rewritten to track later
work, and a superseding review is a new `REVIEW-*` document registered in `docs/INDEX.yaml`.

## Records

- `REVIEW-20260923-memory-commercial-readiness-audit.md` — commercial readiness audit plus the
  fix ledger for the same day's remediation round. The ledger marks per-finding status and, where
  later evidence overturned an original diagnosis, records the correction instead of the superseded
  claim.
- `REVIEW-20260923-mem0-capability-parity-matrix.md` — mem0 algorithm/semantics capability
  alignment matrix (batches 1–19) with per-batch evidence.
- `REVIEW-20260923-memory-mem0-implementation-audit.md` — implementation audit of the mem0
  capability alignment work.
- `REVIEW-20260928-mem0-wire-compatibility-plan.md` — the mem0 platform-wire (`mem0-platform`
  external protocol) compatibility surface: gap forensics, implementation record, and the
  six evidence rounds (official Python/JS SDK runs, framing audit, field-level contract
  reconciliation). Rolling continuation sections are appended under their own dated headings
  per the disposition recorded in the document itself.
