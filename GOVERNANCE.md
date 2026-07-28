# Governance

DMLPact is currently maintainer-led. The repository owner is the release
maintainer and final decision maker.

Decisions favor a small auditable safety boundary, evidence from PostgreSQL
behavior, stable machine contracts, and long-term maintenance cost. Accepted
SQL scope requires tests demonstrating both intended execution and fail-closed
behavior under drift.

Significant changes are discussed publicly in issues or pull requests unless
they concern an embargoed vulnerability. Releases require green protected
checks, an updated changelog, a signed annotated tag, generated checksums, an
SBOM, and provenance attestations.

As sustained contributors emerge, commit and release access can be added based
on demonstrated judgment, review quality, reliability, and adherence to the
security model. Governance will be revisited before v1.0 or when a second
maintainer joins.
