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
