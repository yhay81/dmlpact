# Safety model

## Trust boundary

DMLPact trusts the local binary, operating system, PostgreSQL server, connection
endpoint, and the role's catalog visibility. It treats SQL files, plan files,
receipt files, database state, and command callers as potentially stale or
incorrect.

The plan and receipt use unkeyed SHA-256 integrity hashes. They detect accidental
or uncoordinated modification; they are not digital signatures and do not stop
an attacker who can replace both an artifact and its expected trust context.
Use signed Git commits/releases and access-controlled artifact storage where
authenticity matters.

## Planning invariants

- SQL is valid UTF-8, at most 1 MiB, and parses as exactly one supported
  PostgreSQL statement.
- UPDATE and DELETE have an explicit WHERE expression.
- Functions, subqueries, placeholders, joins, auxiliary tables, and returning
  clauses are absent.
- The connection is TLS-protected unless every endpoint is local and the caller
  explicitly opts in to plaintext.
- The target resolves to one ordinary permanent table.
- The target has no inheritance/partition links and is not referenced by a
  foreign key that can cascade or set values in another table.
- Rewrite rules are absent. User triggers and row-level security are absent
  unless explicitly acknowledged in the sealed plan.
- INSERT targets have no defaults, identity columns, or generated columns.
- The target count is within `max_rows`.
- Target-row evidence is at most 64 MiB.
- `EXPLAIN` is used without `ANALYZE`.

## Apply invariants

Before DML, DMLPact durably creates a prepared receipt, starts a
`REPEATABLE READ` transaction, sets local timeouts, and acquires a
`SHARE ROW EXCLUSIVE` lock. It then compares:

- transport policy;
- endpoint fingerprint, observed server address/port, database, role, search
  path, interpretation settings, and server version;
- canonical table identity and safety-relevant catalog fingerprint;
- raw SQL, normalized SQL, count query, and target-evidence query digests;
- target count and exact target-row fingerprint;
- trigger/RLS acknowledgements and row budget;
- plan self-hash and expiry.

Any mismatch refuses before statement execution. After execution, affected rows
must exactly equal the pre-count. Otherwise the transaction rolls back.

## Known limits

- Table-level locking can block writers and may be operationally expensive.
- Counting and canonicalizing target rows can scan substantial data. Timeouts
  limit duration, but query plans and indexes still matter.
- `--allow-triggers` acknowledges all current user triggers. Triggered writes
  inside PostgreSQL share the transaction, but external effects initiated by
  database extensions or functions may not be reversible.
- `--allow-row-security` acknowledges role-dependent visibility and effects.
- Catalog fingerprints cover columns, constraints, indexes, triggers, rewrite
  rules, and RLS policies. They do not model every extension-specific behavior.
- Built-in-looking operators, casts, types, constraints, indexes, and other
  trusted schema objects can invoke server-side code. DMLPact rejects explicit
  function expressions but does not prove that a PostgreSQL schema contains no
  user-defined code.
- A server or privileged role can misrepresent catalog/state evidence.
- `uncertain` means operators must reconcile the database using the plan and
  receipt; retrying blindly may duplicate intended effects.

## Operational recommendations

- Use a least-privilege role dedicated to the intended table and operation.
- Keep `max_rows`, timeouts, and plan lifetime as small as practical.
- Review the plan digest and preconditions in a separate approval step.
- Store SQL, plan, and receipt together in access-controlled audit storage.
- Test rollback and reconciliation procedures before production use.
- Monitor PostgreSQL locks, statement timeouts, replication lag, and audit logs.
