# DMLPact

Plan, constrain, apply, and audit PostgreSQL data changes.

> Status: concept stage. Do not use this repository against production databases.

DMLPact is an agent-native control plane for PostgreSQL DML. Reads are bounded by default. Writes require an immutable plan, explicit limits, state preconditions, and a resulting receipt.

## Why

Traditional SQL clients make it easy to execute a syntactically valid statement without understanding its blast radius. Agents need a contract that exposes affected rows, locks, timeouts, rollback feasibility, and state drift before mutation.

```bash
dmlpact inspect --tables users,orders --fields columns,keys
dmlpact plan --sql-file update.sql --max-rows 100
dmlpact apply plan_01J...
dmlpact receipt show rcpt_01J...
```

## Product principles

- PostgreSQL first, not lowest-common-denominator SQL.
- Read-only by default.
- No raw write executes without a plan.
- Hard row, byte, and time budgets.
- Transactional application with state preconditions.
- Secrets never appear in argv or output.
- Every mutation produces an auditable receipt.

## Initial scope

The first release focuses on `SELECT`, `INSERT`, `UPDATE`, and `DELETE` in a single PostgreSQL database. Schema migrations, cross-database transactions, and autonomous query generation are explicitly out of scope.

See [CONCEPT.md](CONCEPT.md) for the proposed safety model and MVP.

## License

MIT
