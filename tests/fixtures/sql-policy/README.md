# SQL policy corpus

`v0.1/corpus.json` is a deterministic, MIT-licensed set of 1,000 labeled
PostgreSQL statements. Ten manually curated families contain 100 cases each:

- accepted `INSERT ... VALUES`, bounded `UPDATE`, and bounded `DELETE`;
- accepted quoted identifiers, escaped literals, and comments;
- rejected CTEs, subqueries, dynamic expressions, and multiple statements;
- rejected unsupported mutation forms and ambiguous or invalid SQL.

The expected decision, statement kind, requested table, inserted-row count, or
stable refusal code is labeled from DMLPact's documented safety contract. The
generator does not invoke DMLPact or derive labels from parser output.

`tests/sql_policy_corpus.rs` verifies the corpus digest and independently
scores every case through the same in-memory analyzer used before planning or
applying a mutation. It requires exact agreement with the pinned metrics:

- 400/400 declared-supported statements accepted;
- 600/600 unsupported or ambiguous statements rejected;
- 1,000/1,000 exact decisions and classifications.

Regenerate the corpus and target metrics from the repository root:

```bash
python3 tests/fixtures/sql-policy/v0.1/generate_corpus.py
python3 tests/fixtures/sql-policy/v0.1/generate_corpus.py --check
cargo test --locked --test sql_policy_corpus
```

This corpus measures the static SQL policy boundary. Live PostgreSQL stress,
schema evidence, TLS, locks, triggers, row-level security, plan revalidation,
and receipt durability remain separate gates.
