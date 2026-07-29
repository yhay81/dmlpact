# DMLPact

[![CI](https://github.com/yhay81/dmlpact/actions/workflows/ci.yml/badge.svg)](https://github.com/yhay81/dmlpact/actions/workflows/ci.yml)
[![license: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Plan, constrain, apply, and audit bounded PostgreSQL data changes.

DMLPact puts a machine-verifiable boundary between a proposed `INSERT`,
`UPDATE`, or `DELETE` and its execution. It creates a sealed, expiring plan,
rechecks database identity, schema, target count, and the exact target-row
fingerprint under a table lock, then either commits the exact SQL or refuses
without executing it.

Version 0.2 is deliberately narrow and conservative. It is suitable for
evaluation and controlled automation, but it is not a substitute for backups,
application invariants, least-privilege roles, or operational review.

## What it prevents

- `UPDATE` or `DELETE` without a `WHERE` clause.
- Multiple statements, joins, subqueries, explicit function expressions,
  placeholders, `RETURNING`, `UPDATE FROM`, `DELETE USING`, and `INSERT SELECT`.
- Applying SQL whose bytes or normalized AST differ from the approved plan.
- Applying after the endpoint, server address, database, role, search path,
  interpretation settings, schema, target count, or exact target-row set has
  drifted.
- Exceeding the approved row, statement-time, or lock-time budget.
- Silently using plaintext transport for a remote database.
- Mutating a table with rewrite rules, or unacknowledged user triggers or
  row-level security.
- Mutating through inheritance/partition links or foreign-key actions that can
  change another table.
- Applying without first creating a new append-only receipt file.

## Install

Release archives include the binary, completions, documentation, checksums, and
an SBOM:

```bash
gh release download v0.3.0 --repo yhay81/dmlpact
```

See [INSTALL.md](INSTALL.md) for platform-specific asset selection,
checksum- and provenance-verified installation, updating, and removal.

To build from source with the declared Rust 1.85 MSRV:

```bash
git clone https://github.com/yhay81/dmlpact.git
cd dmlpact
cargo build --release --locked
```

## Quick start

Connection material is accepted only through an environment variable. TLS with
native root certificates is required by default:

```bash
export DMLPACT_DATABASE_URL='postgresql://app@db.example.com/app'
```

For a local development server only, use a loopback address or Unix socket and
acknowledge plaintext transport on each database command:

```bash
export DMLPACT_DATABASE_URL='postgresql://postgres@127.0.0.1/app'
```

Create `change.sql`:

```sql
UPDATE public.accounts
SET active = false
WHERE id IN (101, 102);
```

Validate it without a database:

```bash
dmlpact lint --sql change.sql
```

Create a read-only, 15-minute plan:

```bash
dmlpact plan \
  --sql change.sql \
  --out change.plan.json \
  --max-rows 2 \
  --statement-timeout 20s \
  --lock-timeout 2s
```

Review `change.plan.json`, especially `preconditions`, `limits`, and
`plan_sha256`. Apply the exact SQL and plan:

```bash
dmlpact apply \
  --sql change.sql \
  --plan change.plan.json \
  --receipt change.receipt.ndjson
```

Verify the receipt offline:

```bash
dmlpact receipt verify --receipt change.receipt.ndjson
```

For the local connection example, add `--allow-insecure-localhost` to `plan`
and `apply`. The flag is rejected for any non-loopback host.

## Execution model

Planning opens a read-only `REPEATABLE READ` transaction. It resolves the target
table, captures safety-relevant catalog evidence, pre-counts target rows,
fingerprints their canonical JSON representation, and runs `EXPLAIN` without
`ANALYZE`. The plan stores hashes, not executable SQL.

Applying requires the original SQL file and a new receipt path. It opens a
read-write `REPEATABLE READ` transaction, applies local timeouts, takes a
`SHARE ROW EXCLUSIVE` table lock, and rechecks every sealed precondition.
Only then does it execute the normalized statement. An affected-row mismatch
rolls back. A commit whose result cannot be observed is recorded as
`uncertain` and exits with code 5.

The receipt is two-event NDJSON: a durable `prepared` event written before the
database operation and one terminal `committed`, `rolled_back`, `refused`, or
`uncertain` event. Each event hashes its content and links to the previous
event.

## Supported SQL

- Plain single-table `INSERT ... VALUES` with one or more equal-width rows.
- Single-table `UPDATE ... SET ... WHERE ...`.
- Single-table `DELETE FROM ... WHERE ...`.
- Ordinary permanent PostgreSQL tables on PostgreSQL 13 or newer.
- Literals, identifiers, comparisons, boolean expressions, casts, and explicit
  value lists that the PostgreSQL parser accepts.

Run `dmlpact capabilities` for the machine-readable scope. Unsupported syntax
is refused rather than partially interpreted.

For v0.1, INSERT targets cannot contain column defaults, identity columns, or
generated columns. This avoids implicit server expressions and non-transactional
sequence effects hidden from the proposal. Tables participating in inheritance
or referenced by cascading/SET NULL/SET DEFAULT foreign keys are also refused.

## Machine contracts

All successful commands except completions emit JSON. Errors emit JSON on
stderr with stable classes and codes. Generate schemas and the compact contract:

```bash
dmlpact schema plan
dmlpact schema receipt-event
dmlpact capabilities
dmlpact contract
```

Exit classes are: `0` success or fully receipted safety refusal, `1` I/O or
database transport, `2` usage, `3` policy, `4` budget, and `5` invalid contract
or uncertain commit.

The checked-in
[artifact compatibility corpus](tests/fixtures/contracts/README.md) freezes a
sealed v0.1 plan and complete hash-linked receipt. CI checks exact bytes,
offline acceptance, and declared fail-closed mutations on every supported OS.

The versioned [PostgreSQL performance harness](benchmarks/README.md) publishes
raw 10,000- and 100,000-target plan/apply/receipt baselines plus a bounded lock
contention refusal. Initial measurements remain observation-only until the
documented p95 and client/server timing-separation policy is established.

## Security and limitations

Read [the safety model](docs/safety-model.md) and [SECURITY.md](SECURITY.md)
before using DMLPact with valuable data. Trigger acknowledgement does not make
trigger side effects transactional outside PostgreSQL. Row-level security can
make visibility role-dependent. Environment variables can be visible to other
same-user processes on some systems. A valid receipt proves DMLPact's observed
workflow; it does not prove business correctness.

## Project

- [Concept and design rationale](CONCEPT.md)
- [Contracts and reason codes](docs/contracts.md)
- [Roadmap](ROADMAP.md)
- [Contributing](CONTRIBUTING.md)
- [Support](SUPPORT.md)
- [Governance](GOVERNANCE.md)
- [Changelog](CHANGELOG.md)

Licensed under the [MIT License](LICENSE).
