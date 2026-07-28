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

- [x] Publish an initial reproducible 10k/100k planning, apply, receipt, and
  bounded lock-contention baseline with raw hosted-runner measurements.
- [ ] Add policy files for centrally managed defaults and table allowlists.
- [ ] Add receipt reconciliation guidance and optional audit-log correlation.
- [ ] Evaluate parameter binding without weakening exact proposal identity.
- [ ] Expand PostgreSQL version and proxy compatibility fixtures.

Current compatibility evidence: the checked-in v0.1 corpus pins a sealed plan
and complete two-event receipt byte-for-byte. Twelve declared mutations cover
unknown fields, schema/tool versions, plan hashes, event hashes, receipt
identity, timestamps, and previous-event linkage. A second released minor
version and migration or no-migration evidence are still required.

## v1.0 quality criteria

DMLPact reaches v1.0 only when every gate below has published, reproducible
evidence. Supporting broader SQL, downloads, or stars does not substitute for
fail-closed analysis, exact effect boundaries, or real operator use.

### Product and compatibility

- CLI, proposal, plan, receipt-chain, verification, capability, schema, error,
  and exit-code contracts remain compatible across at least two released
  pre-1.0 minor versions.
- Golden proposals, plans, and receipts from every supported version are
  accepted by the current offline verifier or have a tested no-clobber
  migration command and guide.
- PostgreSQL version, parser boundary, transport, isolation, trigger, RLS,
  schema, expiry, and lock assumptions remain sealed in the plan and are
  revalidated before mutation.
- Unsupported syntax, ambiguous semantics, unavailable metadata, or degraded
  connection security is refused rather than rewritten or guessed.

### Mutation correctness and security

- A published corpus of at least 1,000 labeled INSERT, UPDATE, DELETE, CTE,
  subquery, expression, quoting, comment, multi-statement, and unsupported SQL
  cases has 100% acceptance of the declared supported subset and 100% rejection
  of every labeled unsupported or ambiguous case.
- Cross-version live PostgreSQL stress completes at least 10,000 aggregate
  plan/apply attempts with zero mutation outside the exact fingerprinted target
  set, zero affected-row limit escape, and zero committed mutation without a
  durable receipt-chain event.
- The adversarial artifact corpus has 100% rejection of proposal, database,
  schema, expiry, EXPLAIN, target-count, row-fingerprint, limit, transport,
  plan-seal, receipt-link, and receipt-payload mutations.
- TLS fixtures have 100% refusal of plaintext non-local connections, invalid
  server identity, untrusted roots, and unauthorized local-only exceptions.
- An independent security review covers SQL parsing, identifier handling,
  transaction isolation, locking, TOCTOU races, triggers, RLS, TLS, credentials,
  artifacts, receipt publication, and diagnostic disclosure; all critical and
  high findings are resolved.
- No known critical or high-severity vulnerability is open at release time.

### Performance and bounds

- Plan generation for the published 10,000-target-row fixture completes below
  5 seconds p95 on the documented PostgreSQL and runner configuration.
- Locked revalidation plus client-side apply overhead remains below 2 seconds
  p95 for the same fixture, excluding server mutation execution time.
- The published 100,000-target-row bounded fixture keeps DMLPact peak resident
  memory below 256 MiB or fails before mutation with an explicit resource-limit
  result.
- Statement bytes, target rows, runtime, lock wait, plan age, receipt size, and
  diagnostic data never exceed configured bounds without a structured refusal.
- Database image, schema, corpus, runner image, raw measurements, and regression
  thresholds are versioned with the repository.

### Delivery and maintenance

- Required CI and live PostgreSQL gates remain green for 30 consecutive days
  before the v1.0 tag, including client builds on Linux, macOS, and Windows.
- Releases originate only from protected `main` and signed annotated tags; all
  native archives have verified checksums, GitHub-hosted provenance, and a
  CycloneDX SBOM attestation.
- At least two active maintainers exercise the release and mutation-incident
  runbooks; v1.0 is blocked while the project has only one release-capable
  maintainer.
- Security reports are acknowledged within 3 business days and receive an
  initial assessment within 7.

### Adoption evidence

- At least three independent production-like pilots are recorded in
  [ADOPTERS.md](ADOPTERS.md) with no limit escape or unreceipted mutation.
- At least two adopters report repeat use separated by 30 days.
- At least one sanitized public workflow shows a mutation, refusal, rollback,
  or escalation decision improved by DMLPact evidence.
- At least one non-maintainer issue, discussion, SQL corpus case, documentation
  change, test, operator playbook, or code contribution is resolved and
  credited.

Maintainer-authored fixtures, automated downloads, stars, and synthetic
accounts cannot satisfy adoption gates.

Broad SQL syntax is not a success metric. Scope expands only when its effect
boundary can be tested and explained.
