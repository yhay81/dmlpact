## Summary

## Safety and contract impact

- [ ] Accepted SQL boundary is unchanged, or new positive and rejection fixtures are included.
- [ ] Plan, receipt, JSON Schema, reason-code, and exit-code effects are documented.
- [ ] No credentials, private SQL/data, or production artifacts are included.

## Verification

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --locked -- -D warnings`
- [ ] `cargo test --all-targets --locked`
- [ ] Live PostgreSQL test, if database behavior changed
