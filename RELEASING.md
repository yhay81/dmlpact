# Releasing

Releases are built only from signed annotated `vX.Y.Z` tags.

1. Confirm the version in `Cargo.toml` and heading in `CHANGELOG.md` match.
2. Run all local checks from [CONTRIBUTING.md](CONTRIBUTING.md), plus the live
   PostgreSQL fixture.
3. Merge through the protected `main` branch and confirm every required check.
4. Create and push a signed annotated tag:

   ```bash
   git tag -s v0.1.0 -m "DMLPact v0.1.0"
   git push origin v0.1.0
   ```

5. The release workflow validates the tag signature, builds native archives,
   creates a CycloneDX SBOM and SHA-256 checksums, attaches attestations, and
   creates the GitHub release.
6. Independently download the release, verify checksums and attestations,
   inspect archive contents, and run `--version`, `capabilities`, `lint`, and a
   disposable PostgreSQL plan/apply/receipt lifecycle.

Never reuse or move a release tag. Publish a new patch release for corrections.
