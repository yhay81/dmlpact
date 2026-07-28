# Fuzzing DMLPact

DMLPact continuously fuzzes its untrusted SQL boundary with AddressSanitizer.
The `sql_document` target exercises the production PostgreSQL parser and every
single-statement, AST-shape, expression, table, and row-bound policy check
without requiring a database.

Install a current nightly toolchain and the pinned local runner, then run:

```bash
cargo install cargo-fuzz --version 0.13.2 --locked
mkdir -p fuzz/corpus/sql_document
cp fuzz/seeds/*.sql fuzz/corpus/sql_document/
cargo +nightly fuzz run sql_document
```

Pull requests receive a five-minute ClusterFuzzLite code-change run. A
15-minute batch run executes weekly on `main`, seeded by accepted INSERT,
UPDATE, and DELETE forms, and publishes machine-readable findings to GitHub
code scanning.
Each code-changing `main` update also saves a comparison build so later pull
requests can distinguish newly introduced crashes. The accumulated corpus is
pruned after every weekly batch.

Minimized SQL may contain private schema names or values. Keep crashes private
until reviewed, add a deterministic acceptance or rejection regression test,
and use [SECURITY.md](SECURITY.md) for security-relevant findings.
