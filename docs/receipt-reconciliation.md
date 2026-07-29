# Receipt reconciliation

DMLPact writes a durable `prepared` receipt event before it starts a database
transaction and appends one terminal event after the transaction reaches an
observed outcome. The database and the local receipt file cannot participate in
one atomic transaction, so an interruption or storage failure can require
operator reconciliation.

## Never retry an incomplete apply blindly

Preserve the SQL file, sealed plan, receipt file, stderr error document, and
relevant PostgreSQL audit or application records. Do not edit or append to the
receipt manually.

Run the offline verifier first:

```bash
dmlpact receipt verify --receipt change.receipt.ndjson
```

Use the result and the original apply error as follows:

| Evidence | Meaning | Required action |
| --- | --- | --- |
| Complete receipt, `committed` | DMLPact observed the commit and durably recorded it | Do not retry |
| Complete receipt, `rolled_back` or `refused` | DMLPact recorded a non-committed outcome | Correct the cause and create a fresh plan before another apply |
| Complete receipt, `uncertain` | PostgreSQL did not give DMLPact a reliable commit outcome | Reconcile the target rows and database audit evidence before deciding whether any new DML is safe |
| `committed_receipt_finalization_uncertain` error | PostgreSQL commit succeeded, but DMLPact could not confirm durable publication of the terminal receipt event | Do not retry; preserve the receipt and reconcile its planned target set with the committed database state |
| `commit_receipt_finalization_uncertain` error | Neither the PostgreSQL commit outcome nor durable publication of the terminal receipt event could be confirmed | Do not retry; preserve all evidence and reconcile the target rows and database audit evidence |
| Incomplete receipt ending at `prepared` | No durable terminal outcome is available | Treat the outcome as operationally uncertain unless the apply error proves the commit succeeded; reconcile before any retry |
| Invalid or unreadable receipt | The evidence is truncated, corrupt, or unavailable | Preserve the original bytes and use the plan, target rows, audit records, and application state to reconcile; never repair the receipt in place |

The verifier can reject a partially appended terminal event rather than return
an incomplete result. A terminal event may also be readable even when the
filesystem could not confirm its durability. In both cases, retain the original
apply error because it records whether the database commit itself was observed.

## Reconciliation checklist

1. Confirm the plan hash, SQL hash, database identity, canonical table, and
   target-set hash from the preserved plan and prepared receipt.
2. Inspect the exact planned target rows using a read-only connection and the
   same role and database identity.
3. Compare PostgreSQL audit logs, application records, or another authoritative
   source with the intended mutation and apply time.
4. Record the conclusion outside the immutable receipt, including the evidence
   used and the operator identity.
5. Create a fresh plan only after determining that another mutation is safe.

The unkeyed receipt hashes detect accidental or uncoordinated modification; they
do not prove who performed the database operation. Use access-controlled audit
storage and PostgreSQL auditing where attribution is required.
