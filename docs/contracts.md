# Machine contracts

## Output

Successful commands emit one pretty-printed JSON document to stdout, except
`completions`, which emits shell source. Errors emit one compact JSON document
to stderr and leave stdout empty.

`apply` safety refusals exit 0 only after a complete `refused` receipt exists.
This lets automation distinguish a successful safety decision from a CLI,
transport, or malformed-contract failure. Always inspect the returned `state`.

## Stable schemas

Generate current JSON Schemas with:

```text
dmlpact schema plan
dmlpact schema receipt-event
dmlpact schema receipt-verification
dmlpact schema apply-result
dmlpact schema error
dmlpact schema capabilities
```

Schema identifiers use `dmlpact.<document>.v1`. Additive fields within a schema
version are avoided in sealed plan and receipt input documents because unknown
fields are denied. A breaking contract requires a new schema identifier.

The versioned fixtures and self-consistent adversarial mutations in
[`tests/fixtures/contracts/`](../tests/fixtures/contracts/README.md) pin plan
seals and receipt-chain behavior without contacting PostgreSQL.

## Exit classes

| Code | Class | Meaning |
| ---: | --- | --- |
| 0 | success | Completed, including a fully receipted `refused` outcome |
| 1 | I/O | File, clock, TLS, connection, or database operation failed |
| 2 | usage | CLI value or local input shape is invalid |
| 3 | policy | SQL or database object is outside the supported safety boundary |
| 4 | budget | Row or evidence budget was exceeded |
| 5 | contract | Plan/receipt integrity failed, or commit outcome is uncertain |

## Apply reason codes

Common terminal reason codes include:

- `applied`
- `target_count_drift`
- `target_set_drift`
- `table_schema_drift`
- `database_identity_drift`
- `transport_policy_drift`
- `row_budget_exceeded`
- `user_triggers_denied`
- `row_security_denied`
- `statement_execution_failed`
- `affected_rows_mismatch`
- `commit_outcome_uncertain`

Consumers should preserve unknown reason codes and branch primarily on
`state`; new non-breaking diagnostic codes may be added.

## Receipt verification

`receipt verify` is offline. It validates strict JSON decoding, schema/tool
versions, event self-hashes, sequence, identity linkage, previous-event hash,
timestamps, and terminal-state shape. `integrity_valid: true` does not attest to
the truthfulness of a compromised PostgreSQL server or local binary.
