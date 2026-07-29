# Releasing

Releases are built only from signed annotated `vX.Y.Z` tags.

1. Confirm the version in `Cargo.toml` and heading in `CHANGELOG.md` match.
2. Run all local checks from [CONTRIBUTING.md](CONTRIBUTING.md), plus the live
   PostgreSQL fixture.
3. Merge through the protected `main` branch and confirm every required check.
4. Create and push a signed annotated tag:

   ```bash
   git tag -s v0.3.0 -m "DMLPact v0.3.0"
   git push origin v0.3.0
   ```

5. The release workflow validates the tag signature, builds native archives,
   creates a CycloneDX SBOM and SHA-256 checksums, attaches attestations, and
   creates the GitHub release. Each archive includes a downloadable
   `.intoto.jsonl` provenance bundle for local verification.
6. Independently download the release and verify checksums and attestations:

   ```bash
   sha256sum --check SHA256SUMS
   gh attestation verify dmlpact-v0.3.0-linux-x86_64.tar.gz \
     --repo yhay81/dmlpact
   gh attestation verify dmlpact-v0.3.0-linux-x86_64.tar.gz \
     --repo yhay81/dmlpact \
     --bundle dmlpact-v0.3.0-linux-x86_64.tar.gz.intoto.jsonl \
     --signer-workflow yhay81/dmlpact/.github/workflows/release.yml
   gh attestation verify dmlpact-v0.3.0-linux-x86_64.tar.gz \
     --repo yhay81/dmlpact \
     --predicate-type https://cyclonedx.org/bom
   ```

7. Inspect archive contents and run `--version`, `capabilities`, `lint`, and a
   disposable PostgreSQL plan/apply/receipt lifecycle.

## crates.io

The first crates.io release must be published manually because Trusted
Publishing can only be configured after the crate exists. From the exact signed
release commit, run `cargo publish --dry-run --locked`, review
`cargo package --list --locked`, then publish:

```bash
cargo publish --locked
```

Use a Cargo credential provider backed by the operating-system credential
store. Never put a crates.io token in Git, workflow YAML, logs, or a
repository-level Actions secret. If Cargo times out after upload, check the
crates.io page and index before retrying; an accepted version is immutable.

After the first manual release:

1. Add the crate's Trusted Publisher in crates.io, restricted to
   `yhay81/dmlpact`, the dedicated publish workflow filename, and the protected
   `crates-io` GitHub environment.
2. Add that workflow only after the mapping exists. Grant only
   `contents: read` and `id-token: write`, pin every action to an immutable
   commit, exchange OIDC with `rust-lang/crates-io-auth-action`, and run
   `cargo publish --locked`.
3. Remove any temporary API token, verify registry ownership and account
   recovery without recording secrets, and require environment approval for
   every publish.
4. Install the exact version from crates.io in a clean environment and repeat
   the PostgreSQL CLI smoke checks.

Never reuse or move a release tag. Publish a new patch release for corrections.
