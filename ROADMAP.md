# Roadmap

## v0.1 — bounded single-table DML

- [x] Fail-closed PostgreSQL parser boundary.
- [x] TLS-required connections with explicit local-only exception.
- [x] Read-only sealed plans with expiry, limits, schema evidence, and EXPLAIN
  digest.
- [x] Exact target-row fingerprints for UPDATE and DELETE.
- [x] Locked revalidation and affected-row postcondition.
- [x] Durable hash-linked receipts and offline verification.
- [x] JSON Schemas, completions, cross-platform CI, live PostgreSQL tests, SBOM,
  provenance, and signed releases.

## v0.2 — operational evaluation

- [ ] Benchmark planning overhead and lock behavior across table sizes.
- [ ] Add policy files for centrally managed defaults and table allowlists.
- [ ] Add receipt reconciliation guidance and optional audit-log correlation.
- [ ] Evaluate parameter binding without weakening exact proposal identity.
- [ ] Expand PostgreSQL version and proxy compatibility fixtures.

## v1.0 criteria

- Independent security review of parser, transaction, TLS, and artifact model.
- Documented production pilots with no limit escapes or unreceipted mutations.
- Stable v1 contract with migration policy and compatibility fixtures.
- Measured resource ceilings and operator playbooks for blocked/uncertain cases.
- At least two active maintainers and a published release-support policy.

Broad SQL syntax is not a success metric. Scope expands only when its effect
boundary can be tested and explained.
