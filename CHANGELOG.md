# Changelog

All notable changes are documented here. The project follows Semantic
Versioning before 1.0 with minor versions allowed to change unsupported
behavior while preserving documented contracts where practical.

## [Unreleased]

### Changed

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

[Unreleased]: https://github.com/yhay81/dmlpact/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/yhay81/dmlpact/releases/tag/v0.1.0
