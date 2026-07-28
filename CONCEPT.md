# DMLPact concept

## Thesis

DMLPact turns proposed PostgreSQL DML and explicit safety limits into an
immutable, expiring contract, then applies exactly that contract or refuses.

## Problem

SQL clients optimize for access and execution. They do not establish that the
database, schema, target rows, and safety budget observed during review are
still the same at execution time. This gap is dangerous for unattended
automation and easy for humans to miss.

Text prompts and a row count are insufficient. A target set can change while
keeping the same count; an `EXPLAIN ANALYZE` can execute DML; triggers and
row-level security can hide effects; a lost connection during commit can make
the outcome unknowable.

## Contract

DMLPact separates the workflow into four explicit artifacts:

1. The SQL file is the proposal and remains outside the plan.
2. The sealed plan binds exact SQL digests to database identity, schema
   evidence, target count, target-row fingerprint, limits, and expiration.
3. Apply revalidates that evidence under a table lock before execution.
4. The hash-linked receipt records preparation and the observed terminal
   outcome.

Passing a plan file to `apply` is the approval action. A modified plan fails its
self-hash; modified SQL fails its raw or normalized digest.

## v0.1 safety boundary

The first implementation supports one ordinary permanent PostgreSQL table and
one plain `INSERT VALUES`, `UPDATE ... WHERE`, or `DELETE ... WHERE` statement.
It rejects dynamic expressions, auxiliary relations, server-side functions,
and syntax whose effect boundary cannot be established with the implemented
analysis.

Planning uses a read-only `REPEATABLE READ` transaction. Applying uses
`REPEATABLE READ`, local statement/lock/idle timeouts, and a
`SHARE ROW EXCLUSIVE` table lock. This conflicts with concurrent writers before
the final evidence is captured while allowing ordinary readers.

UPDATE and DELETE targets are serialized by PostgreSQL as canonical `jsonb`
text, ordered deterministically, length-delimited, and hashed with SHA-256.
Evidence is capped at 64 MiB. INSERT binds the exact normalized values statement
and its row count.

## Receipt semantics

The receipt path must not exist. DMLPact creates it before connecting for the
apply transaction and durably writes a `prepared` event. A second event records:

- `committed`: PostgreSQL confirmed commit;
- `rolled_back`: execution began but the transaction was rolled back;
- `refused`: no DML was executed because a precondition failed;
- `uncertain`: commit was attempted but its result could not be observed.

A prepared-only receipt is incomplete. It is deliberately distinguishable from
a complete receipt, so a crash cannot be misreported as a confirmed rollback.

## Non-goals

- Generating SQL from natural language.
- DDL, stored procedures, arbitrary queries, or cross-database transactions.
- Replacing database backups, application authorization, or business review.
- Proving side effects in external systems triggered by database code.
- Automatically escalating privileges or retrieving production credentials.

## Success measures

- No execution after a plan, SQL, identity, schema, count, target-set, or budget
  mismatch.
- Every post-preparation outcome is explicit and integrity-verifiable.
- Stable JSON contracts can be consumed without scraping human text.
- Supported changes remain easy to understand and complete in a few commands.
- Real users can report prevented near misses and independently verify releases.
