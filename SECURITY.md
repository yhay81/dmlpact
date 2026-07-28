# Security policy

## Supported versions

Until 1.0, only the latest released minor version receives security fixes.

## Report a vulnerability

Use
[GitHub private vulnerability reporting](https://github.com/yhay81/dmlpact/security/advisories/new).
Do not open a public issue for suspected vulnerabilities involving SQL boundary
bypass, credential exposure, TLS downgrade, plan/receipt integrity, row-limit
escape, or unintended execution.

Include the affected version, platform, PostgreSQL version, minimal
reproduction, expected/observed behavior, and impact. Remove real credentials,
connection strings, customer data, and production receipts.

The maintainer aims to acknowledge a report within seven days. Response and
release timing depend on severity and maintainer availability; no service-level
agreement is provided.

## Scope

Security properties and known limitations are documented in
[docs/safety-model.md](docs/safety-model.md). A policy refusal or conservative
false positive is normally not a vulnerability. Executing beyond a sealed
limit, accepting unsupported SQL, leaking connection material, or reporting a
false committed/rolled-back outcome is security-sensitive.

## Release and dependency policy

Dependabot monitors Rust and GitHub Actions dependencies. CI checks
`Cargo.lock` against RustSec advisories. Tagged releases use signed annotated
tags and include checksums, CycloneDX SBOMs, and GitHub/Sigstore attestations.
See [RELEASING.md](RELEASING.md).

Pull requests are checked with GitHub Dependency Review and fail when they
introduce a dependency with a known moderate-or-higher-severity vulnerability.
A weekly OpenSSF Scorecard analysis publishes authenticated results and uploads
SARIF findings to GitHub code scanning. CodeQL default setup analyzes Rust and
workflow sources with extended security queries.
