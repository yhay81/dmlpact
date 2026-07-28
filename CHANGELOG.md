# Changelog

All notable changes are documented here. The project follows Semantic
Versioning before 1.0 with minor versions allowed to change unsupported
behavior while preserving documented contracts where practical.

## [Unreleased]

### Added

- Added a privacy-conscious adoption report form that captures evaluation,
  repeat-use, limitations, evidence, and public-listing permission.
- Added a monthly maintainer-continuity drill that recovers the public Git
  mirror and verifies signed tags, release checksums, build/SBOM attestations,
  and the released native binary without repository write access.
- Added pull-request dependency review and weekly OpenSSF Scorecard analysis,
  with every action pinned to an immutable commit SHA.

## [0.2.0] - 2026-07-29

### Compatibility

- Preserved the public v0.1 CLI and machine contracts. The v0.2 reader accepts
  the digest-pinned v0.1 plan and receipt corpus byte-for-byte; no migration is
  required.

### Added

- Added a digest-pinned v0.1 plan and two-event receipt corpus with exact
  round trips and twelve declared artifact mutations.
- Added a reproducible PostgreSQL benchmark for 10k/100k target plans,
  exact bounded mutations, verified receipts, and lock-timeout refusal with
  raw runner and database identity retained for 90 days.

### Changed

- Decoupled artifact acceptance from the current binary version so v0.2 can
  verify the published v0.1 plan and receipt contracts.
- Defined measurable v1.0 compatibility, SQL classification, mutation
  correctness, TLS and artifact security, performance, delivery, maintenance,
  contribution, and repeat-adoption gates.

## [0.1.0] - 2026-07-28

### Added

- Fail-closed analysis for single-table INSERT VALUES, UPDATE, and DELETE.
- Sealed expiring plans with database, schema, count, exact target-set, timeout,
  transport, trigger, RLS, and EXPLAIN evidence.
- Locked `REPEATABLE READ` application with exact affected-row postconditions.
- Durable two-event hash-linked NDJSON receipts and offline verification.
- TLS-required PostgreSQL connections with an explicit local-only exception.
- Stable JSON outputs, errors, JSON Schemas, capabilities, compact contract,
  exit classes, and five shell completion formats.
- Unit, CLI, cross-platform, MSRV, dependency-audit, and live PostgreSQL tests.
- Signed release workflow with checksums, CycloneDX SBOM, and build
  attestations.

[Unreleased]: https://github.com/yhay81/dmlpact/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/yhay81/dmlpact/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/yhay81/dmlpact/releases/tag/v0.1.0
