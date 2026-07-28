# Security policy

## Supported versions

Until 1.0, only the latest released minor version receives security fixes.

## Report a vulnerability

Use GitHub's private vulnerability reporting for this repository. Do not open a
public issue for suspected vulnerabilities involving SQL boundary bypass,
credential exposure, TLS downgrade, plan/receipt integrity, row-limit escape,
or unintended execution.

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
