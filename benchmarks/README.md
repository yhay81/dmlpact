# DMLPact performance baseline

This directory defines the reproducible, observation-only PostgreSQL baseline
used to calibrate DMLPact's v1.0 performance, locking, and resource thresholds.
Timing and memory are not yet release thresholds.

## Workloads

`setup.sql` creates a dedicated 100,000-row synthetic table with a primary key,
10,000 selected rows, and no triggers, rewrite rules, row-level security, or
referential actions. The harness measures fresh CLI processes for:

- plan, apply, and offline receipt verification over exactly 10,000 targets;
- plan, apply, and offline receipt verification over exactly 100,000 targets;
- a 250 ms lock-timeout refusal while another session holds an
  `ACCESS EXCLUSIVE` table lock;
- complete plan JSON Schema generation.

Committed runs must change exactly the fingerprinted target count, leave the
90,000 non-target rows unchanged in the 10k case, produce a complete
integrity-valid two-event receipt, and match the expected final database state.
The contention run must refuse without mutation and also produce a complete
integrity-valid receipt.

The raw result records GNU `time` wall time and peak resident memory, SQL, plan,
and receipt bytes, target-set and artifact digests, database schema and identity,
PostgreSQL and runner images, affected-row and post-state checks, lock timeout,
and the exact DMLPact commit. Database setup and the release build are excluded.

Apply timing includes both DMLPact's locked client-side revalidation and server
mutation execution. It is an end-to-end upper bound, not an isolated claim
about the v1.0 client-overhead threshold.

## Run

The supported environment is the `ubuntu-latest` runner and digest-pinned
`postgres:17-alpine` service selected by `.github/workflows/benchmark.yml`. For
safety, `run.sh` refuses to reset any database whose current name is not exactly
`dmlpact_benchmark`.

Run manually with the **Benchmark** workflow, or against an expendable local
database:

```bash
export DMLPACT_DATABASE_URL='postgresql://postgres:postgres@127.0.0.1:5432/dmlpact_benchmark'
benchmarks/run.sh benchmark-results.json
jq . benchmark-results.json
```

GNU `time`, GNU `stat`, `timeout`, `psql`, `jq`, Git, Cargo, and the locked Rust
dependency graph are required. Plans and receipts are temporary and are not
uploaded.

The workflow retains raw JSON for 90 days. Pull requests gate exact mutation,
receipt, resource-bound, and lock-refusal semantics—not observed timing or
memory. A single shared-runner sample is not a regression and does not
establish p95. Before enabling v1.0 thresholds, publish the sample and warm-up
policy, client/server timing separation, baseline window, p95 calculation, and
a noise-aware regression rule.
