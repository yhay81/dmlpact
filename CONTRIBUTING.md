# Contributing

Issues and pull requests are welcome, especially for reproducible safety bugs,
PostgreSQL compatibility fixtures, documentation corrections, and small
fail-closed improvements.

Before coding, open an issue for any change that expands accepted SQL or alters
transaction, plan, receipt, exit-code, or trust semantics.

## Development

Rust 1.85 is the minimum supported version. Run:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
cargo package --locked --allow-dirty
```

The SQL policy boundary is continuously fuzzed. See [FUZZING.md](FUZZING.md)
for the reproducible local command and crash-handling rules.

Live tests require PostgreSQL 13+ and `psql`:

```bash
export DMLPACT_TEST_DATABASE_URL='postgresql://postgres:postgres@127.0.0.1/postgres'
cargo build --locked
bash tests/live_postgres.sh target/debug/dmlpact
```

Use a disposable database. The script drops and recreates a table named
`dmlpact_accounts`.

Every newly accepted SQL form needs positive, rejection, drift, and receipt
tests. Never add a permissive fallback for an AST variant that is not fully
understood. Do not include secrets or production artifacts in tests.

By participating, you agree to follow [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
