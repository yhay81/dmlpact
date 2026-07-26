# DMLPact concept

## One-line thesis

DMLPact lets humans and software agents plan, constrain, apply, and audit
PostgreSQL data changes through an explicit transaction contract.

## Problem

Database CLIs make SQL easy to execute, but they do not make autonomous data
changes safe. An agent needs to know, before mutation:

- which database and schema it will affect;
- how many rows may change;
- whether the schema or target set drifted after inspection;
- which locks and indexes are involved;
- when the operation must abort; and
- what evidence remains after commit or rollback.

Human-oriented prompts and textual `EXPLAIN` output are too ambiguous for this
job. A missed flag can turn a bounded update into an incident.

## Target users and jobs

- Engineering and data agents performing maintenance or remediation.
- Developers automating one-off production-safe DML.
- Platform teams standardizing controls around autonomous database access.
- Reviewers approving a prepared data-change plan.

The primary job is: **turn proposed SQL and safety limits into an immutable plan,
then apply exactly that plan or refuse.**

## Product principles

1. Read-only is the default.
2. Planning and applying are separate operations.
3. Every write has explicit row, time, and scope limits.
4. Drift invalidates a plan.
5. Credentials never appear in arguments, output, or receipts.
6. Refusal is a successful safety outcome with a stable reason code.
7. PostgreSQL correctness comes before broad database support.

## Proposed command contract

```text
dmlpact schema --brief --format json
dmlpact inspect --database app --table public.accounts --format json
dmlpact query --sql-file selection.sql --read-only --format json
dmlpact plan --sql-file update.sql --max-rows 500 --timeout 20s --out plan.json
dmlpact apply plan.json --approve-digest <digest> --format json
dmlpact receipt show <receipt-id> --format json
```

Connection material is supplied through environment variables, protected files,
or a future secret-broker adapter—not command-line arguments.

## Plan model

An immutable plan records:

- server, database, role, and search-path identities without secrets;
- SQL digest and normalized statement class;
- schema and relevant object fingerprints;
- transaction isolation and lock timeout;
- allowed tables, statement count, and operation types;
- maximum rows, duration, returned bytes, and retry count;
- target-set count and a bounded primary-key sample or digest;
- relevant constraint and index information;
- non-executing `EXPLAIN` output where safe;
- required preconditions and approval digest;
- rollback strategy and whether it is actually available;
- creation and expiration time.

`EXPLAIN ANALYZE` is never used during planning because it can execute DML.

## Apply invariants

`dmlpact apply` must:

1. open a transaction with the declared limits;
2. confirm server, role, schema, and object fingerprints;
3. re-evaluate preconditions and target-set bounds;
4. execute only the planned statement digest;
5. abort on drift, timeout, unexpected statement class, or limit breach;
6. commit only after postconditions pass;
7. emit a receipt for commit or rollback.

An approval acknowledges a specific plan digest. It is not an unconstrained
permission to run SQL.

## Receipt model

The receipt includes plan and SQL digests, database identity, timestamps,
transaction outcome, affected-row counts, notices, bounded returned data,
precondition and postcondition results, schema fingerprints, and audit-safe
error details. It explicitly states whether rollback occurred and whether any
effects outside PostgreSQL transactions were possible.

## Initial scope

Version 0.1 will support:

- PostgreSQL only;
- single-database `INSERT`, `UPDATE`, and `DELETE`;
- parameterized SQL files;
- transaction, statement, and lock timeouts;
- row-count and table allowlists;
- plan expiration and drift detection;
- JSON receipts with redaction;
- interactive human approval or digest-based automation.

## Non-goals

- DDL migrations or schema management.
- Natural-language-to-SQL generation.
- Cross-database transactions.
- A database proxy, policy server, or hosted control plane.
- A guarantee that application-level side effects can be rolled back.
- Automatic production credentials or privilege escalation.

## Differentiation and defensibility

Existing SQL clients optimize for access and execution. DMLPact's value is the
machine-verifiable boundary between proposal and mutation: immutable plans,
fail-closed limits, drift detection, and receipts. Enterprise adoption can
compound through policy templates and database-specific safety knowledge.

## Success measures

- Zero executions beyond declared row and statement limits.
- Unsafe or drifted plans refused in the benchmark suite.
- Planning overhead and apply latency.
- Number of prevented near-misses in real workflows.
- Percentage of receipts accepted by security and audit teams.
- Time and tokens required for an agent to complete a bounded data change.

## Key risks and open questions

- The target set can change between planning and transaction acquisition.
- Large validation queries can cause their own load or locks.
- Triggers and stored procedures may create effects not obvious from SQL text.
- Row counts are not sufficient for every business invariant.
- Enterprise environments vary in proxying, permissions, and audit requirements.

The project must never imply that a transactional receipt replaces database
backups, application invariants, or operational review.
